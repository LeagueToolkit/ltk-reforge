//! The blob store (#ADR-0093): a mod's bytes, kept by content id. Nothing above the store
//! holds bytes: a run and a snapshot name bytes by content id, and a path is a key in a map of
//! the run's, never a location in a store.
//!
//! The bytes a store hands out are a [`ByteView`]: a get is a clone of a view, and a slice of
//! one is a view of the same blob. The library ships the trait, the memory store and the
//! layered store; a store on disk is its owner's.

use std::collections::BTreeMap;
use std::sync::{Arc, PoisonError, RwLock};

pub use byteview::ByteView;

use crate::model::ContentId;

/// Bytes by content id.
pub trait BlobStore: Send + Sync {
    /// The bytes under an id, or nothing.
    fn get(&self, id: ContentId) -> Option<ByteView>;

    /// Keeps bytes under their id, which it gives. Bytes already held are not written twice.
    fn put(&self, bytes: ByteView) -> ContentId;
}

impl<S: BlobStore + ?Sized> BlobStore for &S {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        (**self).get(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        (**self).put(bytes)
    }
}

impl<S: BlobStore + ?Sized> BlobStore for Arc<S> {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        (**self).get(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        (**self).put(bytes)
    }
}

impl<S: BlobStore + ?Sized> BlobStore for Box<S> {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        (**self).get(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        (**self).put(bytes)
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

    fn put(&self, bytes: ByteView) -> ContentId {
        self.top.put(bytes)
    }
}
