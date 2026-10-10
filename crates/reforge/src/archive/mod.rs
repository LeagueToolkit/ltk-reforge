//! The archive: a `.fantome` or a plain zip of the same layout, from an untrusted site. This is
//! the only reader of untrusted zips.
//!
//! Everything is read into memory under caps, and nothing is extracted to disk (#ADR-0024). The
//! archive's bytes are borrowed and held once. Only deflate and stored entries are read. No name
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
use crate::diag::{Halted, Logged, Sink};
use crate::model::entity::{Extra, ModImage, ModMeta, WadLayout};
use crate::model::{ByteView, ChunkPath, ContentId, Spelled, WadHash, WadName};
use crate::store::BlobStore;

/// The raw reading's version (#ADR-0095): one number, raised when the zip or WAD reading, the
/// hashing or the snapshot's shape changes.
pub const RAW_VERSION: u32 = 2;

/// One file of a WAD, as the reader holds it before its bytes go into the store.
#[derive(Debug, Clone)]
struct Chunk {
    /// The hash the client finds it by.
    hash: WadHash,
    /// Its path, when the archive spells one. A chunk named by its hash and a chunk of a packed
    /// WAD have none, unless another entry of the WAD spells the same path.
    path: Option<ChunkPath>,
    /// Its bytes, inflated.
    bytes: ByteView,
}

impl Chunk {
    fn new(hash: WadHash, path: Option<ChunkPath>, bytes: &[u8]) -> Self {
        Self {
            hash,
            path,
            bytes: ByteView::from(bytes),
        }
    }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFile {
    /// Its path, when the archive spells one. A chunk named by its hash and a chunk of a packed
    /// WAD have none, unless another entry of the WAD spells the same path.
    pub path: Option<ChunkPath>,
    /// The id its bytes are in the store under.
    pub content: ContentId,
}

/// A WAD of the raw snapshot: its loose chunks and the chunks of its packed form, as one.
#[derive(Debug, Clone)]
pub struct RawWad {
    /// The name as the archive first spells it.
    pub name: WadName,
    /// How the archive holds it: packed when any packed form of it was read.
    pub layout: WadLayout,
    /// Its files, by the hash the client finds each by. The first chunk the archive lists at a
    /// path is the file.
    pub files: BTreeMap<WadHash, RawFile>,
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
#[derive(Debug, Clone)]
pub struct RawSnapshot {
    /// The raw reading's version, [`RAW_VERSION`] when this program wrote it.
    pub version: u32,
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

    /// What the reader found, its bytes put into the store.
    fn of(meta: ModMeta, parts: Parts, store: &dyn BlobStore) -> Self {
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
                            content: store.put(chunk.bytes),
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
            meta,
            wads,
            extras,
            image,
            findings: Vec::new(),
        }
    }
}

/// Reads an archive from its bytes, which are borrowed for the call and not kept, into the
/// store: the stage `read`.
///
/// A problem short of a fatal one is a diagnostic, and the archive is read as far as it reads.
/// `file_name` is for display and is never opened.
pub fn read(
    bytes: &[u8],
    file_name: &str,
    caps: &Caps,
    store: &dyn BlobStore,
    sink: &mut Sink<'_>,
) -> Result<RawSnapshot, Halted> {
    let file_name = Spelled::new(file_name);
    let _span = tracing::info_span!("read", archive = %file_name).entered();
    let mark = sink.mark();
    let mut reader = match Reader::open(bytes, &file_name, caps) {
        Ok(reader) => reader,
        Err(fatal) => return Err(fatal.halt(sink)),
    };
    let outcome = reader.read_entries();
    let mut parts = reader.finish();
    // The findings of the entries read are raised also when the read ends early.
    std::mem::take(&mut parts.findings).raise(caps, sink);
    if let Err(fatal) = outcome {
        return Err(fatal.halt(sink));
    }
    let sha256 = ContentId::of(bytes);
    let mod_id = sha256.hex()[..12].to_string();
    let info = std::mem::take(&mut parts.info);
    let meta = ModMeta {
        file_name,
        sha256,
        mod_id,
        name: info.name,
        author: info.author,
        version: info.version,
        description: info.description,
    };
    let mut raw = RawSnapshot::of(meta, parts, store);
    raw.findings = sink.logged_since(mark);
    Ok(raw)
}

/// Reads an archive file into the store. A file over the cap for the archive file is not read.
pub fn read_file(
    path: &Utf8Path,
    caps: &Caps,
    store: &dyn BlobStore,
    sink: &mut Sink<'_>,
) -> Result<RawSnapshot, Halted> {
    let file_name = path.file_name().unwrap_or(path.as_str());
    let not_read = |error: std::io::Error| NotZip {
        file: Spelled::new(file_name),
        reason: error.to_string(),
    };
    let size = match fs_err::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) => return Err(sink.halt(not_read(error))),
    };
    if size > caps.archive_bytes {
        return Err(sink.halt(BudgetExceeded::ArchiveFile {
            bytes: size,
            cap: caps.archive_bytes,
        }));
    }
    match fs_err::read(path) {
        Ok(bytes) => read(&bytes, file_name, caps, store, sink),
        Err(error) => Err(sink.halt(not_read(error))),
    }
}
