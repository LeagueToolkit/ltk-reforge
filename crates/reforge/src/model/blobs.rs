//! The bytes a run's files name, fetched from the run's store when a stage uses them
//! (#ADR-0100). A file node holds the content id of its bytes, and a stage asks for them where
//! it reads them: nothing above the store keeps them past the stage that asked.
//!
//! The model names the store by what it asks of it: [`BlobAccess`], which every store of the
//! `store` module answers. What a stage holds of the bytes it fetched is counted while it
//! holds them ([`Fetched`]), and the most it held at once is what the run's working-set cap
//! bounds.

use std::fmt;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use byteview::ByteView;

use super::hash::ContentId;

/// A run's store as the model reads and writes it.
pub trait BlobAccess: Send + Sync {
    /// The bytes under an id, not checked against it: a run's store holds only what the run
    /// put there or found there whole.
    fn fetch(&self, id: ContentId) -> Option<ByteView>;

    /// Whether it holds bytes under an id, which are not read.
    fn holds(&self, id: ContentId) -> bool;

    /// Keeps the bytes a stage made, under the id it gives.
    fn keep(&self, bytes: ByteView) -> ContentId;
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

/// The run's store, as a mod and its package hold it.
///
/// A fetch of an id the store lacks gives no bytes and is remembered: the run ends the stage
/// with a fault ([`Blobs::missing`]), so no stage judges a file it could not read. Every fetch
/// is counted while its bytes are held ([`Blobs::take_peak`]).
#[derive(Clone)]
pub struct Blobs {
    access: Arc<dyn BlobAccess>,
    missing: Arc<Mutex<Option<ContentId>>>,
    tally: Arc<Tally>,
}

impl Blobs {
    /// The store a run's bytes are fetched from and kept in.
    pub fn new(access: Arc<dyn BlobAccess>) -> Self {
        Self {
            access,
            missing: Arc::default(),
            tally: Arc::default(),
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

    /// The bytes under an id. An id the store lacks gives none, and is the one
    /// [`Blobs::missing`] names when it is the first.
    pub fn fetch(&self, id: ContentId) -> Fetched {
        if let Some(bytes) = self.get(id) {
            return bytes;
        }
        let mut missing = self.missing.lock().unwrap_or_else(PoisonError::into_inner);
        missing.get_or_insert(id);
        Fetched::held(ByteView::from(&b""[..]))
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
