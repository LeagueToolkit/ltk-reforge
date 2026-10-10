//! What `read` finds: one type per code of `docs/design/CODES.md`, "archive", and the collector
//! that holds the findings of the entries until each code is raised once per subject.
//!
//! Before `load` nothing has an id, so a subject is the name the archive spells: a WAD's, or an
//! entry's own when it is under no WAD.

use std::fmt::Write as _;

use indexmap::IndexMap;
use serde::{Serialize, Serializer};

use super::caps::Caps;
use super::packed::PackedError;
use crate::diag::{Channel, Code, Diag, Message, Record, Severity, Sink, Subject};
use crate::model::{PathError, Spelled, WadHash};

const fn code(name: &'static str, severity: Severity) -> Code {
    Code::new(name, Channel::Input, severity)
}

/// A chunk hash as a report holds it: sixteen hex digits.
pub(super) fn hash_hex<S: Serializer>(hash: &WadHash, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&format!("{:016x}", hash.0))
}

fn error_text<E: ToString, S: Serializer>(error: &E, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&error.to_string())
}

/// Writes `1 entry` or `N entries`, then the text.
fn entries<'a>(f: &'a mut Message, n: usize, text: &str) -> &'a mut Message {
    let _ = write!(f, "{n} {}", if n == 1 { "entry" } else { "entries" });
    f.text(text)
}

/// The archive is not a zip, or its file does not read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotZip {
    /// The archive's file name.
    pub file: Spelled,
    /// What failed.
    pub reason: String,
}

impl Diag for NotZip {
    const CODE: Code = code("archive.not-zip", Severity::Fatal);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message) {
        f.spelled(self.file.raw())
            .text(" is not a zip, or does not read: ")
            .spelled(&self.reason);
    }
}

/// The archive holds more entries than the cap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TooManyEntries {
    /// The entries the archive holds.
    pub entries: usize,
    /// The cap on the entries of one archive.
    pub cap: usize,
}

impl Diag for TooManyEntries {
    const CODE: Code = code("archive.too-many-entries", Severity::Fatal);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message) {
        let _ = write!(f, "{} entries, over the cap of {}", self.entries, self.cap);
    }
}

/// The archive is larger than a run reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "limit", rename_all = "kebab-case")]
pub enum BudgetExceeded {
    /// The archive file is larger than the cap for it. It is not read.
    ArchiveFile {
        /// The file's size.
        bytes: u64,
        /// The cap on the archive file.
        cap: u64,
    },
    /// The entries and the chunks of packed WADs inflate to more than the cap for all of them.
    Inflated {
        /// The cap on all inflated bytes.
        cap: u64,
        /// The entry at which the cap was passed.
        entry: Spelled,
    },
}

impl Diag for BudgetExceeded {
    const CODE: Code = code("archive.budget-exceeded", Severity::Fatal);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message) {
        match self {
            BudgetExceeded::ArchiveFile { bytes, cap } => {
                let _ = write!(f, "the archive is {bytes} bytes, over the cap of {cap}");
            }
            BudgetExceeded::Inflated { cap, entry } => {
                let _ = write!(f, "the archive inflates to more than {cap} bytes, at ");
                f.spelled(entry.raw());
            }
        }
    }
}

/// An entry over the cap for one entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LargeEntry {
    /// The entry.
    pub entry: Spelled,
    /// The inflated size the archive declares for it. One at or under the cap inflates past it.
    pub declared: u64,
}

/// Entries over the cap for one entry, skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntriesTooLarge {
    /// The WAD the entries are under, or the entry itself when it is under none.
    pub within: Spelled,
    /// The cap on one entry, inflated.
    pub cap: u64,
    /// The entries, in archive order.
    pub entries: Vec<LargeEntry>,
}

impl Diag for EntriesTooLarge {
    const CODE: Code = code("archive.entry-too-large", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Entry(self.within.clone())
    }

    fn message(&self, f: &mut Message) {
        let _ = write!(
            entries(f, self.entries.len(), " over the cap of "),
            "{} bytes for one entry, skipped: ",
            self.cap
        );
        f.list(&self.entries, |f, e| {
            f.spelled(e.entry.raw());
        });
    }
}

/// Compressed entries that fail their checksum, skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntriesCorrupt {
    /// The WAD the entries are under, or the entry itself when it is under none.
    pub within: Spelled,
    /// The entries, in archive order.
    pub entries: Vec<Spelled>,
}

impl Diag for EntriesCorrupt {
    const CODE: Code = code("archive.entry-corrupt", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Entry(self.within.clone())
    }

    fn message(&self, f: &mut Message) {
        entries(
            f,
            self.entries.len(),
            " compressed and failing the checksum, skipped: ",
        );
        f.list(&self.entries, |f, e| {
            f.spelled(e.raw());
        });
    }
}

/// An entry that does not open or read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnreadableEntry {
    /// The entry.
    pub entry: Spelled,
    /// What failed.
    pub reason: String,
}

/// Entries that do not open or read, skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntriesUnreadable {
    /// The WAD the entries are under, or the entry itself when it is under none.
    pub within: Spelled,
    /// The entries, in archive order.
    pub entries: Vec<UnreadableEntry>,
}

impl Diag for EntriesUnreadable {
    const CODE: Code = code("archive.entry-unreadable", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Entry(self.within.clone())
    }

    fn message(&self, f: &mut Message) {
        entries(
            f,
            self.entries.len(),
            " not opening or not reading, skipped: ",
        );
        f.list(&self.entries, |f, e| {
            f.spelled(e.entry.raw())
                .text(" (")
                .spelled(&e.reason)
                .text(")");
        });
    }
}

/// An entry in a codec that is not read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodecEntry {
    /// The entry.
    pub entry: Spelled,
    /// The zip method number of its codec: 12 is bzip2, 14 LZMA, 93 Zstandard.
    pub method: u16,
}

/// Entries in a codec other than deflate or stored, skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodecsUnsupported {
    /// The WAD the entries are under, or the entry itself when it is under none.
    pub within: Spelled,
    /// The entries, in archive order.
    pub entries: Vec<CodecEntry>,
}

impl Diag for CodecsUnsupported {
    const CODE: Code = code("archive.codec-unsupported", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Entry(self.within.clone())
    }

    fn message(&self, f: &mut Message) {
        entries(
            f,
            self.entries.len(),
            " in a codec other than deflate or stored, skipped: ",
        );
        f.list(&self.entries, |f, e| {
            f.spelled(e.entry.raw());
            let _ = write!(f, " (method {})", e.method);
        });
    }
}

/// An entry whose name is no path inside a WAD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnsafeName {
    /// The entry.
    pub entry: Spelled,
    /// Why the name is refused.
    #[serde(serialize_with = "error_text")]
    pub why: PathError,
}

/// Entries of a WAD whose name is no path inside a WAD, skipped. The WAD's own name may be the
/// one refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamesUnsafe {
    /// The WAD the entries are under, as the archive spells it.
    pub wad: Spelled,
    /// The entries, in archive order.
    pub entries: Vec<UnsafeName>,
}

impl Diag for NamesUnsafe {
    const CODE: Code = code("archive.name-unsafe", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Entry(self.wad.clone())
    }

    fn message(&self, f: &mut Message) {
        entries(
            f,
            self.entries.len(),
            " with a name that is no path inside a WAD, skipped: ",
        );
        f.list(&self.entries, |f, e| {
            f.spelled(e.entry.raw())
                .text(" (")
                .spelled(&e.why.to_string())
                .text(")");
        });
    }
}

/// A packed WAD that does not read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnreadableWad {
    /// The packed WAD's entry.
    pub entry: Spelled,
    /// Why it does not read.
    #[serde(flatten)]
    pub why: PackedError,
}

/// Packed forms of a WAD that do not mount or do not read. No chunk of them is taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WadUnreadable {
    /// The WAD, as the archive first spells it.
    pub wad: Spelled,
    /// The packed WAD entries, in archive order.
    pub entries: Vec<UnreadableWad>,
}

impl Diag for WadUnreadable {
    const CODE: Code = code("archive.wad-unreadable", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Entry(self.wad.clone())
    }

    fn message(&self, f: &mut Message) {
        f.count(self.entries.len(), "packed WAD")
            .text(" not reading, skipped: ");
        f.list(&self.entries, |f, e| {
            f.spelled(e.entry.raw())
                .text(" (")
                .spelled(&e.why.to_string())
                .text(")");
        });
    }
}

/// Stored entries with a wrong checksum, read as stored (#MOD-ARTIFACT-0006).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StoredCrcWrong {
    /// The WAD the entries are under, or the entry itself when it is under none.
    pub within: Spelled,
    /// The entries, in archive order.
    pub entries: Vec<Spelled>,
}

impl Diag for StoredCrcWrong {
    const CODE: Code = code("archive.stored-crc-wrong", Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(6));

    fn subject(&self) -> Subject {
        Subject::Entry(self.within.clone())
    }

    fn message(&self, f: &mut Message) {
        entries(
            f,
            self.entries.len(),
            " stored with a wrong checksum, read as stored: ",
        );
        f.list(&self.entries, |f, e| {
            f.spelled(e.raw());
        });
    }
}

/// Chunks with a numeric suffix, skipped (#MOD-ARTIFACT-0008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepackDuplicates {
    /// The WAD, as the archive first spells it.
    pub wad: Spelled,
    /// The entries, in archive order.
    pub entries: Vec<Spelled>,
}

impl Diag for RepackDuplicates {
    const CODE: Code = code("archive.repack-duplicate", Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(8));

    fn subject(&self) -> Subject {
        Subject::Entry(self.wad.clone())
    }

    fn message(&self, f: &mut Message) {
        entries(
            f,
            self.entries.len(),
            " with a numeric suffix a repack kept, skipped: ",
        );
        f.list(&self.entries, |f, e| {
            f.spelled(e.raw());
        });
    }
}

/// A chunk at a path an earlier chunk of the WAD holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DuplicateChunk {
    /// The path's hash.
    #[serde(serialize_with = "hash_hex")]
    pub hash: WadHash,
    /// The entry that holds the chunk: the chunk's own, or the packed WAD's.
    pub entry: Spelled,
}

/// Second chunks at one path of a WAD, skipped. The first stays the chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChunkDuplicates {
    /// The WAD, as the archive first spells it.
    pub wad: Spelled,
    /// The chunks skipped, in the order read.
    pub chunks: Vec<DuplicateChunk>,
}

impl Diag for ChunkDuplicates {
    const CODE: Code = code("archive.chunk-duplicate", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Entry(self.wad.clone())
    }

    fn message(&self, f: &mut Message) {
        f.count(self.chunks.len(), "chunk")
            .text(" at a path an earlier chunk holds, skipped: ");
        f.list(&self.chunks, |f, c| {
            f.path(c.hash);
        });
    }
}

/// `META/info.json` does not parse. The mod has no name, author, version or description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InfoUnreadable {
    /// The entry.
    pub entry: Spelled,
    /// What failed.
    pub reason: String,
}

impl Diag for InfoUnreadable {
    const CODE: Code = code("archive.info-unreadable", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Entry(self.entry.clone())
    }

    fn message(&self, f: &mut Message) {
        f.text("META/info.json does not parse: ")
            .spelled(&self.reason);
    }
}

/// The codes `read` raises. `archive.no-skin` is not among them: what a champion's skin is, is
/// known to `load`.
pub const CODES: &[Code] = &[
    NotZip::CODE,
    TooManyEntries::CODE,
    BudgetExceeded::CODE,
    EntriesTooLarge::CODE,
    EntriesCorrupt::CODE,
    EntriesUnreadable::CODE,
    CodecsUnsupported::CODE,
    NamesUnsafe::CODE,
    WadUnreadable::CODE,
    StoredCrcWrong::CODE,
    RepackDuplicates::CODE,
    ChunkDuplicates::CODE,
    InfoUnreadable::CODE,
];

/// Items of one code by subject, the subjects in the order first seen.
type Grouped<T> = IndexMap<Spelled, Vec<T>>;

/// A finding that ends the read.
#[derive(Debug)]
pub(super) enum Fatal {
    /// The bytes are no zip.
    NotZip(NotZip),
    /// More entries than the cap.
    TooManyEntries(TooManyEntries),
    /// More bytes than a cap.
    BudgetExceeded(BudgetExceeded),
}

impl Fatal {
    /// Raises the finding and gives the error the stage ends with.
    pub(super) fn halt(self, sink: &mut Sink<'_>) -> crate::diag::Halted {
        match self {
            Fatal::NotZip(finding) => sink.halt(finding),
            Fatal::TooManyEntries(finding) => sink.halt(finding),
            Fatal::BudgetExceeded(finding) => sink.halt(finding),
        }
    }
}

/// What the entries were found to be, held until the last entry is read so that each code is
/// raised once per subject with every instance in it.
#[derive(Debug, Default)]
pub(super) struct Findings {
    too_large: Grouped<LargeEntry>,
    corrupt: Grouped<Spelled>,
    unreadable: Grouped<UnreadableEntry>,
    codecs: Grouped<CodecEntry>,
    unsafe_names: Grouped<UnsafeName>,
    wads_unreadable: Grouped<UnreadableWad>,
    stored_crc_wrong: Grouped<Spelled>,
    repack_duplicates: Grouped<Spelled>,
    chunk_duplicates: Grouped<DuplicateChunk>,
    info_unreadable: IndexMap<Spelled, String>,
}

impl Findings {
    pub(super) fn too_large(&mut self, within: &Spelled, entry: &Spelled, declared: u64) {
        self.too_large
            .entry(within.clone())
            .or_default()
            .push(LargeEntry {
                entry: entry.clone(),
                declared,
            });
    }

    pub(super) fn corrupt(&mut self, within: &Spelled, entry: &Spelled) {
        self.corrupt
            .entry(within.clone())
            .or_default()
            .push(entry.clone());
    }

    pub(super) fn unreadable(&mut self, within: &Spelled, entry: &Spelled, reason: String) {
        self.unreadable
            .entry(within.clone())
            .or_default()
            .push(UnreadableEntry {
                entry: entry.clone(),
                reason,
            });
    }

    pub(super) fn codec(&mut self, within: &Spelled, entry: &Spelled, method: u16) {
        self.codecs
            .entry(within.clone())
            .or_default()
            .push(CodecEntry {
                entry: entry.clone(),
                method,
            });
    }

    pub(super) fn unsafe_name(&mut self, wad: &Spelled, entry: &Spelled, why: PathError) {
        self.unsafe_names
            .entry(wad.clone())
            .or_default()
            .push(UnsafeName {
                entry: entry.clone(),
                why,
            });
    }

    pub(super) fn wad_unreadable(&mut self, wad: &Spelled, entry: &Spelled, why: PackedError) {
        self.wads_unreadable
            .entry(wad.clone())
            .or_default()
            .push(UnreadableWad {
                entry: entry.clone(),
                why,
            });
    }

    pub(super) fn stored_crc_wrong(&mut self, within: &Spelled, entry: &Spelled) {
        self.stored_crc_wrong
            .entry(within.clone())
            .or_default()
            .push(entry.clone());
    }

    pub(super) fn repack_duplicate(&mut self, wad: &Spelled, entry: &Spelled) {
        self.repack_duplicates
            .entry(wad.clone())
            .or_default()
            .push(entry.clone());
    }

    pub(super) fn chunk_duplicate(&mut self, wad: &Spelled, entry: &Spelled, hash: WadHash) {
        self.chunk_duplicates
            .entry(wad.clone())
            .or_default()
            .push(DuplicateChunk {
                hash,
                entry: entry.clone(),
            });
    }

    pub(super) fn info_unreadable(&mut self, entry: &Spelled, reason: String) {
        self.info_unreadable.entry(entry.clone()).or_insert(reason);
    }

    /// Raises every finding: the codes in the order of `CODES`, the subjects of a code in the
    /// order the archive lists them.
    pub(super) fn raise(self, caps: &Caps, sink: &mut Sink<'_>) {
        for (within, entries) in self.too_large {
            sink.raise(EntriesTooLarge {
                within,
                cap: caps.entry_bytes,
                entries,
            });
        }
        for (within, entries) in self.corrupt {
            sink.raise(EntriesCorrupt { within, entries });
        }
        for (within, entries) in self.unreadable {
            sink.raise(EntriesUnreadable { within, entries });
        }
        for (within, entries) in self.codecs {
            sink.raise(CodecsUnsupported { within, entries });
        }
        for (wad, entries) in self.unsafe_names {
            sink.raise(NamesUnsafe { wad, entries });
        }
        for (wad, entries) in self.wads_unreadable {
            sink.raise(WadUnreadable { wad, entries });
        }
        for (within, entries) in self.stored_crc_wrong {
            sink.raise(StoredCrcWrong { within, entries });
        }
        for (wad, entries) in self.repack_duplicates {
            sink.raise(RepackDuplicates { wad, entries });
        }
        for (wad, chunks) in self.chunk_duplicates {
            sink.raise(ChunkDuplicates { wad, chunks });
        }
        for (entry, reason) in self.info_unreadable {
            sink.raise(InfoUnreadable { entry, reason });
        }
    }
}
