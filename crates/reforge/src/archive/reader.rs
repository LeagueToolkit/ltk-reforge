//! The reader: one pass over the zip's entries, in archive order. Each entry is named, opened,
//! inflated under the caps and put where its name says. A packed WAD stored as it is is read
//! where it lies in the archive, never copied whole. What an entry is found to be is held in
//! `Findings` and raised by the caller, also when the read ends early.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Cursor, Read, Seek, Write};

use reforge_magic::HEAD_LEN;
use zip::result::ZipError;
use zip::{CompressionMethod, ZipArchive};

use super::caps::{Budget, Caps};
use super::findings::{BudgetExceeded, Fatal, Findings, NotZip, TooManyEntries};
use super::info::Info;
use super::name::{EntryName, WadPart};
use super::packed::{PackedFailure, PackedWad};
use super::source::{Input, Source, Window};
use super::{ArchiveWad, Chunk};
use crate::diag::{Sink, Step, Steps};
use crate::model::entity::{Extra, MAX_IMAGE_BYTES, ModImage, WadLayout};
use crate::model::{ByteView, ChunkPath, ContentId, Spelled, WadHash, WadName};
use crate::store::{BlobStore, ReadSeek, Spool};

/// The two codecs a run reads. Fewer decoders are fewer for an untrusted archive to reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Codec {
    Stored,
    Deflated,
}

/// What the archive declares of an entry.
#[derive(Debug, Clone, Copy)]
struct Declared {
    method: CompressionMethod,
    size: u64,
}

impl Declared {
    fn codec(self) -> Result<Codec, EntryError> {
        match self.method {
            CompressionMethod::Stored => Ok(Codec::Stored),
            CompressionMethod::Deflated => Ok(Codec::Deflated),
            other => Err(EntryError::Codec(method_number(other))),
        }
    }
}

#[expect(
    deprecated,
    reason = "zip gives a method it does not read as its number no other way"
)]
fn method_number(method: CompressionMethod) -> u16 {
    method.to_u16()
}

/// Why an entry gives no bytes. The archive is read on without it.
#[derive(Debug, thiserror::Error)]
enum EntryError {
    /// A codec other than deflate or stored.
    #[error("zip method {0} is not read")]
    Codec(u16),
    /// More bytes than the cap for one entry.
    #[error("{declared} bytes declared, over the cap for one entry")]
    TooLarge { declared: u64 },
    /// A compressed entry whose bytes fail the checksum.
    #[error("the checksum fails")]
    Corrupt,
    /// The entry does not open or does not read.
    #[error("{0}")]
    Unreadable(String),
}

impl From<ZipError> for EntryError {
    fn from(error: ZipError) -> Self {
        EntryError::Unreadable(error.to_string())
    }
}

/// Why an entry gives no bytes, and whether the read goes on.
#[derive(Debug)]
enum Failure {
    Entry(EntryError),
    OverBudget,
    /// The bytes did not write where they were going: a spool of the store's on disk.
    Out(io::Error),
}

impl From<EntryError> for Failure {
    fn from(error: EntryError) -> Self {
        Failure::Entry(error)
    }
}

/// Where an entry's bytes go as they inflate, and what they are once all have come.
trait Out: Write {
    type Done;

    fn done(self) -> io::Result<Self::Done>;
}

/// The bytes held in memory.
impl Out for Vec<u8> {
    type Done = Vec<u8>;

    fn done(self) -> io::Result<Vec<u8>> {
        Ok(self)
    }
}

/// The bytes counted and dropped.
impl Out for io::Sink {
    type Done = ();

    fn done(self) -> io::Result<()> {
        Ok(())
    }
}

/// The bytes where the store keeps what is too large to hold, to be read in part.
impl Out for Box<dyn Spool> {
    type Done = Box<dyn ReadSeek>;

    fn done(self) -> io::Result<Box<dyn ReadSeek>> {
        self.finish()
    }
}

/// An entry passed through to an output: its bytes inflated into memory or the store's spool,
/// or counted.
struct Passed<T> {
    out: T,
    /// The bytes it inflated to.
    len: u64,
    /// The entry is stored and its recorded checksum is wrong.
    crc_wrong: bool,
}

/// Whether a read failed on the entry's checksum.
fn is_checksum_failure(error: &ZipError) -> bool {
    // `zip` reports a wrong checksum as an `io::Error` of kind `InvalidData`, the kind a damaged
    // deflate stream has too. Its text is the only mark (zip 2.4, `crc32.rs`). The tests of the
    // stored and the compressed case fail when a later version words it otherwise.
    matches!(error, ZipError::Io(e)
        if e.kind() == io::ErrorKind::InvalidData && e.to_string() == "Invalid checksum")
}

/// Why a pass over an entry stopped.
enum Stopped {
    /// The entry did not open or read.
    Zip(ZipError),
    /// Its bytes did not write.
    Out(io::Error),
}

/// Reads to the end into `out`, or gives nothing when more than `cap` bytes come.
fn read_capped(
    reader: &mut impl Read,
    cap: u64,
    out: &mut impl Write,
) -> Result<Option<u64>, Stopped> {
    let mut buf = [0u8; 1 << 14];
    let mut reader = reader.take(cap.saturating_add(1));
    let mut len = 0u64;
    loop {
        let read = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(read) => read,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(Stopped::Zip(e.into())),
        };
        out.write_all(&buf[..read]).map_err(Stopped::Out)?;
        len += read as u64;
    }
    Ok((len <= cap).then_some(len))
}

/// A WAD as its entries come in.
struct OpenWad {
    /// The name as the archive first spells it.
    name: WadName,
    /// None until a chunk is kept or a packed form mounts.
    layout: Option<WadLayout>,
    chunks: BTreeMap<WadHash, Chunk>,
}

impl OpenWad {
    fn subject(&self) -> Spelled {
        Spelled::new(self.name.as_str())
    }

    /// Whether a chunk at the path is held. A later spelling of the path names the chunk that
    /// is held when nothing named it.
    fn holds(&mut self, hash: WadHash, path: Option<&ChunkPath>) -> bool {
        let Some(held) = self.chunks.get_mut(&hash) else {
            return false;
        };
        if held.path.is_none() {
            held.path = path.cloned();
        }
        true
    }
}

/// What the reader gives when the last entry is read, or when the read ends early.
pub(super) struct Parts {
    pub(super) info: Info,
    pub(super) wads: Vec<ArchiveWad>,
    pub(super) extras: Vec<Extra>,
    pub(super) image: Option<ModImage>,
    pub(super) findings: Findings,
}

/// The entry a mod's image is at, as Fantome names it.
const IMAGE_ENTRY: &str = "META/image.png";

/// The pass over the archive's entries.
pub(super) struct Reader<'a> {
    source: Source<'a>,
    zip: ZipArchive<Input<'a>>,
    caps: &'a Caps,
    store: &'a dyn BlobStore,
    budget: Budget,
    info: Info,
    /// By lowercase name: the loose and the packed forms of one WAD are one WAD, in any case.
    wads: BTreeMap<String, OpenWad>,
    extras: Vec<Extra>,
    image: Option<ModImage>,
    findings: Findings,
}

impl<'a> Reader<'a> {
    /// Opens the zip's directory. The archive's bytes are never copied whole.
    pub(super) fn open(
        source: Source<'a>,
        file: &Spelled,
        caps: &'a Caps,
        store: &'a dyn BlobStore,
    ) -> Result<Self, Fatal> {
        if source.len() > caps.archive_bytes {
            return Err(Fatal::BudgetExceeded(BudgetExceeded::ArchiveFile {
                bytes: source.len(),
                cap: caps.archive_bytes,
            }));
        }
        let not_zip = |reason: String| {
            Fatal::NotZip(NotZip {
                file: file.clone(),
                reason,
            })
        };
        let input = source.open().map_err(|e| not_zip(e.to_string()))?;
        let zip = ZipArchive::new(input).map_err(|e| not_zip(e.to_string()))?;
        if zip.len() > caps.entries {
            return Err(Fatal::TooManyEntries(TooManyEntries {
                entries: zip.len(),
                cap: caps.entries,
            }));
        }
        Ok(Self {
            source,
            zip,
            caps,
            store,
            budget: Budget::new(caps.total_bytes),
            info: Info::default(),
            wads: BTreeMap::new(),
            extras: Vec::new(),
            image: None,
            findings: Findings::default(),
        })
    }

    /// Reads every entry, in archive order. Each WAD is a step, begun at its first entry
    /// (#ADR-0097). `read` reads no cancel: what it reads is under the caps.
    pub(super) fn read_entries(&mut self, sink: &mut Sink<'_>) -> Result<(), Fatal> {
        let mut steps = Steps::new(self.wads_named());
        for index in 0..self.zip.len() {
            self.entry(index, &mut steps, sink)?;
        }
        Ok(())
    }

    /// How many WADs the entries name, the loose and the packed forms of one WAD as one.
    fn wads_named(&self) -> usize {
        let names = self.zip.file_names().map(|name| name.replace('\\', "/"));
        let wads = names.filter_map(|name| match EntryName::parse(&name) {
            EntryName::InWad { wad, .. } => Some(wad.lower().to_string()),
            _ => None,
        });
        wads.collect::<BTreeSet<String>>().len()
    }

    /// What was read: the WADs in lowercase name order, each one's chunks in path-hash order.
    pub(super) fn finish(self) -> Parts {
        let wads = self
            .wads
            .into_values()
            .filter_map(|wad| {
                Some(ArchiveWad {
                    name: wad.name,
                    layout: wad.layout?,
                    chunks: wad.chunks.into_values().collect(),
                })
            })
            .collect();
        Parts {
            info: self.info,
            wads,
            extras: self.extras,
            image: self.image,
            findings: self.findings,
        }
    }

    fn entry(&mut self, index: usize, steps: &mut Steps, sink: &mut Sink<'_>) -> Result<(), Fatal> {
        let Some(name) = self.zip.name_for_index(index) else {
            return Ok(());
        };
        // Zips made on Windows by some packers separate with `\`.
        let name = Spelled::new(name.replace('\\', "/"));
        match EntryName::parse(name.raw()) {
            EntryName::Directory => Ok(()),
            EntryName::Extra => {
                self.extra(index, name);
                Ok(())
            }
            EntryName::Info => self.info(index, &name),
            EntryName::UnsafeWad { wad, why } => {
                self.findings.unsafe_name(&wad, &name, why);
                Ok(())
            }
            EntryName::InWad { wad, part } => {
                if !self.wads.contains_key(wad.lower()) {
                    steps.tell(sink, || Step::Wad {
                        wad: wad.to_string(),
                    });
                }
                let key = self.open_wad(wad);
                self.wad_entry(index, &name, &key, part)
            }
        }
    }

    /// The key of a WAD, which is kept from its first mention so that its findings share one
    /// subject.
    fn open_wad(&mut self, name: WadName) -> String {
        let key = name.lower().to_string();
        self.wads.entry(key.clone()).or_insert_with(|| OpenWad {
            name,
            layout: None,
            chunks: BTreeMap::new(),
        });
        key
    }

    fn wad(&mut self, key: &str) -> &mut OpenWad {
        self.wads
            .get_mut(key)
            .unwrap_or_else(|| panic!("the WAD {key:?} was opened before its entry was read"))
    }

    fn wad_entry(
        &mut self,
        index: usize,
        name: &Spelled,
        key: &str,
        part: WadPart,
    ) -> Result<(), Fatal> {
        let subject = self.wad(key).subject();
        match part {
            WadPart::Packed => self.packed(index, name, key, &subject),
            WadPart::Chunk(path) => self.loose(index, name, key, path.hash(), Some(path)),
            WadPart::Hashed(hash) => self.loose(index, name, key, hash, None),
            WadPart::RepackDuplicate => {
                self.findings.repack_duplicate(&subject, name);
                Ok(())
            }
            WadPart::Unsafe(why) => {
                self.findings.unsafe_name(&subject, name, why);
                Ok(())
            }
        }
    }

    /// A chunk as its own entry. The first chunk at a path stays the chunk.
    fn loose(
        &mut self,
        index: usize,
        name: &Spelled,
        key: &str,
        hash: WadHash,
        path: Option<ChunkPath>,
    ) -> Result<(), Fatal> {
        let wad = self.wad(key);
        let subject = wad.subject();
        if wad.holds(hash, path.as_ref()) {
            self.findings.chunk_duplicate(&subject, name, hash);
            return Ok(());
        }
        let Some(bytes) = self.inflate(index, name, &subject)? else {
            return Ok(());
        };
        let content = self.store.put(ByteView::from(bytes));
        let wad = self.wad(key);
        wad.layout.get_or_insert(WadLayout::Loose);
        wad.chunks.insert(
            hash,
            Chunk {
                hash,
                path,
                content,
            },
        );
        Ok(())
    }

    /// Mounts a packed WAD over a reader of its bytes, and puts each chunk into the store as it
    /// is read and before the next is (#ADR-0100): the chunk's id, or none for a chunk at a
    /// path the WAD holds already, which is not put.
    fn mount(
        &mut self,
        bytes: impl Read + Seek,
        len: u64,
        key: &str,
    ) -> Result<Vec<(WadHash, Option<ContentId>)>, PackedFailure> {
        let held: BTreeSet<WadHash> = self.wad(key).chunks.keys().copied().collect();
        let mut seen = BTreeSet::new();
        let store = self.store;
        PackedWad::new(bytes, len, self.caps).chunks(&mut self.budget, |hash, data| {
            let first = !held.contains(&hash) && seen.insert(hash);
            (hash, first.then(|| store.put(ByteView::from(data))))
        })
    }

    /// A whole WAD as one entry. Its chunks join the WAD's loose ones.
    fn packed(
        &mut self,
        index: usize,
        name: &Spelled,
        key: &str,
        subject: &Spelled,
    ) -> Result<(), Fatal> {
        let stored = self
            .declared(index)
            .is_ok_and(|declared| declared.method == CompressionMethod::Stored);
        let read = if stored {
            let Some(window) = self.window(index, name, subject)? else {
                return Ok(());
            };
            let len = window.len();
            self.mount(window, len, key)
        } else {
            let Some(spooled) = self.spooled(index, name, subject)? else {
                return Ok(());
            };
            self.mount(spooled.out, spooled.len, key)
        };
        let chunks = match read {
            Ok(chunks) => chunks,
            Err(PackedFailure::OverBudget(_)) => return Err(self.over_budget(name)),
            Err(PackedFailure::Unreadable(why)) => {
                self.findings.wad_unreadable(subject, name, why);
                return Ok(());
            }
        };
        let mut duplicates = Vec::new();
        let wad = self.wad(key);
        wad.layout = Some(WadLayout::Packed);
        for (hash, content) in chunks {
            match (wad.chunks.entry(hash), content) {
                (Entry::Vacant(place), Some(content)) => {
                    place.insert(Chunk {
                        hash,
                        path: None,
                        content,
                    });
                }
                _ => duplicates.push(hash),
            }
        }
        for hash in duplicates {
            self.findings.chunk_duplicate(subject, name, hash);
        }
        Ok(())
    }

    fn info(&mut self, index: usize, name: &Spelled) -> Result<(), Fatal> {
        let Some(bytes) = self.inflate(index, name, name)? else {
            return Ok(());
        };
        match Info::parse(&bytes) {
            Ok(info) => self.info = info,
            Err(error) => self.findings.info_unreadable(name, error.to_string()),
        }
        Ok(())
    }

    /// An entry outside `META/info.json` and the WADs: its name, size and first bytes are kept,
    /// enough to say what it is. Only the mod's image is read whole (#ADR-0051).
    fn extra(&mut self, index: usize, name: Spelled) {
        let declared = match self.declared(index) {
            Ok(declared) => declared,
            Err(error) => {
                self.findings.unreadable(&name, &name, error.to_string());
                return;
            }
        };
        let mut head = Vec::new();
        if declared.codec().is_ok()
            && let Ok(file) = self.zip.by_index(index)
        {
            // A head that reads short or not at all still leaves the name to tell by.
            let _ = file.take(HEAD_LEN as u64).read_to_end(&mut head);
        }
        let named = name.raw().trim_start_matches('/');
        if self.image.is_none() && named.eq_ignore_ascii_case(IMAGE_ENTRY) {
            self.image = Some(self.image_at(index, declared.size));
        }
        self.extras.push(Extra {
            name,
            size: declared.size,
            head,
        });
    }

    /// The mod's image. An entry that does not read raises nothing here: the build says why
    /// the package has no thumbnail.
    fn image_at(&mut self, index: usize, declared: u64) -> ModImage {
        if declared > MAX_IMAGE_BYTES {
            return ModImage::TooLarge(declared);
        }
        match self.through(index, || Ok(Vec::new())) {
            // The declared size is the archive's word.
            Ok(inflated) if inflated.len > MAX_IMAGE_BYTES => ModImage::TooLarge(inflated.len),
            Ok(inflated) => ModImage::Held(ByteView::from(inflated.out)),
            Err(_) => ModImage::Unreadable,
        }
    }

    /// An entry's bytes, or none when the entry is skipped with a finding.
    fn inflate(
        &mut self,
        index: usize,
        name: &Spelled,
        within: &Spelled,
    ) -> Result<Option<Vec<u8>>, Fatal> {
        let passed = self.through(index, || Ok(Vec::new()));
        Ok(self
            .kept(passed, name, within)?
            .map(|inflated| inflated.out))
    }

    /// A compressed entry inflated once into the store's spool, to be read in part: a scratch
    /// file for a store on disk, removed when the reader is dropped, and memory for a store in
    /// memory (#ADR-0100). None when the entry is skipped with a finding.
    fn spooled(
        &mut self,
        index: usize,
        name: &Spelled,
        within: &Spelled,
    ) -> Result<Option<Passed<Box<dyn ReadSeek>>>, Fatal> {
        let store = self.store;
        let passed = match self.through(index, || store.spool()) {
            Err(Failure::Out(error)) => {
                // A full disk changes no result: the bytes inflate into memory instead.
                tracing::warn!(%error, "an entry does not spool, and inflates in memory");
                self.through(index, || Ok(Vec::new()))
                    .map(|inflated| Passed {
                        out: Box::new(Cursor::new(inflated.out)) as Box<dyn ReadSeek>,
                        len: inflated.len,
                        crc_wrong: inflated.crc_wrong,
                    })
            }
            passed => passed,
        };
        self.kept(passed, name, within)
    }

    /// A stored entry where it lies in the archive, its bytes checked and counted but not
    /// held, or none when the entry is skipped with a finding.
    fn window(
        &mut self,
        index: usize,
        name: &Spelled,
        within: &Spelled,
    ) -> Result<Option<Window<Input<'a>>>, Fatal> {
        let passed = self.through(index, || Ok(io::sink()));
        let Some(passed) = self.kept(passed, name, within)? else {
            return Ok(None);
        };
        let start = self.zip.by_index_raw(index).map(|file| file.data_start());
        let window = start.map_err(|e| e.to_string()).and_then(|start| {
            self.source
                .window(start, passed.len)
                .map_err(|e| e.to_string())
        });
        match window {
            Ok(window) => Ok(Some(window)),
            Err(reason) => {
                self.findings.unreadable(within, name, reason);
                Ok(None)
            }
        }
    }

    /// What passed through, or none when the entry is skipped with a finding.
    fn kept<W>(
        &mut self,
        passed: Result<Passed<W>, Failure>,
        name: &Spelled,
        within: &Spelled,
    ) -> Result<Option<Passed<W>>, Fatal> {
        match passed {
            Ok(passed) => {
                if passed.crc_wrong {
                    self.findings.stored_crc_wrong(within, name);
                }
                Ok(Some(passed))
            }
            Err(Failure::OverBudget) => Err(self.over_budget(name)),
            // Only a spool writes where it can fail, and its caller inflates in memory instead.
            Err(Failure::Out(error)) => {
                self.findings.unreadable(within, name, error.to_string());
                Ok(None)
            }
            Err(Failure::Entry(error)) => {
                match error {
                    EntryError::Codec(method) => self.findings.codec(within, name, method),
                    EntryError::TooLarge { declared } => {
                        self.findings.too_large(within, name, declared);
                    }
                    EntryError::Corrupt => self.findings.corrupt(within, name),
                    EntryError::Unreadable(reason) => {
                        self.findings.unreadable(within, name, reason);
                    }
                }
                Ok(None)
            }
        }
    }

    fn over_budget(&self, name: &Spelled) -> Fatal {
        Fatal::BudgetExceeded(BudgetExceeded::Inflated {
            cap: self.budget.cap(),
            entry: name.clone(),
        })
    }

    fn declared(&mut self, index: usize) -> Result<Declared, EntryError> {
        let file = self.zip.by_index_raw(index)?;
        Ok(Declared {
            method: file.compression(),
            size: file.size(),
        })
    }

    /// The entry inflated into a fresh output under the caps, and charged to the budget.
    fn through<W: Out>(
        &mut self,
        index: usize,
        fresh: impl Fn() -> io::Result<W>,
    ) -> Result<Passed<W::Done>, Failure> {
        let declared = self.declared(index)?;
        let codec = declared.codec()?;
        let too_large = EntryError::TooLarge {
            declared: declared.size,
        };
        if declared.size > self.caps.entry_bytes {
            return Err(too_large.into());
        }
        // The declared size is the archive's word. The read itself stops at the smaller of the
        // entry cap and what is left of the budget.
        let cap = self.caps.entry_bytes.min(self.budget.left());
        let mut out = fresh().map_err(Failure::Out)?;
        let (len, crc_wrong) = match self.checked(index, cap, &mut out) {
            Ok(len) => (len, false),
            // Some packers write a stored entry's checksum wrong. The bytes are what they are,
            // and a WAD that is damaged fails to mount on its own terms.
            Err(Stopped::Zip(e)) if is_checksum_failure(&e) && codec == Codec::Stored => {
                out = fresh().map_err(Failure::Out)?;
                match self.unchecked(index, cap, &mut out) {
                    Ok(len) => (len, true),
                    Err(Stopped::Zip(e)) => return Err(EntryError::from(e).into()),
                    Err(Stopped::Out(e)) => return Err(Failure::Out(e)),
                }
            }
            Err(Stopped::Zip(e)) if is_checksum_failure(&e) => {
                return Err(EntryError::Corrupt.into());
            }
            Err(Stopped::Zip(e)) => return Err(EntryError::from(e).into()),
            Err(Stopped::Out(e)) => return Err(Failure::Out(e)),
        };
        let Some(len) = len else {
            return Err(if cap < self.caps.entry_bytes {
                Failure::OverBudget
            } else {
                too_large.into()
            });
        };
        let out = out.done().map_err(Failure::Out)?;
        self.budget.charge(len).map_err(|_| Failure::OverBudget)?;
        Ok(Passed {
            out,
            len,
            crc_wrong,
        })
    }

    /// The entry inflated and held to its checksum.
    fn checked(
        &mut self,
        index: usize,
        cap: u64,
        out: &mut impl Write,
    ) -> Result<Option<u64>, Stopped> {
        let mut file = self.zip.by_index(index).map_err(Stopped::Zip)?;
        read_capped(&mut file, cap, out)
    }

    /// The entry's bytes as the archive holds them, which for a stored entry are its content.
    fn unchecked(
        &mut self,
        index: usize,
        cap: u64,
        out: &mut impl Write,
    ) -> Result<Option<u64>, Stopped> {
        let mut file = self.zip.by_index_raw(index).map_err(Stopped::Zip)?;
        read_capped(&mut file, cap, out)
    }
}
