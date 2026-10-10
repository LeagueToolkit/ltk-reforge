//! The ingest tier (#ADR-0093): blobs in a directory, one file per blob, which any number of
//! processes read and write at once.
//!
//! `cacache` keeps the files. A put writes a temporary file and renames it to the blob's hash,
//! so a blob is whole or absent; a get checks the bytes against the hash before it answers.
//! The store uses it by hash alone, with SHA-256 as the algorithm, so a blob's address is its
//! content id. `cacache`'s index, a key to a hash, is never written.

use cacache::{Algorithm, Integrity};
use camino::{Utf8Path, Utf8PathBuf};

use super::BlobStore;
use crate::model::{ByteView, ContentId};

/// A store of blobs in a directory, by content id.
///
/// A put that does not write (a full disk, a directory that does not open) is logged and still
/// gives the id: a get of it then answers nothing, as for any blob the store lacks. A blob whose
/// bytes no longer match its hash answers nothing and is removed, so the next put of it writes
/// it again.
#[derive(Debug, Clone)]
pub struct IngestStore {
    dir: Utf8PathBuf,
}

impl IngestStore {
    /// The store in a directory, made at the first put when absent.
    pub fn new(dir: impl Into<Utf8PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The directory it keeps its blobs in.
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    /// Whether it holds a blob under an id. The bytes are not read, so not checked.
    pub fn contains(&self, id: ContentId) -> bool {
        cacache::exists_sync(&self.dir, &integrity(id))
    }
}

/// The address `cacache` keeps a blob of this id at.
fn integrity(id: ContentId) -> Integrity {
    Integrity::from_hex(id.hex(), Algorithm::Sha256)
        .unwrap_or_else(|error| panic!("the content id {id:?} is no SHA-256 to cacache: {error}"))
}

impl BlobStore for IngestStore {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        let address = integrity(id);
        match cacache::read_hash_sync(&self.dir, &address) {
            Ok(bytes) => Some(ByteView::from(bytes)),
            Err(cacache::Error::IntegrityError(error)) => {
                tracing::warn!(?id, %error, "a blob of the ingest store fails its hash");
                // Another process may hold the file open; the next put writes it again.
                let _ = cacache::remove_hash_sync(&self.dir, &address);
                None
            }
            Err(_) => None,
        }
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        let id = ContentId::of(&bytes);
        if self.contains(id) {
            return id;
        }
        match cacache::write_hash_sync_with_algo(Algorithm::Sha256, &self.dir, &bytes) {
            Ok(written) => assert!(
                written.matches(&integrity(id)).is_some(),
                "cacache keeps the blob {id:?} at {written}"
            ),
            Err(error) => tracing::warn!(?id, %error, "a blob is not written to the ingest store"),
        }
        id
    }
}
