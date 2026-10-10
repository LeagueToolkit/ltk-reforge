//! The bytes a run's files name, fetched from the run's store when a stage uses them
//! (#ADR-0100). A file node holds the content id of its bytes, and a stage asks for them where
//! it reads them: nothing above the store keeps them past the stage that asked.
//!
//! The model names the store by what it asks of it: [`BlobAccess`], which every store of the
//! `store` module answers.

use std::fmt;
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

/// The run's store, as a mod and its package hold it.
///
/// A fetch of an id the store lacks gives no bytes and is remembered: the run ends the stage
/// with a fault ([`Blobs::missing`]), so no stage judges a file it could not read.
#[derive(Clone)]
pub struct Blobs {
    access: Arc<dyn BlobAccess>,
    missing: Arc<Mutex<Option<ContentId>>>,
}

impl Blobs {
    /// The store a run's bytes are fetched from and kept in.
    pub fn new(access: Arc<dyn BlobAccess>) -> Self {
        Self {
            access,
            missing: Arc::default(),
        }
    }

    /// The bytes under an id, or none: a caller that gets none reports it.
    pub fn get(&self, id: ContentId) -> Option<ByteView> {
        self.access.fetch(id)
    }

    /// Whether the store holds bytes under an id, which are not read.
    pub fn holds(&self, id: ContentId) -> bool {
        self.access.holds(id)
    }

    /// The bytes under an id. An id the store lacks gives none, and is the one
    /// [`Blobs::missing`] names when it is the first.
    pub fn fetch(&self, id: ContentId) -> ByteView {
        if let Some(bytes) = self.access.fetch(id) {
            return bytes;
        }
        let mut missing = self.missing.lock().unwrap_or_else(PoisonError::into_inner);
        missing.get_or_insert(id);
        ByteView::from(&b""[..])
    }

    /// Keeps the bytes a stage made, under the id it gives.
    pub fn keep(&self, bytes: ByteView) -> ContentId {
        self.access.keep(bytes)
    }

    /// The first id a fetch asked for and the store lacked.
    pub fn missing(&self) -> Option<ContentId> {
        *self.missing.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The run's store, read for what a caller reports missing itself.
impl BlobAccess for Blobs {
    fn fetch(&self, id: ContentId) -> Option<ByteView> {
        self.get(id)
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
