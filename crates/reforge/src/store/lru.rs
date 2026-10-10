//! A store that holds what it read from the store under it, up to a number of bytes, and
//! drops the blob read least recently first (#ADR-0100). A run puts one between its memory
//! layer and a store on disk, so that a blob two stages read is read from disk once: `load`
//! parses a bin that `fix` writes again and `build` reads back, and `facts` and `build` read
//! the same meshes and textures.
//!
//! A run gives it a quarter of the working-set cap ([`SHARE_OF_WORKING_SET`]): 256 MiB at the
//! default cap of 1 GiB. The working-set cap bounds what a stage holds of its fetches at once,
//! and what this store holds comes on top, so a run holds at most a quarter more than the cap
//! of the mod's bytes. A quarter holds the median mod's raw bytes, 28 MB, several times over,
//! and the largest file a stage held over the reduced corpus, 157 MB, so a file one stage read
//! is still held when the next reads it, while the app keeps the bound it runs under on a
//! machine of 8 GB. A store in memory gets none: its bytes are in memory already.
//!
//! The store knows ids and bytes alone. A blob larger than the bound passes through and is
//! not held. A put goes to the store under it, which is where a blob is kept.

use std::collections::BTreeMap;
use std::io;
use std::sync::{Mutex, PoisonError};

use super::{BlobStore, Spool, empty_blob};
use crate::model::{ByteView, ContentId, Head};

/// The divisor of the working-set cap that gives a run's LRU its bound: a quarter.
pub const SHARE_OF_WORKING_SET: u64 = 4;

/// One blob held: its bytes, whether they were checked against their id, and when it was last
/// read.
#[derive(Debug)]
struct Held {
    bytes: ByteView,
    checked: bool,
    read_at: u64,
}

/// What the store holds, in the order of the last read.
#[derive(Debug, Default)]
struct Blobs {
    held: BTreeMap<ContentId, Held>,
    /// The ids by when each was last read, the least recent first.
    order: BTreeMap<u64, ContentId>,
    bytes: u64,
    clock: u64,
}

impl Blobs {
    /// The bytes held under an id, now the most recently read. A read checked takes only
    /// bytes that were checked.
    fn read(&mut self, id: ContentId, checked: bool) -> Option<ByteView> {
        let held = self
            .held
            .get_mut(&id)
            .filter(|held| held.checked || !checked)?;
        self.order.remove(&held.read_at);
        self.clock += 1;
        held.read_at = self.clock;
        self.order.insert(self.clock, id);
        Some(held.bytes.clone())
    }

    /// Holds bytes read from the store under, dropping the least recently read until they
    /// fit. Bytes larger than the bound are not held.
    fn hold(&mut self, id: ContentId, bytes: &ByteView, checked: bool, bound: u64) {
        let size = bytes.len() as u64;
        if size > bound {
            return;
        }
        if let Some(old) = self.held.remove(&id) {
            self.order.remove(&old.read_at);
            self.bytes -= old.bytes.len() as u64;
        }
        while self.bytes + size > bound {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            if let Some(dropped) = self.held.remove(&oldest) {
                self.bytes -= dropped.bytes.len() as u64;
            }
        }
        self.clock += 1;
        self.order.insert(self.clock, id);
        self.bytes += size;
        let held = Held {
            bytes: bytes.clone(),
            checked,
            read_at: self.clock,
        };
        self.held.insert(id, held);
    }
}

/// A store over another that holds what it read from it, up to a bound in bytes, the least
/// recently read dropped first.
#[derive(Debug)]
pub struct Lru<S> {
    under: S,
    bound: u64,
    blobs: Mutex<Blobs>,
}

impl<S: BlobStore> Lru<S> {
    /// A store over `under` that holds at most `bound` bytes of what it read.
    pub fn new(under: S, bound: u64) -> Self {
        Self {
            under,
            bound,
            blobs: Mutex::default(),
        }
    }

    /// The most bytes it holds.
    pub fn bound(&self) -> u64 {
        self.bound
    }

    /// The bytes it holds.
    pub fn held(&self) -> u64 {
        self.lock().bytes
    }

    /// Whether it holds an id's bytes.
    pub fn holds(&self, id: ContentId) -> bool {
        self.lock().held.contains_key(&id)
    }

    /// The store under it.
    pub fn under(&self) -> &S {
        &self.under
    }

    /// The store under it, what it held dropped.
    pub fn into_under(self) -> S {
        self.under
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Blobs> {
        self.blobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A read through it: what it holds, else what `read` gets from under, then held.
    fn read(
        &self,
        id: ContentId,
        checked: bool,
        read: impl FnOnce() -> Option<ByteView>,
    ) -> Option<ByteView> {
        if let Some(empty) = empty_blob(id) {
            return Some(empty);
        }
        if let Some(bytes) = self.lock().read(id, checked) {
            return Some(bytes);
        }
        let bytes = read()?;
        self.lock().hold(id, &bytes, checked, self.bound);
        Some(bytes)
    }
}

impl<S: BlobStore> BlobStore for Lru<S> {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        self.read(id, true, || self.under.get(id))
    }

    fn get_unchecked(&self, id: ContentId) -> Option<ByteView> {
        self.read(id, false, || self.under.get_unchecked(id))
    }

    fn get_head(&self, id: ContentId, len: usize) -> Option<Head> {
        let held = self.lock().read(id, false);
        match held {
            Some(bytes) => Some(Head::of(&bytes, len)),
            None => self.under.get_head(id, len),
        }
    }

    fn contains(&self, id: ContentId) -> bool {
        id == ContentId::EMPTY || self.holds(id) || self.under.contains(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        self.under.put(bytes)
    }

    fn spool(&self) -> io::Result<Box<dyn Spool>> {
        self.under.spool()
    }

    fn on_disk(&self) -> bool {
        self.under.on_disk()
    }
}
