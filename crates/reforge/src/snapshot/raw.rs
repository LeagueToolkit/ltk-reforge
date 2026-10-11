//! The raw snapshot as a cache (#ADR-0095): the snapshots `read` gave, by the archive's
//! SHA-256, with their bytes in an ingest store (#ADR-0093). A run that finds its archive's
//! snapshot there takes it in place of `read`, and one that does not reads the archive into
//! the ingest store and leaves its snapshot there.
//!
//! The cache is a directory: the ingest store's blobs, its table of facts per blob, and one
//! entry per archive,
//! a file named by the archive's SHA-256, the reading version and the caps, which holds the raw
//! snapshot's JSON. An entry is written to a temporary file and renamed, so it is whole or
//! absent, and any number of processes read and write the cache at once. An entry that does
//! not read, is of another reading or caps, or names a blob the store lacks is a miss.
//!
//! The cache evicts nothing: it is cleared by archive (#ADR-0093), each entry of the archive
//! with the blobs no other entry names and their facts.
//!
//! A run takes the cache beside the store it reads through (`Run::in_cache`), not a porter: the
//! cache is a store, no setting, and the settings are what a snapshot names (#ADR-0094). Which
//! cache a run reads changes nothing in its report.

use std::collections::BTreeSet;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;

use super::{from_json, to_json};
use crate::archive::{Caps, RAW_VERSION, RawSnapshot};
use crate::model::ContentId;
use crate::store::{BlobStore, FactTable, IngestStore};

/// The directory of the entries, beside the ingest store's own.
const ENTRIES: &str = "raw";

/// The directory an entry is written in before it is renamed into place: `cacache`'s own, on
/// the same volume as the entries.
const TEMPORARY: &str = "tmp";

/// An entry of the cache: an archive's raw snapshot, by what its file's name says.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawEntry {
    /// The archive's SHA-256.
    pub sha256: ContentId,
    /// The raw reading's version it was read at.
    pub version: u32,
    /// The first sixteen hex digits of the SHA-256 of the caps it was read under, as JSON.
    pub caps_hash: String,
}

impl RawEntry {
    /// The entry a file's name gives: `<sha256>-<version>-<caps16>.json`, as [`RawCache::entry`]
    /// names it. None for a name of another shape.
    fn named(name: &str) -> Option<Self> {
        let stem = name.strip_suffix(".json")?;
        let mut parts = stem.split('-');
        let (sha256, version, caps) = (parts.next()?, parts.next()?, parts.next()?);
        let caps_hex = caps.len() == 16 && caps.bytes().all(|b| b.is_ascii_hexdigit());
        if parts.next().is_some() || !caps_hex {
            return None;
        }
        let entry = Self {
            sha256: ContentId::parse(sha256)?,
            version: version.parse().ok()?,
            caps_hash: caps.to_string(),
        };
        // A name spelled otherwise than `RawCache::entry` spells it is no entry's: the entry
        // is found again by its name alone.
        (entry.file_name() == name).then_some(entry)
    }

    /// The name of its file in the entries' directory.
    fn file_name(&self) -> String {
        format!(
            "{}-{}-{}.json",
            self.sha256.hex(),
            self.version,
            self.caps_hash
        )
    }
}

/// What a clear of the cache removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cleared {
    /// The entries removed.
    pub entries: usize,
    /// The blobs removed: those no entry left names.
    pub blobs: usize,
    /// The bytes of those blobs.
    pub bytes: u64,
}

/// The raw snapshots of archives, with their bytes in an ingest store.
#[derive(Debug, Clone)]
pub struct RawCache {
    dir: Utf8PathBuf,
    store: IngestStore,
    facts: Arc<FactTable>,
}

impl RawCache {
    /// The cache in a directory, made at the first write when absent.
    pub fn new(dir: impl Into<Utf8PathBuf>) -> Self {
        let dir = dir.into();
        let store = IngestStore::new(dir.clone());
        let facts = Arc::new(FactTable::in_dir(&dir));
        Self { dir, store, facts }
    }

    /// The table of facts per blob of the ingest store, in the same directory (#ADR-0100).
    pub fn facts(&self) -> Arc<FactTable> {
        Arc::clone(&self.facts)
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
        let entry = RawEntry {
            sha256,
            version,
            caps_hash: ContentId::of(&caps).hex()[..16].to_string(),
        };
        self.dir.join(ENTRIES).join(entry.file_name())
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

    /// Every entry the cache holds, sorted by archive, reading and caps, from the names of
    /// their files alone. A file in the entries' directory whose name is no entry's is not
    /// listed.
    pub fn entries(&self) -> Vec<RawEntry> {
        let files = self.entry_files().into_iter();
        let mut entries: Vec<RawEntry> = files.map(|(entry, _)| entry).collect();
        entries.sort();
        entries
    }

    /// The bytes of the blobs an entry names that the store holds, each blob once: none for
    /// an entry that does not read. Each blob's size is asked of the store, so a caller asks
    /// for the entries whose bytes it shows alone.
    pub fn entry_bytes(&self, entry: &RawEntry) -> u64 {
        let path = self.dir.join(ENTRIES).join(entry.file_name());
        self.bytes_of(&self.named(&path))
    }

    /// Removes every entry of the archive with this SHA-256, at any reading and caps, and the
    /// blobs they name that no other entry names, with their facts (#ADR-0093, #ADR-0096).
    pub fn clear(&self, sha256: ContentId) -> std::io::Result<Cleared> {
        self.clear_where(|entry| entry == sha256)
    }

    /// Removes every entry of an archive whose SHA-256 is not in `keep`, and the blobs they
    /// name that no kept entry names, with their facts.
    pub fn clear_unnamed(&self, keep: &[ContentId]) -> std::io::Result<Cleared> {
        self.clear_where(|entry| !keep.contains(&entry))
    }

    /// Removes the entries whose archive `cleared` picks, then the blobs they name that no
    /// entry left names. Every entry left is read first, as the mark: an entry that does not
    /// read names nothing, as a get takes it for a miss.
    fn clear_where(&self, cleared: impl Fn(ContentId) -> bool) -> std::io::Result<Cleared> {
        let (gone, left): (Vec<_>, Vec<_>) = self
            .entry_files()
            .into_iter()
            .partition(|(name, _)| cleared(name.sha256));
        let marked: BTreeSet<ContentId> =
            left.iter().flat_map(|(_, path)| self.named(path)).collect();
        let mut named = BTreeSet::new();
        for (_, path) in &gone {
            named.extend(self.named(path));
            fs::remove_file(path)?;
        }
        let unmarked: Vec<ContentId> = named.difference(&marked).copied().collect();
        let bytes = self.bytes_of(&unmarked);
        for &id in &unmarked {
            self.store.remove(id)?;
        }
        self.facts.remove(&unmarked);
        Ok(Cleared {
            entries: gone.len(),
            blobs: unmarked.len(),
            bytes,
        })
    }

    /// Each entry file, by what its name says, with its path.
    fn entry_files(&self) -> Vec<(RawEntry, Utf8PathBuf)> {
        let Ok(files) = fs::read_dir(self.dir.join(ENTRIES)) else {
            return Vec::new();
        };
        let named = files.flatten().filter_map(|file| {
            let path = Utf8PathBuf::from_path_buf(file.path()).ok()?;
            let entry = RawEntry::named(path.file_name()?)?;
            Some((entry, path))
        });
        named.collect()
    }

    /// The ids an entry's snapshot names, each once. None for an entry that does not read.
    fn named(&self, entry: &Utf8Path) -> BTreeSet<ContentId> {
        let json = fs::read_to_string(entry).ok();
        let raw = json.and_then(|json| from_json::<RawSnapshot>(&json).ok());
        raw.map(|raw| raw.ids().collect()).unwrap_or_default()
    }

    /// The bytes of these blobs in the store: none for one it lacks.
    fn bytes_of<'i>(&self, ids: impl IntoIterator<Item = &'i ContentId>) -> u64 {
        let size = |&id| self.store.get_head(id, 0).map_or(0, |head| head.size);
        ids.into_iter().map(size).sum()
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
