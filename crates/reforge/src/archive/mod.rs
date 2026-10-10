//! The archive: a `.fantome` or a plain zip of the same layout, from an untrusted site. This is
//! the only reader of untrusted zips.
//!
//! Everything is read under caps, and nothing is extracted to disk (#ADR-0024). An archive file
//! is read entry by entry and never held whole; bytes in memory are borrowed and held once
//! (#ADR-0100). Only deflate and stored entries are read. No name
//! the archive spells leaves this module as a string that could become a path: a WAD's name is a
//! `WadName`, a chunk's path a `ChunkPath` or a hash, and every other text a `Spelled`. An
//! archive broken by its format is reported and not recovered (#ADR-0025).
//!
//! The read gives the raw snapshot (#ADR-0094): what the archive alone determines, its entries
//! inflated, its WADs unpacked and each file's bytes put into the blob store under their content
//! id (#ADR-0093). The snapshot names bytes by id and holds none. `load` opens the mod from the
//! snapshot and the store, and nothing of the archive is kept beside them.
//!
//! `docs/design/ARCHITECTURE.md`, "Untrusted input".

mod caps;
mod findings;
mod info;
mod kind;
mod name;
mod packed;
mod reader;
mod source;

use std::collections::BTreeMap;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

pub use self::caps::Caps;
pub use self::findings::{
    BudgetExceeded, CODES, ChunkDuplicates, CodecEntry, CodecsUnsupported, DuplicateChunk,
    EntriesCorrupt, EntriesTooLarge, EntriesUnreadable, InfoUnreadable, LargeEntry, NamesUnsafe,
    NotZip, RepackDuplicates, StoredCrcWrong, TooManyEntries, UnreadableEntry, UnreadableWad,
    UnsafeName, WadUnreadable,
};
pub use self::kind::file_kind;
pub use self::packed::PackedError;
use self::reader::{Parts, Reader};
use self::source::Source;
use crate::diag::{Halted, Logged, Sink};
use crate::model::entity::{Extra, ModImage, ModMeta, WadLayout};
use crate::model::{ByteView, ChunkPath, ContentId, Spelled, WadHash, WadName};
use crate::store::BlobStore;

/// The raw reading's version (#ADR-0095): one number, raised when the zip or WAD reading, the
/// hashing or the snapshot's shape changes.
pub const RAW_VERSION: u32 = 4;

/// One file of a WAD, as the reader holds it: its bytes are in the store from the moment they
/// are inflated, and the reader keeps their id alone (#ADR-0100).
#[derive(Debug, Clone)]
struct Chunk {
    /// The hash the client finds it by.
    hash: WadHash,
    /// Its path, when the archive spells one. A chunk named by its hash and a chunk of a packed
    /// WAD have none, unless another entry of the WAD spells the same path.
    path: Option<ChunkPath>,
    /// The id its bytes are in the store under.
    content: ContentId,
}

/// A WAD as the reader holds it: its loose chunks and the chunks of its packed form, as one.
#[derive(Debug, Clone)]
struct ArchiveWad {
    /// The name as the archive first spells it.
    name: WadName,
    /// How the archive holds it: packed when any packed form of it was read.
    layout: WadLayout,
    /// Its chunks, in path-hash order. The first chunk the archive lists at a path is the chunk.
    chunks: Vec<Chunk>,
}

/// One file of a WAD of the raw snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawFile {
    /// Its path, when the archive spells one. A chunk named by its hash and a chunk of a packed
    /// WAD have none, unless another entry of the WAD spells the same path.
    pub path: Option<ChunkPath>,
    /// The id its bytes are in the store under.
    pub content: ContentId,
}

/// A WAD of the raw snapshot: its loose chunks and the chunks of its packed form, as one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawWad {
    /// The name as the archive first spells it.
    pub name: WadName,
    /// How the archive holds it: packed when any packed form of it was read.
    pub layout: WadLayout,
    /// Its files, by the hash the client finds each by. The first chunk the archive lists at a
    /// path is the file.
    #[serde(with = "files_by_hash")]
    pub files: BTreeMap<WadHash, RawFile>,
}

/// A WAD's files as JSON: a list in hash order, each hash written as the report writes a chunk
/// hash, since a JSON map's keys are strings. A hash listed twice does not read.
mod files_by_hash {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::RawFile;
    use crate::model::WadHash;
    use crate::model::hash::wad_hex;

    #[derive(Serialize, Deserialize)]
    struct Row {
        #[serde(with = "wad_hex")]
        hash: WadHash,
        #[serde(flatten)]
        file: RawFile,
    }

    pub(super) fn serialize<S: Serializer>(
        files: &BTreeMap<WadHash, RawFile>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.collect_seq(files.iter().map(|(&hash, file)| Row {
            hash,
            file: file.clone(),
        }))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<WadHash, RawFile>, D::Error> {
        let rows = Vec::<Row>::deserialize(d)?;
        let listed = rows.len();
        let files: BTreeMap<_, _> = rows.into_iter().map(|row| (row.hash, row.file)).collect();
        if files.len() != listed {
            return Err(serde::de::Error::custom("a chunk hash is listed twice"));
        }
        Ok(files)
    }
}

/// An entry of the archive outside `META/info.json` and the WADs. Its first bytes are in the
/// store, enough to tell what it is; the rest are not read (#ADR-0051).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawExtra {
    /// The entry's name.
    pub name: Spelled,
    /// Its inflated size.
    pub size: u64,
    /// The id its first bytes are in the store under.
    pub head: ContentId,
}

/// The archive's `META/image.png`: the id of its bytes, or why the run does not hold them
/// (#ADR-0051). The image is the one extra read whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "image", content = "of", rename_all = "kebab-case")]
pub enum RawImage {
    /// The entry's bytes, inflated, are in the store under this id.
    Held(ContentId),
    /// The entry is over `MAX_IMAGE_BYTES`, at this size.
    TooLarge(u64),
    /// The entry does not read.
    Unreadable,
}

impl RawImage {
    /// The image as the mod holds it, with its bytes from the store, or the id the store
    /// lacks.
    pub fn open(self, store: &dyn BlobStore) -> Result<ModImage, ContentId> {
        Ok(match self {
            RawImage::Held(id) => ModImage::Held(store.get(id).ok_or(id)?),
            RawImage::TooLarge(bytes) => ModImage::TooLarge(bytes),
            RawImage::Unreadable => ModImage::Unreadable,
        })
    }
}

/// The raw snapshot (#ADR-0094): what the archive alone determines, with every byte of it in
/// the store and named here by content id. The stage `read` returns it.
///
/// Its JSON is written and read where the other snapshots' are (`snapshot::to_json`). A
/// change of its shape raises [`RAW_VERSION`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSnapshot {
    /// The raw reading's version, [`RAW_VERSION`] when this program wrote it.
    pub version: u32,
    /// The caps it was read under: what was inflated and what refused is theirs (#ADR-0095).
    pub caps: Caps,
    /// What the archive says of itself, and what identifies it: its SHA-256 among the rest.
    /// The file name is the caller's word for display, not the archive's.
    pub meta: ModMeta,
    /// Its WADs, in lowercase name order.
    pub wads: Vec<RawWad>,
    /// Its entries outside `META/info.json` and the WADs, in archive order. A separator `\` in a
    /// name is written `/`.
    pub extras: Vec<RawExtra>,
    /// Its `META/image.png`, when it has one. The entry is among the extras too.
    pub image: Option<RawImage>,
    /// What the read raised, in order.
    pub findings: Vec<Logged>,
}

impl RawExtra {
    /// The extra as the mod holds it, with its first bytes from the store, or the id the store
    /// lacks.
    pub fn open(&self, store: &dyn BlobStore) -> Result<Extra, ContentId> {
        Ok(Extra {
            name: self.name.clone(),
            size: self.size,
            head: store.get(self.head).ok_or(self.head)?.to_vec(),
        })
    }
}

impl RawSnapshot {
    /// The extras as the mod holds them, with each one's first bytes from the store, or the
    /// first id the store lacks.
    pub fn open_extras(&self, store: &dyn BlobStore) -> Result<Vec<Extra>, ContentId> {
        self.extras.iter().map(|extra| extra.open(store)).collect()
    }

    /// Every id it names: its files', its extras' first bytes' and its image's.
    pub fn ids(&self) -> impl Iterator<Item = ContentId> + '_ {
        let files = self.wads.iter().flat_map(|wad| wad.files.values());
        let image = self.image.and_then(|image| match image {
            RawImage::Held(id) => Some(id),
            RawImage::TooLarge(_) | RawImage::Unreadable => None,
        });
        let files = files.map(|file| file.content);
        let heads = self.extras.iter().map(|extra| extra.head);
        files.chain(heads).chain(image)
    }

    /// What the reader found, the extras' first bytes and the image put into the store beside
    /// the chunks it put there as it read them.
    fn of(meta: ModMeta, caps: Caps, parts: Parts, store: &dyn BlobStore) -> Self {
        let wads = parts
            .wads
            .into_iter()
            .map(|wad| RawWad {
                name: wad.name,
                layout: wad.layout,
                files: wad
                    .chunks
                    .into_iter()
                    .map(|chunk| {
                        let file = RawFile {
                            path: chunk.path,
                            content: chunk.content,
                        };
                        (chunk.hash, file)
                    })
                    .collect(),
            })
            .collect();
        let extras = parts
            .extras
            .into_iter()
            .map(|extra| RawExtra {
                name: extra.name,
                size: extra.size,
                head: store.put(ByteView::from(extra.head)),
            })
            .collect();
        let image = parts.image.map(|image| match image {
            ModImage::Held(bytes) => RawImage::Held(store.put(bytes)),
            ModImage::TooLarge(bytes) => RawImage::TooLarge(bytes),
            ModImage::Unreadable => RawImage::Unreadable,
        });
        Self {
            version: RAW_VERSION,
            caps,
            meta,
            wads,
            extras,
            image,
            findings: Vec::new(),
        }
    }
}

/// An archive to be read: its bytes in memory or its file, with its SHA-256, which identifies
/// it before anything is inflated.
#[derive(Debug, Clone, Copy)]
pub struct Archive<'a> {
    source: Source<'a>,
    file_name: &'a str,
    sha256: ContentId,
}

impl<'a> Archive<'a> {
    /// An archive in memory, borrowed for the read and not kept. `file_name` is for display and
    /// is never opened.
    pub fn bytes(bytes: &'a [u8], file_name: &'a str) -> Self {
        Self {
            source: Source::Bytes(bytes),
            file_name,
            sha256: ContentId::of(bytes),
        }
    }

    /// An archive file, its SHA-256 taken in one pass that holds nothing of it. A file over the
    /// cap for the archive file, or one that does not read, ends the read.
    pub fn file(path: &'a Utf8Path, caps: &Caps, sink: &mut Sink<'_>) -> Result<Self, Halted> {
        let not_read = |error: std::io::Error| NotZip {
            file: Spelled::new(file_name_of(path)),
            reason: error.to_string(),
        };
        let len = match fs_err::metadata(path) {
            Ok(metadata) => metadata.len(),
            Err(error) => return Err(sink.halt(not_read(error))),
        };
        if len > caps.archive_bytes {
            return Err(sink.halt(BudgetExceeded::ArchiveFile {
                bytes: len,
                cap: caps.archive_bytes,
            }));
        }
        let sha256 = fs_err::File::open(path)
            .map(std::io::BufReader::new)
            .and_then(ContentId::of_reader)
            .map_err(|error| sink.halt(not_read(error)))?;
        Ok(Self {
            source: Source::File { path, len },
            file_name: file_name_of(path),
            sha256,
        })
    }

    /// The archive's SHA-256.
    pub fn sha256(&self) -> ContentId {
        self.sha256
    }

    /// The name it is shown by.
    pub fn file_name(&self) -> &'a str {
        self.file_name
    }

    /// Reads the archive into the store: the stage `read`. A file is read entry by entry, and
    /// a stored packed WAD where it lies in it (#ADR-0100).
    ///
    /// A problem short of a fatal one is a diagnostic, and the archive is read as far as it
    /// reads.
    pub fn read(
        &self,
        caps: &Caps,
        store: &dyn BlobStore,
        sink: &mut Sink<'_>,
    ) -> Result<RawSnapshot, Halted> {
        let file_name = Spelled::new(self.file_name);
        let _span = tracing::info_span!("read", archive = %file_name).entered();
        let mark = sink.mark();
        let mut reader = match Reader::open(self.source, &file_name, caps, store) {
            Ok(reader) => reader,
            Err(fatal) => return Err(fatal.halt(sink)),
        };
        let outcome = reader.read_entries(sink);
        let mut parts = reader.finish();
        // The findings of the entries read are raised also when the read ends early.
        std::mem::take(&mut parts.findings).raise(caps, sink);
        if let Err(fatal) = outcome {
            return Err(fatal.halt(sink));
        }
        let info = std::mem::take(&mut parts.info);
        let meta = ModMeta {
            file_name,
            sha256: self.sha256,
            mod_id: self.sha256.hex()[..12].to_string(),
            name: info.name,
            author: info.author,
            version: info.version,
            description: info.description,
        };
        let mut raw = RawSnapshot::of(meta, *caps, parts, store);
        raw.findings = sink.logged_since(mark);
        Ok(raw)
    }
}

/// Reads an archive from its bytes, which are borrowed for the call and not kept, into the
/// store: the stage `read`. `file_name` is for display and is never opened.
pub fn read(
    bytes: &[u8],
    file_name: &str,
    caps: &Caps,
    store: &dyn BlobStore,
    sink: &mut Sink<'_>,
) -> Result<RawSnapshot, Halted> {
    Archive::bytes(bytes, file_name).read(caps, store, sink)
}

/// Reads an archive file into the store, entry by entry. A file over the cap for the archive
/// file is not read.
pub fn read_file(
    path: &Utf8Path,
    caps: &Caps,
    store: &dyn BlobStore,
    sink: &mut Sink<'_>,
) -> Result<RawSnapshot, Halted> {
    Archive::file(path, caps, sink)?.read(caps, store, sink)
}

/// The name of an archive file, as `read` takes it for display.
pub fn file_name_of(path: &Utf8Path) -> &str {
    path.file_name().unwrap_or(path.as_str())
}
