//! What the app keeps on disk for the history: the indexes it installed, its clones of the
//! census history and the downloads a stop kept, measured for the inputs page and removed when
//! the user asks (#ADR-0061). Nothing outside the app's own folders is measured as the app's or
//! removed.

use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::Stamp;
use serde::Serialize;

use crate::archive::Clones;
use crate::error::AppError;

/// An index the app installed that the settings do not choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct KeptIndex {
    /// Its folder.
    pub dir: String,
    /// The newest build it holds.
    pub newest_build: String,
    /// How many builds it holds.
    pub builds: usize,
    /// Its bytes on disk.
    pub bytes: u64,
}

/// What the app keeps on disk for the history, as the inputs page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct StorageView {
    /// The bytes of the index the settings choose. Zero when they choose none.
    pub index_bytes: u64,
    /// Whether the index the settings choose is one the app installed.
    pub index_owned: bool,
    /// The index with the most builds among those the app installed and the settings do not
    /// choose.
    pub kept: Option<KeptIndex>,
    /// The bytes of every index the app installed, the one the settings choose among them: what
    /// their removal frees.
    pub indexes_bytes: u64,
    /// The bytes of the app's clones of the census history.
    pub census_bytes: u64,
    /// The bytes of the downloads kept to resume.
    pub downloads_bytes: u64,
}

/// The app's folders for the history.
pub struct Storage<'a> {
    /// The folder each index is installed in, one folder per index.
    pub indexes: &'a Utf8Path,
    /// The app's clones of the census history.
    pub clones: &'a Clones,
    /// The folder a release's files download into.
    pub downloads: &'a Utf8Path,
}

impl Storage<'_> {
    /// What the folders hold, with `chosen` the index the settings choose.
    pub fn view(&self, chosen: Option<&Utf8Path>) -> StorageView {
        let installed = folders(self.indexes);
        let indexes_bytes = installed.iter().map(|dir| bytes_of(dir)).sum();
        let kept = installed
            .into_iter()
            .filter(|dir| Some(dir.as_path()) != chosen)
            // An install that stopped: its stamp is written before its database.
            .filter(|dir| dir.extension() != Some("partial"))
            .filter_map(|dir| Some((Stamp::read(&dir).ok()?, dir)))
            .max_by(|(a, x), (b, y)| a.builds.cmp(&b.builds).then_with(|| x.cmp(y)))
            .map(|(stamp, dir)| KeptIndex {
                bytes: bytes_of(&dir),
                dir: dir.to_string(),
                newest_build: stamp.newest_build.to_string(),
                builds: stamp.builds,
            });
        StorageView {
            index_bytes: chosen.map_or(0, bytes_of),
            index_owned: chosen.is_some_and(|dir| self.owns(dir)),
            kept,
            indexes_bytes,
            census_bytes: self.clone_dirs().iter().map(|dir| bytes_of(dir)).sum(),
            downloads_bytes: bytes_of(self.downloads),
        }
    }

    /// Whether the index at `dir` is one the app installed.
    pub fn owns(&self, dir: &Utf8Path) -> bool {
        dir.starts_with(self.indexes)
    }

    /// Removes every index the app installed.
    pub fn remove_indexes(&self) -> Result<(), AppError> {
        folders(self.indexes).iter().try_for_each(|dir| remove(dir))
    }

    /// Removes the app's clones of the census history, and what a bootstrap that stopped left
    /// beside them.
    pub fn remove_clones(&self) -> Result<(), AppError> {
        self.clone_dirs().iter().try_for_each(|dir| remove(dir))
    }

    /// Removes the downloads kept to resume.
    pub fn remove_downloads(&self) -> Result<(), AppError> {
        remove(self.downloads)
    }

    fn clone_dirs(&self) -> [Utf8PathBuf; 4] {
        let whole = &self.clones.whole;
        [
            whole.clone(),
            self.clones.partial.clone(),
            Utf8PathBuf::from(format!("{whole}.new")),
            Utf8PathBuf::from(format!("{whole}.old")),
        ]
    }
}

/// The folders in `root`, in the order of their names.
fn folders(root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<Utf8PathBuf> = entries
        .flatten()
        .filter_map(|entry| Utf8PathBuf::from_path_buf(entry.path()).ok())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// The bytes of the files under `path`. A link is counted as itself and not followed, and what
/// does not read counts as nothing.
fn bytes_of(path: &Utf8Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| Utf8PathBuf::from_path_buf(entry.path()).ok())
        .map(|child| bytes_of(&child))
        .sum()
}

/// Removes the folder at `dir` and all it holds. One that is absent is removed already.
fn remove(dir: &Utf8Path) -> Result<(), AppError> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::NotRemoved {
            path: dir.to_string(),
            detail: error.to_string(),
        }),
    }
}
