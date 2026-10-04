//! The archive: a `.fantome` or a plain zip of the same layout, from an untrusted site. This is
//! the only reader of untrusted zips.
//!
//! Everything is read into memory under caps, and nothing is extracted to disk (#ADR-0024). The
//! archive's bytes are borrowed and held once. Only deflate and stored entries are read. No name
//! the archive spells leaves this module as a string that could become a path: a WAD's name is a
//! `WadName`, a chunk's path a `ChunkPath` or a hash, and every other text a `Spelled`. An
//! archive broken by its format is reported and not recovered (#ADR-0025).
//!
//! `docs/design/ARCHITECTURE.md`, "Untrusted input".

mod caps;
mod findings;
mod info;
mod kind;
mod name;
mod packed;
mod reader;

use std::sync::Arc;

use camino::Utf8Path;

pub use self::caps::Caps;
pub use self::findings::{
    BudgetExceeded, CODES, ChunkDuplicates, CodecEntry, CodecsUnsupported, DuplicateChunk,
    EntriesCorrupt, EntriesTooLarge, EntriesUnreadable, InfoUnreadable, LargeEntry, NamesUnsafe,
    NotZip, RepackDuplicates, StoredCrcWrong, TooManyEntries, UnreadableEntry, UnreadableWad,
    UnsafeName, WadUnreadable,
};
pub use self::kind::file_kind;
pub use self::packed::PackedError;
use self::reader::Reader;
use crate::diag::{Halted, Sink};
use crate::model::entity::{Extra, FileKind, ModImage, ModMeta, WadLayout};
use crate::model::{ChunkPath, ContentId, Spelled, WadHash, WadName};

/// One file of a WAD of the archive, in memory.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The hash the client finds it by.
    pub hash: WadHash,
    /// Its path, when the archive spells one. A chunk named by its hash and a chunk of a packed
    /// WAD have none, unless another entry of the WAD spells the same path.
    pub path: Option<ChunkPath>,
    /// Its bytes, inflated.
    pub bytes: Arc<[u8]>,
    /// The digest of its bytes.
    pub content: ContentId,
    /// What it is, by its first bytes.
    pub kind: FileKind,
}

impl Chunk {
    fn new(hash: WadHash, path: Option<ChunkPath>, bytes: &[u8]) -> Self {
        Self {
            hash,
            path,
            bytes: Arc::from(bytes),
            content: ContentId::of(bytes),
            kind: file_kind(bytes),
        }
    }
}

/// A WAD of the archive: its loose chunks and the chunks of its packed form, as one.
#[derive(Debug, Clone)]
pub struct ArchiveWad {
    /// The name as the archive first spells it.
    pub name: WadName,
    /// How the archive holds it: packed when any packed form of it was read.
    pub layout: WadLayout,
    /// Its chunks, in path-hash order. The first chunk the archive lists at a path is the chunk.
    pub chunks: Vec<Chunk>,
}

/// One mod archive, read into memory: the stage `read` returns.
#[derive(Debug, Clone)]
pub struct Archive {
    /// What the archive says of itself, and what identifies it.
    pub meta: ModMeta,
    /// Its WADs, in lowercase name order.
    pub wads: Vec<ArchiveWad>,
    /// Its entries outside `META/info.json` and the WADs, in archive order. A separator `\` in a
    /// name is written `/`.
    pub extras: Vec<Extra>,
    /// Its `META/image.png`, when it has one. The entry is among the extras too.
    pub image: Option<ModImage>,
}

impl Archive {
    /// Reads an archive from its bytes, which are borrowed for the call and not kept.
    ///
    /// A problem short of a fatal one is a diagnostic, and the archive is read as far as it
    /// reads. `file_name` is for display and is never opened.
    pub fn read(
        bytes: &[u8],
        file_name: &str,
        caps: &Caps,
        sink: &mut Sink<'_>,
    ) -> Result<Archive, Halted> {
        let file_name = Spelled::new(file_name);
        let _span = tracing::info_span!("read", archive = %file_name).entered();
        let mut reader = match Reader::open(bytes, &file_name, caps) {
            Ok(reader) => reader,
            Err(fatal) => return Err(fatal.halt(sink)),
        };
        let outcome = reader.read_entries();
        let parts = reader.finish();
        // The findings of the entries read are raised also when the read ends early.
        parts.findings.raise(caps, sink);
        if let Err(fatal) = outcome {
            return Err(fatal.halt(sink));
        }
        let sha256 = ContentId::of(bytes);
        let mod_id = sha256.hex()[..12].to_string();
        Ok(Archive {
            meta: ModMeta {
                file_name,
                sha256,
                mod_id,
                name: parts.info.name,
                author: parts.info.author,
                version: parts.info.version,
                description: parts.info.description,
            },
            wads: parts.wads,
            extras: parts.extras,
            image: parts.image,
        })
    }

    /// Reads an archive file. A file over the cap for the archive file is not read.
    pub fn read_file(path: &Utf8Path, caps: &Caps, sink: &mut Sink<'_>) -> Result<Archive, Halted> {
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
            Ok(bytes) => Self::read(&bytes, file_name, caps, sink),
            Err(error) => Err(sink.halt(not_read(error))),
        }
    }
}
