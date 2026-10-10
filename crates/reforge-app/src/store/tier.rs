//! The run tier (#ADR-0093): what runs make, kept by content id in a `fjall` keyspace, one
//! process writing it. The key is the id's 32 bytes and the value the bytes; values past
//! `fjall`'s separation threshold go to its value log, whose space comes back by its garbage
//! collection after a sweep (#ADR-0096).

use camino::{Utf8Path, Utf8PathBuf};
use fjall::{
    CompressionType, Database, Keyspace, KeyspaceCreateOptions, KvSeparationOptions, Slice,
};
use reforge::model::{ByteView, ContentId};
use reforge::store::BlobStore;

use crate::storage::bytes_of;

/// The keyspace the blobs are kept in.
pub const KEYSPACE: &str = "blobs";

/// The bytes of the empty blob, for its id: answered without a read, as the ingest store
/// answers it.
fn empty_blob(id: ContentId) -> Option<ByteView> {
    (id == ContentId::EMPTY).then(|| ByteView::from(&b""[..]))
}

/// A store of what runs make, in a `fjall` database in a folder of its own.
///
/// A write that fails is logged and still gives the id: a get of it then answers nothing, as
/// for any blob the tier lacks. A clone is a handle on the same database.
#[derive(Clone)]
pub struct RunTier {
    dir: Utf8PathBuf,
    // The last handle on the database to drop persists its journal.
    _db: Database,
    blobs: Keyspace,
}

impl RunTier {
    /// The tier in `dir`, made when absent.
    pub fn open(dir: impl Into<Utf8PathBuf>) -> fjall::Result<Self> {
        let dir = dir.into();
        let db = Database::builder(&dir).open()?;
        // Uncompressed: `lsm-tree` 3.1.10 counts a compressed log file's stale bytes against
        // its uncompressed size, so a file whose every value was removed is never dead and
        // never dropped. Most of a run's bytes are compressed textures and chunks already.
        let separation = KvSeparationOptions::default().compression(CompressionType::None);
        let options = || KeyspaceCreateOptions::default().with_kv_separation(Some(separation));
        let blobs = db.keyspace(KEYSPACE, options)?;
        Ok(Self {
            dir,
            _db: db,
            blobs,
        })
    }

    /// The folder it keeps its database in.
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    /// Every id it holds, in order: the keyspace's keys sort by their bytes, as ids do.
    pub fn ids(&self) -> fjall::Result<Vec<ContentId>> {
        let mut ids = Vec::new();
        for guard in self.blobs.iter() {
            let key = guard.key()?;
            // A key of another length is none of the tier's: it never writes one.
            if let Ok(id) = <[u8; 32]>::try_from(&*key) {
                ids.push(ContentId(id));
            }
        }
        Ok(ids)
    }

    /// Removes the blob under an id. An id it lacks is removed already.
    pub fn remove(&self, id: ContentId) -> fjall::Result<()> {
        self.blobs.remove(id.0)
    }

    /// Its bytes on disk.
    pub fn bytes(&self) -> u64 {
        bytes_of(&self.dir)
    }

    /// Gives back the space of the blobs removed: the memtable flushed so that the removals
    /// reach the tables, then every table compacted, which drops the removed values and the
    /// value log's files no value names. A file dropped is deleted from the disk once no
    /// version of the tree `fjall` keeps names it: at its next flush, or when the tier next
    /// opens.
    ///
    /// `fjall` 3.1 runs the value log's garbage collection inside compaction and has no call
    /// of its own for it; the calls here are its hidden ones, which its tests use.
    pub fn collect(&self) -> fjall::Result<()> {
        self.blobs.rotate_memtable_and_wait()?;
        // The first compaction drops the removed values and counts their log files' stale
        // bytes; a log file is dropped by the compaction after the one that found it dead.
        self.blobs.major_compact()?;
        self.blobs.major_compact()
    }

    fn read(&self, id: ContentId) -> Option<ByteView> {
        match self.blobs.get(id.0) {
            Ok(value) => value.map(ByteView::from),
            Err(error) => {
                tracing::warn!(?id, %error, "a blob of the run tier does not read");
                None
            }
        }
    }
}

impl BlobStore for RunTier {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        if let Some(empty) = empty_blob(id) {
            return Some(empty);
        }
        let bytes = self.read(id)?;
        if ContentId::of(&bytes) == id {
            return Some(bytes);
        }
        tracing::warn!(?id, "a blob of the run tier fails its hash");
        // The next put of the id writes it again.
        if let Err(error) = self.blobs.remove(id.0) {
            tracing::warn!(?id, %error, "a blob of the run tier that fails its hash stays");
        }
        None
    }

    fn get_unchecked(&self, id: ContentId) -> Option<ByteView> {
        empty_blob(id).or_else(|| self.read(id))
    }

    fn contains(&self, id: ContentId) -> bool {
        if id == ContentId::EMPTY {
            return true;
        }
        self.blobs.contains_key(id.0).unwrap_or_else(|error| {
            tracing::warn!(?id, %error, "the run tier does not say whether it holds a blob");
            false
        })
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        if bytes.is_empty() {
            return ContentId::EMPTY;
        }
        let id = ContentId::of(&bytes);
        // The key is the content: two writers of one id write one value.
        if let Err(error) = self.blobs.insert(id.0, Slice::from(bytes)) {
            tracing::warn!(?id, %error, "a blob is not written to the run tier");
        }
        id
    }

    fn on_disk(&self) -> bool {
        true
    }
}
