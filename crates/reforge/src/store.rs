//! The blob store (#ADR-0093): a mod's bytes, kept by content id. Nothing above the store
//! holds bytes: a run and a snapshot name bytes by content id, and a path is a key in a map of
//! the run's, never a location in a store.
//!
//! The bytes a store hands out are a [`ByteView`]: a get is a clone of a view, and a slice of
//! one is a view of the same blob. The library ships the trait, the memory store, the layered
//! store and the ingest store, the tier on disk that holds what `read` makes of an archive;
//! the store of what runs make is the app's.

mod ingest;

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Seek, Write};
use std::sync::{Arc, PoisonError, RwLock};

pub use self::ingest::IngestStore;
use crate::model::{BlobAccess, ByteView, ContentId};

/// Bytes by content id.
pub trait BlobStore: Send + Sync {
    /// The bytes under an id, or nothing.
    fn get(&self, id: ContentId) -> Option<ByteView>;

    /// The bytes under an id, or nothing, not checked against the id where the store would
    /// check them: a run's reads of what it put or found whole (#ADR-0100).
    fn get_unchecked(&self, id: ContentId) -> Option<ByteView> {
        self.get(id)
    }

    /// Whether it holds bytes under an id. A store that can tell without reading them does.
    fn contains(&self, id: ContentId) -> bool {
        self.get(id).is_some()
    }

    /// Keeps bytes under their id, which it gives. Bytes already held are not written twice.
    fn put(&self, bytes: ByteView) -> ContentId;

    /// A place for bytes too large to hold in memory, written in one pass and then read in
    /// part (#ADR-0100). Nothing of them is kept: they go when their reader is dropped. A
    /// store on disk writes them to a scratch file under a temporary name in its directory; a
    /// store in memory holds them in memory.
    fn spool(&self) -> io::Result<Box<dyn Spool>> {
        Ok(Box::new(MemorySpool::default()))
    }
}

/// Bytes read in part, from any offset.
pub trait ReadSeek: Read + Seek + Send {}

impl<T: Read + Seek + Send> ReadSeek for T {}

/// Bytes being written where a store keeps what is too large to hold in memory.
pub trait Spool: Write + Send {
    /// The bytes written, to be read from the start.
    fn finish(self: Box<Self>) -> io::Result<Box<dyn ReadSeek>>;
}

/// A spool in memory, the default of a store that holds no bytes on disk.
#[derive(Debug, Default)]
struct MemorySpool {
    bytes: Vec<u8>,
}

impl Write for MemorySpool {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Spool for MemorySpool {
    fn finish(self: Box<Self>) -> io::Result<Box<dyn ReadSeek>> {
        Ok(Box::new(Cursor::new(self.bytes)))
    }
}

/// A store as the model reads a run's files from it: unchecked.
impl<S: BlobStore + ?Sized> BlobAccess for S {
    fn fetch(&self, id: ContentId) -> Option<ByteView> {
        self.get_unchecked(id)
    }

    fn holds(&self, id: ContentId) -> bool {
        self.contains(id)
    }

    fn keep(&self, bytes: ByteView) -> ContentId {
        self.put(bytes)
    }
}

impl<S: BlobStore + ?Sized> BlobStore for &S {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        (**self).get(id)
    }

    fn get_unchecked(&self, id: ContentId) -> Option<ByteView> {
        (**self).get_unchecked(id)
    }

    fn contains(&self, id: ContentId) -> bool {
        (**self).contains(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        (**self).put(bytes)
    }

    fn spool(&self) -> io::Result<Box<dyn Spool>> {
        (**self).spool()
    }
}

impl<S: BlobStore + ?Sized> BlobStore for Arc<S> {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        (**self).get(id)
    }

    fn get_unchecked(&self, id: ContentId) -> Option<ByteView> {
        (**self).get_unchecked(id)
    }

    fn contains(&self, id: ContentId) -> bool {
        (**self).contains(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        (**self).put(bytes)
    }

    fn spool(&self) -> io::Result<Box<dyn Spool>> {
        (**self).spool()
    }
}

impl<S: BlobStore + ?Sized> BlobStore for Box<S> {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        (**self).get(id)
    }

    fn get_unchecked(&self, id: ContentId) -> Option<ByteView> {
        (**self).get_unchecked(id)
    }

    fn contains(&self, id: ContentId) -> bool {
        (**self).contains(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        (**self).put(bytes)
    }

    fn spool(&self) -> io::Result<Box<dyn Spool>> {
        (**self).spool()
    }
}

/// A store in memory: the command's, the tests' and the corpus'.
#[derive(Debug, Default)]
pub struct MemoryStore {
    blobs: RwLock<BTreeMap<ContentId, ByteView>>,
}

impl MemoryStore {
    /// A store with nothing in it.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many blobs it holds.
    pub fn len(&self) -> usize {
        self.blobs
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether it holds nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The ids it holds, in order.
    pub fn ids(&self) -> Vec<ContentId> {
        self.blobs
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .copied()
            .collect()
    }

    /// Every blob, in id order: what moves to a store under this one.
    pub fn into_blobs(self) -> BTreeMap<ContentId, ByteView> {
        self.blobs
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl BlobStore for MemoryStore {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        self.blobs
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&id)
            .cloned()
    }

    fn contains(&self, id: ContentId) -> bool {
        self.blobs
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains_key(&id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        let id = ContentId::of(&bytes);
        self.blobs
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(id)
            .or_insert(bytes);
        id
    }
}

/// A store over another: a get is answered from its own blobs and falls through to the store
/// under it, and a put goes into its own. A run writes into a memory layer of its own over a
/// store, so a stage that ends early leaves nothing under it.
#[derive(Debug)]
pub struct Layered<T, U> {
    top: T,
    under: U,
}

impl<T: BlobStore, U: BlobStore> Layered<T, U> {
    /// A layer `top` over `under`.
    pub fn new(top: T, under: U) -> Self {
        Self { top, under }
    }

    /// The layer's own store.
    pub fn top(&self) -> &T {
        &self.top
    }

    /// The store under the layer.
    pub fn under(&self) -> &U {
        &self.under
    }

    /// Takes the layer apart: its own store, and the one under it.
    pub fn into_parts(self) -> (T, U) {
        (self.top, self.under)
    }
}

impl<T: BlobStore, U: BlobStore> BlobStore for Layered<T, U> {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        self.top.get(id).or_else(|| self.under.get(id))
    }

    fn get_unchecked(&self, id: ContentId) -> Option<ByteView> {
        let top = self.top.get_unchecked(id);
        top.or_else(|| self.under.get_unchecked(id))
    }

    fn contains(&self, id: ContentId) -> bool {
        self.top.contains(id) || self.under.contains(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        self.top.put(bytes)
    }

    fn spool(&self) -> io::Result<Box<dyn Spool>> {
        self.top.spool()
    }
}
