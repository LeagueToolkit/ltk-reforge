//! The census history read with gix, and the writer of the history index.
//!
//! The census history holds one commit per live build and one file per WAD entry (format 2).
//! This crate walks it, folds every build into the lives of each key, and writes the index
//! `reforge::history` defines and reads (#ADR-0005). The repository is the source. The index
//! is exact, stamped with the commit it came from, and disposable: deleting it loses nothing.

mod error;
#[cfg(feature = "fetch")]
mod fetch;
pub mod format;
mod git;
mod seed;
mod walk;
mod write;

use camino::Utf8Path;
use reforge::history::{Families, Stamp};

pub use self::error::CensusError;
#[cfg(feature = "fetch")]
pub use self::fetch::{Fetcher, Received};
pub use self::git::GitRepo;
pub use self::seed::Seed;
pub use self::walk::{BlobId, Change, ChangeSource, Fold, Folds, Progress, Walked, walk};

/// The published census history. Its builds are on the branch `format::BRANCH`.
pub const PUBLISHED: &str = "https://github.com/LeagueToolkit/ltk-census-history";

/// The writer of an index from one history.
pub struct Indexer<'a> {
    source: &'a dyn ChangeSource,
    repo: String,
    progress: Box<dyn FnMut(Progress<'_>) + 'a>,
}

impl<'a> Indexer<'a> {
    /// An indexer of `source`, which the stamp names `repo`.
    pub fn new(source: &'a dyn ChangeSource, repo: impl Into<String>) -> Self {
        Self {
            source,
            repo: repo.into(),
            progress: Box::new(|_| {}),
        }
    }

    /// Sets what hears each build as it is folded.
    #[must_use]
    pub fn on_progress(mut self, progress: impl FnMut(Progress<'_>) + 'a) -> Self {
        self.progress = Box::new(progress);
        self
    }

    /// Walks the whole history and writes the index of `families` to `out`, replacing what is
    /// there.
    pub fn build(&mut self, out: &Utf8Path, families: Families) -> Result<Stamp, CensusError> {
        let walked = walk(self.source, families, &mut *self.progress)?;
        write::write(walked, out, &self.repo, families)
    }

    /// Adds the builds of the history after the head of the index at `out`. An index that
    /// holds every build is left as it is.
    pub fn append(&mut self, out: &Utf8Path) -> Result<Stamp, CensusError> {
        self.append_to(out, out)
    }

    /// Writes the index at `index` with the builds of the history after its head into `out`,
    /// built beside it and renamed into place, and leaves `index` as it is. An index a process
    /// holds open is taken forward this way: on Windows it is not replaced while open.
    ///
    /// When the index holds every build, nothing is written and the stamp is the index's.
    pub fn append_to(&mut self, index: &Utf8Path, out: &Utf8Path) -> Result<Stamp, CensusError> {
        write::append(self.source, &self.repo, index, out, &mut *self.progress)
    }
}
