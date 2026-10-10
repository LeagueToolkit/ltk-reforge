//! The bytes a run's files name, fetched from the run's store when a stage uses them
//! (#ADR-0100). A file node holds the content id of its bytes, and a stage asks for them where
//! it reads them: nothing above the store keeps them past the stage that asked.
//!
//! The model names the store by what it asks of it: [`BlobAccess`], which every store of the
//! `store` module answers. What a stage holds of the bytes it fetched is counted while it
//! holds them ([`Fetched`]), and the most it held at once is what the run's working-set cap
//! bounds.
//!
//! A fetch the store does not answer ends the stage at that fetch ([`Missing`]): it unwinds
//! to the run, which ends the stage with a fault, so no stage goes on to judge or write a file
//! it could not read. `docs/design/ARCHITECTURE.md`, "The model", says why it unwinds.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Deref;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use byteview::ByteView;
use reforge_magic::{GENERATION, HEAD_LEN, HeadKind};

use super::hash::ContentId;

/// The first bytes of a blob, and the size of all of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// At most the length asked for, from the start.
    pub bytes: ByteView,
    /// The whole blob's size in bytes.
    pub size: u64,
}

impl Head {
    /// The first `len` bytes of a whole blob, copied so that the rest is not kept.
    pub fn of(bytes: &[u8], len: usize) -> Self {
        Self {
            bytes: ByteView::from(&bytes[..bytes.len().min(len)]),
            size: bytes.len() as u64,
        }
    }
}

/// A run's store as the model reads and writes it.
pub trait BlobAccess: Send + Sync {
    /// The bytes under an id, not checked against it: a run's store holds only what the run
    /// put there or found there whole.
    fn fetch(&self, id: ContentId) -> Option<ByteView>;

    /// The first `len` bytes under an id and the blob's size, read without the rest where
    /// the store can.
    fn fetch_head(&self, id: ContentId, len: usize) -> Option<Head>;

    /// Whether it holds bytes under an id, which are not read.
    fn holds(&self, id: ContentId) -> bool;

    /// Keeps the bytes a stage made, under the id it gives.
    fn keep(&self, bytes: ByteView) -> ContentId;
}

/// A blob's facts as a table keeps them (#ADR-0100): its size, and its kind as text, stamped
/// with the generation of the matcher that read it. A table reads nothing into the kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactRow {
    /// The blob's size in bytes, which no generation changes.
    pub size: u64,
    /// The kind, as its reader wrote it.
    pub kind: String,
    /// The generation of the reader that wrote the kind.
    pub generation: u32,
}

/// The table of facts per blob a run reads and adds to, beside its store (#ADR-0100).
pub trait FactAccess: Send + Sync {
    /// The row kept for an id, or none.
    fn row(&self, id: ContentId) -> Option<FactRow>;

    /// Keeps rows together, each in place of any row of its id. A table that cannot keep
    /// them says so in the log and loses nothing but the rows: they are read again.
    fn keep_rows(&self, rows: &[(ContentId, FactRow)]);
}

/// The bytes fetched and held, and the most held at once since the run last asked.
#[derive(Debug, Default)]
struct Tally {
    held: AtomicU64,
    peak: AtomicU64,
}

impl Tally {
    fn hold(&self, bytes: u64) {
        let held = self.held.fetch_add(bytes, Ordering::Relaxed) + bytes;
        self.peak.fetch_max(held, Ordering::Relaxed);
    }

    fn release(&self, bytes: u64) {
        self.held.fetch_sub(bytes, Ordering::Relaxed);
    }
}

/// Bytes fetched from the run's store, counted as held until dropped.
pub struct Fetched {
    bytes: ByteView,
    tally: Option<Arc<Tally>>,
}

impl Fetched {
    /// Bytes no store gave, which nothing counts: a chunk a stage made and holds.
    pub fn held(bytes: ByteView) -> Self {
        Self { bytes, tally: None }
    }

    /// The bytes as the store's view of them.
    pub fn view(&self) -> &ByteView {
        &self.bytes
    }

    /// The view alone, no longer counted: for bytes a run keeps whole, such as an image.
    pub fn into_view(mut self) -> ByteView {
        if let Some(tally) = self.tally.take() {
            tally.release(self.bytes.len() as u64);
        }
        std::mem::replace(&mut self.bytes, ByteView::from(&b""[..]))
    }
}

impl Deref for Fetched {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.bytes
    }
}

impl AsRef<[u8]> for Fetched {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for Fetched {
    fn drop(&mut self) {
        if let Some(tally) = &self.tally {
            tally.release(self.bytes.len() as u64);
        }
    }
}

impl fmt::Debug for Fetched {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fetched")
            .field("len", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

/// A fetch the run's store did not answer, by the id it asked for (#ADR-0100).
///
/// [`Blobs::fetch`] unwinds with it, and [`Missing::catch`] takes it back where the run ends
/// the stage. It is never a panic: no hook sees it and nothing is printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missing(pub ContentId);

impl Missing {
    /// Ends the work under way at this fetch, up to the nearest [`Missing::catch`].
    fn throw(self) -> ! {
        resume_unwind(Box::new(self))
    }

    /// Runs work that fetches from a run's store: what it gives, or the fetch the store did
    /// not answer, which ended it. Any other unwind goes on as it came.
    pub fn catch<T>(work: impl FnOnce() -> T) -> Result<T, Missing> {
        catch_unwind(AssertUnwindSafe(work)).map_err(|payload| match payload.downcast() {
            Ok(missing) => *missing,
            Err(other) => resume_unwind(other),
        })
    }
}

/// The run's store, as a mod and its package hold it.
///
/// A fetch of an id the store lacks ends the work under way with [`Missing`], and the id is
/// remembered ([`Blobs::missing`]): the run ends the stage with a fault, so no stage judges a
/// file it could not read. Every fetch is counted while its bytes are held
/// ([`Blobs::take_peak`]).
#[derive(Clone)]
pub struct Blobs {
    access: Arc<dyn BlobAccess>,
    missing: Arc<Mutex<Option<ContentId>>>,
    tally: Arc<Tally>,
    known: Arc<Mutex<BTreeMap<ContentId, BlobFacts>>>,
    table: Option<Arc<dyn FactAccess>>,
    learnt: Arc<Mutex<Vec<(ContentId, FactRow)>>>,
}

/// What is known of a blob without its bytes: what its first bytes say of its kind, and its
/// size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlobFacts {
    /// What its first bytes say, before a name is read.
    pub kind: HeadKind,
    /// Its size in bytes.
    pub size: u64,
}

impl Blobs {
    /// The store a run's bytes are fetched from and kept in.
    pub fn new(access: Arc<dyn BlobAccess>) -> Self {
        Self {
            access,
            missing: Arc::default(),
            tally: Arc::default(),
            known: Arc::default(),
            table: None,
            learnt: Arc::default(),
        }
    }

    /// Reads and adds to this table of facts per blob (#ADR-0100).
    #[must_use]
    pub fn with_facts(mut self, table: Arc<dyn FactAccess>) -> Self {
        self.table = Some(table);
        self
    }

    /// What is known of the blob under an id, or none for an id the store lacks: the table's
    /// row where its kind is of this matcher's generation ([`GENERATION`]), else read from the
    /// blob's first [`HEAD_LEN`] bytes, never the whole blob, and kept for the table
    /// ([`Blobs::keep_facts`]). A second ask in a run reads nothing.
    pub fn facts(&self, id: ContentId) -> Option<BlobFacts> {
        let known = || self.known.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(&facts) = known().get(&id) {
            return Some(facts);
        }
        let facts = match self.kept(id) {
            Some(facts) => facts,
            None => {
                let head = self.access.fetch_head(id, HEAD_LEN)?;
                let facts = BlobFacts {
                    kind: HeadKind::of(&head.bytes),
                    size: head.size,
                };
                let row = FactRow {
                    size: facts.size,
                    kind: serde_json::to_string(&facts.kind)
                        .unwrap_or_else(|error| panic!("{:?} does not write: {error}", facts.kind)),
                    generation: GENERATION,
                };
                let mut learnt = self.learnt.lock().unwrap_or_else(PoisonError::into_inner);
                learnt.push((id, row));
                facts
            }
        };
        known().insert(id, facts);
        Some(facts)
    }

    /// The table's facts of a blob, where its kind is of this matcher's generation and reads.
    fn kept(&self, id: ContentId) -> Option<BlobFacts> {
        let row = self.table.as_ref()?.row(id)?;
        let kind = serde_json::from_str(&row.kind).ok();
        let kind = kind.filter(|_| row.generation == GENERATION)?;
        Some(BlobFacts {
            kind,
            size: row.size,
        })
    }

    /// Writes the facts the run read from blobs since the last call into its table, together.
    pub fn keep_facts(&self) {
        let learnt =
            std::mem::take(&mut *self.learnt.lock().unwrap_or_else(PoisonError::into_inner));
        if let (Some(table), false) = (&self.table, learnt.is_empty()) {
            table.keep_rows(&learnt);
        }
    }

    /// What is known of the blob under an id. An id the store lacks ends the work under way
    /// with [`Missing`], as [`Blobs::fetch`] does.
    pub fn facts_of(&self, id: ContentId) -> BlobFacts {
        match self.facts(id) {
            Some(facts) => facts,
            None => self.lacks(id),
        }
    }

    /// The bytes under an id, or none: a caller that gets none reports it.
    pub fn get(&self, id: ContentId) -> Option<Fetched> {
        let bytes = self.access.fetch(id)?;
        self.tally.hold(bytes.len() as u64);
        Some(Fetched {
            bytes,
            tally: Some(Arc::clone(&self.tally)),
        })
    }

    /// Whether the store holds bytes under an id, which are not read.
    pub fn holds(&self, id: ContentId) -> bool {
        self.access.holds(id)
    }

    /// The bytes under an id. An id the store lacks ends the work under way with [`Missing`],
    /// and is the one [`Blobs::missing`] names when it is the first.
    pub fn fetch(&self, id: ContentId) -> Fetched {
        match self.get(id) {
            Some(bytes) => bytes,
            None => self.lacks(id),
        }
    }

    /// Ends the work under way at a fetch of an id the store lacks.
    fn lacks(&self, id: ContentId) -> ! {
        self.missing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert(id);
        Missing(id).throw()
    }

    /// Keeps the bytes a stage made, under the id it gives.
    pub fn keep(&self, bytes: ByteView) -> ContentId {
        self.access.keep(bytes)
    }

    /// The first id a fetch asked for and the store lacked.
    pub fn missing(&self) -> Option<ContentId> {
        *self.missing.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The most bytes of fetches held at once since the last call, which starts the count
    /// again from what is held now.
    pub fn take_peak(&self) -> u64 {
        let held = self.tally.held.load(Ordering::Relaxed);
        self.tally.peak.swap(held, Ordering::Relaxed)
    }
}

/// The run's store, read for what a caller reports missing itself and keeps whole.
impl BlobAccess for Blobs {
    fn fetch(&self, id: ContentId) -> Option<ByteView> {
        self.get(id).map(Fetched::into_view)
    }

    fn fetch_head(&self, id: ContentId, len: usize) -> Option<Head> {
        self.access.fetch_head(id, len)
    }

    fn holds(&self, id: ContentId) -> bool {
        Blobs::holds(self, id)
    }

    fn keep(&self, bytes: ByteView) -> ContentId {
        Blobs::keep(self, bytes)
    }
}

impl fmt::Debug for Blobs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Blobs")
            .field("missing", &self.missing())
            .finish_non_exhaustive()
    }
}
