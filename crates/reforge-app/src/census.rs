//! The history index taken forward on the user's machine: by the release's delta when one
//! fits the release's index on disk (#ADR-0109), else from the census history, fetched into
//! the app's clone as far as an append reads it, the builds after the index's head appended to
//! a copy (#ADR-0047, #ADR-0057). Either way the new index goes in a folder of its own, and the
//! index the runs read is never written: the settings move to the new folder, and the old one
//! is removed at the next start.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::{SqliteHistory, Stamp};
use reforge_census::{CensusError, DeltaStep, Fetcher, Indexer, Received, apply_delta};
use serde::{Deserialize, Serialize};

use crate::download::Download;
use crate::error::{AppError, chain};
use crate::index::{
    IndexRelease, InstallProgress, Source, delta_head, folder_name, head_folder, path_safe,
};

/// The census history's server.
pub const CENSUS: &str = reforge_census::PUBLISHED;

/// How far an update of the index has got, for the window to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UpdateStep {
    /// The census history is fetching.
    Fetch {
        /// The objects asked for.
        objects: usize,
        /// The bytes read.
        bytes: u64,
    },
    /// The builds are appending.
    Append {
        /// The builds the index holds so far.
        done: usize,
        /// The builds it holds once the append ends.
        of: usize,
        /// The build appended last.
        build: String,
    },
    /// The release's delta is downloading (#ADR-0109).
    Download {
        /// The bytes downloaded.
        done: u64,
        /// The bytes to download.
        total: u64,
    },
    /// The index is being written from the delta and the release's index.
    Write {
        /// The bytes written.
        done: u64,
        /// The bytes to write.
        total: u64,
    },
    /// The index written is being checked against the delta's digest.
    Check,
}

/// What a release's delta did for the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeltaUpdate {
    /// The index taken forward from the release's index, in a new folder under the root.
    Updated(Utf8PathBuf),
    /// The index is at the delta's head.
    UpToDate,
    /// No delta applies: the release holds none, or its index is not under the root.
    NoDelta,
}

/// Whether the release's delta reaches past the head of the index at `index`. None when no
/// delta applies, and the census history is to be asked instead.
pub fn delta_behind(release: &IndexRelease, root: &Utf8Path, index: &Utf8Path) -> Option<bool> {
    let delta = release.delta.as_ref()?;
    let head = delta_head(&delta.name)?;
    let base = root.join(folder_name(&release.tag));
    Stamp::read(&base).ok()?;
    let stamp = Stamp::read(index).ok()?;
    Some(!stamp.head.starts_with(head))
}

/// Takes the index at `index` forward with the release's delta (#ADR-0109): the delta
/// downloaded into `downloads` and applied to the release's index under `root`, into a folder
/// of the new head. The index at `index` is left as it is.
pub fn update_from_release(
    source: &dyn Source,
    release: &IndexRelease,
    root: &Utf8Path,
    downloads: &Utf8Path,
    index: &Utf8Path,
    cancel: &AtomicBool,
    heard: &dyn Fn(UpdateStep),
) -> Result<DeltaUpdate, AppError> {
    let Some(behind) = delta_behind(release, root, index) else {
        return Ok(DeltaUpdate::NoDelta);
    };
    if !behind {
        return Ok(DeltaUpdate::UpToDate);
    }
    let delta = release.delta.as_ref().ok_or(AppError::NoIndexRelease)?;
    let head = delta_head(&delta.name).ok_or(AppError::NoIndexRelease)?;
    let base = root.join(folder_name(&release.tag));
    let file = downloads.join(format!("{}-{}", path_safe(&release.tag), delta.name));
    let mut progress = |seen: InstallProgress| {
        heard(UpdateStep::Download {
            done: seen.done,
            total: seen.total,
        });
    };
    Download::new(source, cancel, &mut progress, |detail| {
        AppError::IndexFetch { detail }
    })
    .fetch(delta, &file)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }

    let out = head_folder(root, head);
    let mut step = |step: DeltaStep| {
        heard(match step {
            DeltaStep::Write { done, total } => UpdateStep::Write { done, total },
            DeltaStep::Check => UpdateStep::Check,
        });
    };
    apply_delta(&file, &base, &out, &mut step).map_err(|error| AppError::IndexAppend {
        detail: chain(&error),
    })?;
    let _ = fs::remove_file(&file);
    SqliteHistory::open(&out).map_err(|error| AppError::IndexUnreadable {
        detail: chain(&error),
    })?;
    Ok(DeltaUpdate::Updated(out))
}

/// An update of the index, as it goes.
#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type, tauri_specta::Event,
)]
pub struct IndexUpdateProgress {
    /// The step under way.
    pub step: UpdateStep,
}

/// Takes indexes forward from the census history.
pub struct Updater<'a> {
    url: String,
    census: &'a Utf8Path,
    root: &'a Utf8Path,
    cancel: &'a AtomicBool,
    heard: Arc<dyn Fn(UpdateStep) + Send + Sync>,
}

impl<'a> Updater<'a> {
    /// An updater that fetches from `url` into the clone at `census`, writes each new index in a
    /// folder under `root`, and stops fetching when `cancel` is set.
    pub fn new(
        url: impl Into<String>,
        census: &'a Utf8Path,
        root: &'a Utf8Path,
        cancel: &'a AtomicBool,
    ) -> Self {
        Self {
            url: url.into(),
            census,
            root,
            cancel,
            heard: Arc::new(|_| {}),
        }
    }

    /// Sets what hears each step as it goes, from the fetch's threads too.
    #[must_use]
    pub fn on_progress(mut self, heard: impl Fn(UpdateStep) + Send + Sync + 'static) -> Self {
        self.heard = Arc::new(heard);
        self
    }

    fn fetcher(&self) -> Fetcher<'a> {
        let heard = Arc::clone(&self.heard);
        Fetcher::new(self.census, &self.url, self.cancel).on_progress(move |got: Received| {
            heard(UpdateStep::Fetch {
                objects: got.objects,
                bytes: got.bytes,
            });
        })
    }

    /// Whether the census history holds builds after the head of the index at `index`.
    pub fn behind(&self, index: &Utf8Path) -> Result<bool, AppError> {
        let stamp = stamp(index)?;
        let tip = self.fetcher().tip().map_err(fetch_error)?;
        Ok(tip != stamp.head)
    }

    /// The index at `index` with the builds after its head, in a new folder under the root.
    /// None when the census history holds no build after it.
    pub fn update(&self, index: &Utf8Path) -> Result<Option<Utf8PathBuf>, AppError> {
        let stamp = stamp(index)?;
        let mut fetcher = self.fetcher();
        let tip = fetcher.tip().map_err(fetch_error)?;
        if tip == stamp.head {
            return Ok(None);
        }
        let repo = fetcher
            .fetch(&stamp.head, stamp.families)
            .map_err(fetch_error)?;
        if self.cancel.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }

        let out = head_folder(self.root, &tip);
        let heard = &self.heard;
        Indexer::new(&repo, CENSUS)
            .on_progress(|p| {
                heard(UpdateStep::Append {
                    done: p.done,
                    of: p.of,
                    build: p.build.build.to_string(),
                });
            })
            .append_to(index, &out)
            .map_err(append_error)?;
        SqliteHistory::open(&out).map_err(|error| AppError::IndexUnreadable {
            detail: chain(&error),
        })?;
        Ok(Some(out))
    }
}

fn stamp(index: &Utf8Path) -> Result<Stamp, AppError> {
    Stamp::read(index).map_err(|error| AppError::IndexUnreadable {
        detail: chain(&error),
    })
}

fn fetch_error(error: CensusError) -> AppError {
    match error {
        CensusError::Cancelled => AppError::Cancelled,
        CensusError::NoCommit(head) => AppError::IndexForeign { head },
        error => AppError::CensusFetch {
            detail: chain(&error),
        },
    }
}

fn append_error(error: CensusError) -> AppError {
    match error {
        CensusError::Diverged { commit, .. } => AppError::IndexForeign { head: commit },
        error => AppError::IndexAppend {
            detail: chain(&error),
        },
    }
}
