//! The game's paths by name hash (FNV-1a, as an entry's), derived from the hash cache's path
//! table and kept on disk: a process that asks reads a few records instead of hashing 2.3M
//! paths (2.2 s a process, 2026-10-01). Builds before 16.17 named an asset by its path's name
//! hash (`VfxAssetRemap.oldAsset`).
//!
//! The file is a pure function of the table, so it is built where it is used and never shipped.
//! Its stamp is the table's file name, which the cache changes with every version: a file of
//! another stamp is built again. It is written under a temporary name and renamed into place.
//! The format and its magic are ltk-forge's: a file that program built is read as it is.
//!
//! | part | bytes |
//! |---|---|
//! | magic `FORGENHX`, format u32, stamp length u32, stamp | 16 + stamp |
//! | record count u64 | 8 |
//! | records sorted by name hash: name hash u32, path hash u64 (`u64::MAX`: several paths) | 12 each |

use std::cmp::Ordering;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::sync::atomic::{self, AtomicU32};
use std::sync::{Mutex, PoisonError};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use ltk_hashdb::HashDb;

use crate::model::{BinHash, WadHash, name_hash};

const MAGIC: &[u8; 8] = b"FORGENHX";
const FORMAT: u32 = 1;
const HEAD: usize = 16;
const RECORD: usize = 12;
/// The path hash written for a name hash several paths share.
const SEVERAL: u64 = u64::MAX;
const FILE: &str = "name-hash.idx";

/// Why the name-hash index can be neither read nor written.
#[derive(Debug, thiserror::Error)]
pub enum NameHashError {
    /// The file or its directory does not read or write.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// The file just written does not read back as the index of its table.
    #[error("{path} was written and does not read back")]
    NotReadBack {
        /// The index file.
        path: Utf8PathBuf,
    },
}

/// What a name hash names among the paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameHashSlot {
    /// One path, by its path hash.
    One(WadHash),
    /// Several paths share the name hash, so it names none.
    Several,
}

impl NameHashSlot {
    /// What the name hash names when two sources each say what it names.
    pub fn merge(self, other: Self) -> Self {
        if self == other {
            self
        } else {
            NameHashSlot::Several
        }
    }
}

/// The index file, open. A lookup reads about twenty records of it.
#[derive(Debug)]
pub struct NameHashIndex {
    file: Mutex<fs::File>,
    /// Where the records start.
    base: u64,
    count: u64,
}

impl NameHashIndex {
    /// The index under `dir` for the table stamped `stamp`, built from `table` when the
    /// directory has none or one of another stamp.
    pub fn open_or_build(
        dir: &Utf8Path,
        stamp: &str,
        table: &HashDb,
    ) -> Result<Self, NameHashError> {
        // Two builds in one process, each to a file of its own.
        static BUILDS: AtomicU32 = AtomicU32::new(0);

        let path = dir.join(FILE);
        if let Some(index) = Self::open(&path, stamp)? {
            return Ok(index);
        }
        let _span = tracing::info_span!("name-hash-index", %path, stamp).entered();
        fs::create_dir_all(dir)?;
        let temporary = dir.join(format!(
            "{FILE}.tmp-{}-{}",
            std::process::id(),
            BUILDS.fetch_add(1, atomic::Ordering::Relaxed)
        ));
        Self::write(&temporary, stamp, table)?;
        if let Err(error) = fs::rename(&temporary, &path) {
            // Another process may have put one in place meanwhile and hold it open. It is the
            // same file, so that one is read.
            fs::remove_file(&temporary)?;
            return Self::open(&path, stamp)?.ok_or(NameHashError::Io(error));
        }
        Self::open(&path, stamp)?.ok_or(NameHashError::NotReadBack { path })
    }

    /// The index at `path`, when it is one of this format and stamp. A file that is cut short
    /// or of another table is no error: it is built again.
    fn open(path: &Utf8Path, stamp: &str) -> Result<Option<Self>, NameHashError> {
        let mut file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let length = file.metadata()?.len();
        let base = (HEAD + stamp.len() + 8) as u64;
        if length < base {
            return Ok(None);
        }
        let mut head = [0u8; HEAD];
        let mut theirs = vec![0u8; stamp.len()];
        let mut count = [0u8; 8];
        file.read_exact(&mut head)?;
        file.read_exact(&mut theirs)?;
        file.read_exact(&mut count)?;
        let same = head[..8] == *MAGIC
            && head[8..12] == FORMAT.to_le_bytes()
            && head[12..] == (stamp.len() as u32).to_le_bytes()
            && theirs == stamp.as_bytes();
        if !same {
            return Ok(None);
        }
        let count = u64::from_le_bytes(count);
        let whole = count
            .checked_mul(RECORD as u64)
            .and_then(|records| records.checked_add(base));
        Ok((whole == Some(length)).then(|| Self {
            file: Mutex::new(file),
            base,
            count,
        }))
    }

    /// What a name hash names among the table's paths. `None` when no path has it.
    pub fn get(&self, hash: BinHash) -> Result<Option<NameHashSlot>, NameHashError> {
        // Every read seeks first, so a file a panic left anywhere is as good as any.
        let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut low, mut high) = (0u64, self.count);
        let mut record = [0u8; RECORD];
        while low < high {
            let middle = low + (high - low) / 2;
            file.seek(SeekFrom::Start(self.base + middle * RECORD as u64))?;
            file.read_exact(&mut record)?;
            let (key, path) = Self::decode(&record);
            match key.cmp(&hash.0) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    return Ok(Some(match path {
                        SEVERAL => NameHashSlot::Several,
                        one => NameHashSlot::One(WadHash(one)),
                    }));
                }
            }
        }
        Ok(None)
    }

    fn decode(record: &[u8; RECORD]) -> (u32, u64) {
        let mut key = [0u8; 4];
        let mut path = [0u8; 8];
        key.copy_from_slice(&record[..4]);
        path.copy_from_slice(&record[4..]);
        (u32::from_le_bytes(key), u64::from_le_bytes(path))
    }

    fn write(path: &Utf8Path, stamp: &str, table: &HashDb) -> Result<(), NameHashError> {
        let mut records: Vec<(u32, u64)> = table
            .iter()
            .map(|(hash, name)| (name_hash(name.as_str()).0, hash))
            .collect();
        records.sort_unstable();
        records.dedup();
        let mut merged: Vec<(u32, u64)> = Vec::with_capacity(records.len());
        for (key, hash) in records {
            match merged.last_mut() {
                Some(last) if last.0 == key => last.1 = SEVERAL,
                _ => merged.push((key, hash)),
            }
        }
        let mut out = BufWriter::new(fs::File::create(path)?);
        out.write_all(MAGIC)?;
        out.write_all(&FORMAT.to_le_bytes())?;
        out.write_all(&(stamp.len() as u32).to_le_bytes())?;
        out.write_all(stamp.as_bytes())?;
        out.write_all(&(merged.len() as u64).to_le_bytes())?;
        for (key, hash) in merged {
            out.write_all(&key.to_le_bytes())?;
            out.write_all(&hash.to_le_bytes())?;
        }
        out.flush()?;
        Ok(())
    }
}
