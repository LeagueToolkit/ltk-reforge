//! The raw snapshot as a cache (#ADR-0095): the snapshots `read` gave, by the archive's
//! SHA-256, with their bytes in an ingest store (#ADR-0093). A run that finds its archive's
//! snapshot there takes it in place of `read`, and one that does not reads the archive into
//! the ingest store and leaves its snapshot there.
//!
//! The cache is a directory: the ingest store's blobs, and beside them one entry per archive,
//! a file named by the archive's SHA-256, the reading version and the caps, which holds the raw
//! snapshot's JSON. An entry is written to a temporary file and renamed, so it is whole or
//! absent, and any number of processes read and write the cache at once. An entry that does
//! not read, is of another reading or caps, or names a blob the store lacks is a miss.
//!
//! A run takes the cache beside the store it reads through (`Run::in_cache`), not a porter: the
//! cache is a store, no setting, and the settings are what a snapshot names (#ADR-0094). Which
//! cache a run reads changes nothing in its report.

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;

use super::{from_json, to_json};
use crate::archive::{Caps, RAW_VERSION, RawSnapshot};
use crate::model::ContentId;
use crate::store::IngestStore;

/// The directory of the entries, beside the ingest store's own.
const ENTRIES: &str = "raw";

/// The directory an entry is written in before it is renamed into place: `cacache`'s own, on
/// the same volume as the entries.
const TEMPORARY: &str = "tmp";

/// The raw snapshots of archives, with their bytes in an ingest store.
#[derive(Debug, Clone)]
pub struct RawCache {
    dir: Utf8PathBuf,
    store: IngestStore,
}

impl RawCache {
    /// The cache in a directory, made at the first write when absent.
    pub fn new(dir: impl Into<Utf8PathBuf>) -> Self {
        let dir = dir.into();
        let store = IngestStore::new(dir.clone());
        Self { dir, store }
    }

    /// The directory it keeps its entries and blobs in.
    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    /// The ingest store the snapshots' bytes are in.
    pub fn store(&self) -> &IngestStore {
        &self.store
    }

    /// The file of the entry of an archive read under these caps at this reading.
    pub fn entry(&self, sha256: ContentId, version: u32, caps: &Caps) -> Utf8PathBuf {
        let caps = serde_json::to_vec(caps)
            .unwrap_or_else(|error| panic!("the caps {caps:?} do not write: {error}"));
        let caps = &ContentId::of(&caps).hex()[..16];
        let name = format!("{}-{version}-{caps}.json", sha256.hex());
        self.dir.join(ENTRIES).join(name)
    }

    /// The raw snapshot of the archive with this SHA-256, read under these caps at this
    /// program's reading, when the cache holds it with every blob it names.
    pub fn get(&self, sha256: ContentId, caps: &Caps) -> Option<RawSnapshot> {
        let entry = self.entry(sha256, RAW_VERSION, caps);
        let json = fs::read_to_string(entry).ok()?;
        let raw = from_json::<RawSnapshot>(&json).ok()?;
        let fits = raw.version == RAW_VERSION && raw.caps == *caps && raw.meta.sha256 == sha256;
        (fits && raw.ids().all(|id| self.store.contains(id))).then_some(raw)
    }

    /// Keeps a raw snapshot whose bytes are in the store, under its archive, reading and caps.
    pub fn put(&self, raw: &RawSnapshot) -> std::io::Result<()> {
        let entry = self.entry(raw.meta.sha256, raw.version, &raw.caps);
        let (entries, temporary) = (self.dir.join(ENTRIES), self.dir.join(TEMPORARY));
        fs::create_dir_all(&entries)?;
        fs::create_dir_all(&temporary)?;
        let mut file = tempfile::NamedTempFile::new_in(&temporary)?;
        std::io::Write::write_all(&mut file, to_json(raw).as_bytes())?;
        match file.persist(&entry) {
            Ok(_) => Ok(()),
            // Another process renamed the same entry into place first, and holds it open.
            Err(_) if entry.exists() => Ok(()),
            Err(error) => Err(error.error),
        }
    }
}
