//! The hash tables downloaded and kept up to date by the app (#ADR-0058): mimir's own updater,
//! over the app's web client, into the hash cache the settings choose, else the one LTK Manager
//! keeps. The cache is shared: its lock keeps two programs from updating it at once, and a
//! reader sees the old tables or the new ones whole.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use camino::Utf8Path;
use ltk_mimir_cache::{
    Fetch, FetchError, HashStore, PlannedTable, Table, UpdateObserver, UpdateOptions, UpdateOutcome,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, chain};
use crate::index::Source;

/// Where the tables are published: the newest release of mimir.
pub const TABLES: &str = "https://github.com/LeagueToolkit/mimir/releases/latest/download";

/// The tables' release, read through a source, stopped when asked.
pub struct Release<'a> {
    source: &'a dyn Source,
    base: String,
    cancel: &'a AtomicBool,
}

impl<'a> Release<'a> {
    /// The release whose files are under `base`, read from `source`.
    pub fn new(source: &'a dyn Source, base: impl Into<String>, cancel: &'a AtomicBool) -> Self {
        Self {
            source,
            base: base.into(),
            cancel,
        }
    }
}

impl Fetch for Release<'_> {
    type Error = AppError;

    fn fetch_to(
        &self,
        filename: &str,
        sink: &mut (dyn Write + Send),
    ) -> Result<u64, FetchError<AppError>> {
        let url = format!("{}/{filename}", self.base);
        // The web client names its failures as the index's.
        let mut reader = self.source.open(&url).map_err(|error| match error {
            AppError::IndexFetch { detail } => {
                FetchError::Transport(AppError::HashesFetch { detail })
            }
            error => FetchError::Transport(error),
        })?;
        let mut buf = vec![0; 64 * 1024];
        let mut total = 0;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(FetchError::Transport(AppError::Cancelled));
            }
            let read = reader.read(&mut buf).map_err(|error| {
                FetchError::Transport(AppError::HashesFetch {
                    detail: format!("{filename}: {error}"),
                })
            })?;
            if read == 0 {
                return Ok(total);
            }
            sink.write_all(&buf[..read]).map_err(FetchError::Sink)?;
            total += read as u64;
        }
    }
}

/// What an update of the tables would download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct HashesCheck {
    /// The tables an update downloads, by name.
    pub behind: Vec<String>,
    /// The bytes it downloads. None when the release lists no size for one of them.
    pub bytes: Option<u64>,
}

/// What an update of the cache at `dir` would download, from the release's list of tables.
pub fn check(dir: &Utf8Path, release: &Release<'_>) -> Result<HashesCheck, AppError> {
    let store = HashStore::at(dir.as_std_path());
    let report = store
        .check(release)
        .map_err(|error| AppError::HashesFetch {
            detail: chain(&error),
        })?;
    let behind = report
        .tables
        .iter()
        .filter(|diff| diff.status.needs_update())
        .map(|diff| diff.table.to_string())
        .collect();
    Ok(HashesCheck {
        behind,
        bytes: report.download_bytes(),
    })
}

/// How far a download of the tables has got, for the window to show.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type, tauri_specta::Event,
)]
pub struct HashesProgress {
    /// The bytes downloaded.
    pub done: u64,
    /// The bytes to download in all. Zero when the release lists no size.
    pub total: u64,
}

/// Hears mimir's updater and sums its tables into one count of bytes.
struct Watch<'a> {
    heard: &'a (dyn Fn(HashesProgress) + Sync),
    total: AtomicU64,
    /// The bytes of the tables downloaded whole.
    before: AtomicU64,
    /// The bytes of the table downloading.
    current: AtomicU64,
}

impl UpdateObserver for Watch<'_> {
    fn planned(&self, tables: &[PlannedTable]) {
        let total = tables.iter().filter_map(|table| table.size_bytes).sum();
        self.total.store(total, Ordering::Relaxed);
    }

    fn progressed(&self, _table: Table, done: u64, _total: Option<u64>) {
        self.current.store(done, Ordering::Relaxed);
        (self.heard)(HashesProgress {
            done: self.before.load(Ordering::Relaxed) + done,
            total: self.total.load(Ordering::Relaxed),
        });
    }

    fn downloaded(&self, _table: Table) {
        let current = self.current.swap(0, Ordering::Relaxed);
        self.before.fetch_add(current, Ordering::Relaxed);
    }
}

/// Downloads the tables the cache at `dir` lacks or holds older, and installs them.
pub fn update(
    dir: &Utf8Path,
    release: &Release<'_>,
    heard: &(dyn Fn(HashesProgress) + Sync),
) -> Result<(), AppError> {
    let store = HashStore::at(dir.as_std_path());
    let watch = Watch {
        heard,
        total: AtomicU64::new(0),
        before: AtomicU64::new(0),
        current: AtomicU64::new(0),
    };
    let options = UpdateOptions::default().observed_by(&watch);
    let outcome = store.update(release, options);
    if release.cancel.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }
    match outcome {
        Ok(UpdateOutcome::Completed(_)) => Ok(()),
        Ok(UpdateOutcome::Locked) => Err(AppError::HashesBusy),
        Err(error) => Err(AppError::HashesFetch {
            detail: chain(&error),
        }),
    }
}
