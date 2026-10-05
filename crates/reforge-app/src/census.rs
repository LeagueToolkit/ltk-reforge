//! The history index taken forward on the user's machine (#ADR-0047, #ADR-0057): the census
//! history fetched into the app's clone as far as an append reads it, and the builds after the
//! index's head appended to a copy of the index in a folder of its own. The index the runs read
//! is never written: the settings move to the new folder, and the old one is removed at the next
//! start.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use reforge::history::{SqliteHistory, Stamp};
use reforge_census::{CensusError, Fetcher, Indexer, Received};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, chain};
use crate::index::folder_name;

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

        let short = tip.get(..12).unwrap_or(&tip);
        let out = self.root.join(folder_name(&format!("head-{short}")));
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
