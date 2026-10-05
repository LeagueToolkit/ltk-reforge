//! What the app holds while it runs, beside its settings: the inputs opened for the settings
//! as they stand, and the downloads under way.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::Utf8PathBuf;
use parking_lot::Mutex;

use crate::archive::Clones;
use crate::error::AppError;
use crate::inputs::{Choice, Inputs};
use crate::storage::Storage;

/// Work the window starts, one at a time, and can stop.
pub struct Job {
    /// Set to stop the work under way.
    pub cancel: AtomicBool,
    running: AtomicBool,
}

impl Job {
    /// No work under way.
    pub fn new() -> Self {
        Self {
            cancel: AtomicBool::new(false),
            running: AtomicBool::new(false),
        }
    }

    /// Marks the work started, unless some is under way.
    pub fn start(&self) -> bool {
        self.cancel.store(false, Ordering::Relaxed);
        !self.running.swap(true, Ordering::AcqRel)
    }

    /// Marks the work ended.
    pub fn end(&self) {
        self.running.store(false, Ordering::Release);
    }
}

impl Default for Job {
    fn default() -> Self {
        Self::new()
    }
}

/// The history index's download, its update from the census history, and the census history's
/// bootstrap: one at a time, since each writes the clone or the indexes.
pub struct IndexJob {
    /// The folder each index is installed in, one folder per index.
    pub root: Utf8PathBuf,
    /// The app's clones of the census history.
    pub clones: Clones,
    /// The folder a release's files download into, kept there while a download stops.
    pub downloads: Utf8PathBuf,
    /// The work under way.
    pub run: Job,
}

impl IndexJob {
    /// No work under way.
    pub fn new(root: Utf8PathBuf, clones: Clones, downloads: Utf8PathBuf) -> Self {
        Self {
            root,
            clones,
            downloads,
            run: Job::new(),
        }
    }

    /// The folders the job writes, to measure and to remove.
    pub fn storage(&self) -> Storage<'_> {
        Storage {
            indexes: &self.root,
            clones: &self.clones,
            downloads: &self.downloads,
        }
    }
}

/// The hash tables' download.
#[derive(Default)]
pub struct HashesJob {
    /// The download under way.
    pub run: Job,
}

/// The inputs opened for one choice, opened again when the choice changes.
pub struct OpenInputs {
    names_index: Utf8PathBuf,
    current: Mutex<Option<(Choice, Arc<Inputs>)>>,
}

impl OpenInputs {
    /// Nothing opened yet. The name-hash index is kept in `names_index`.
    pub fn new(names_index: Utf8PathBuf) -> Self {
        Self {
            names_index,
            current: Mutex::new(None),
        }
    }

    /// The inputs a choice names: the ones open when the choice is the same, else opened now.
    ///
    /// The lock is held while they open, so two calls never open the same install twice.
    pub fn get(&self, choice: &Choice) -> Result<Arc<Inputs>, AppError> {
        let mut current = self.current.lock();
        if let Some((open, inputs)) = &*current
            && open == choice
        {
            return Ok(Arc::clone(inputs));
        }
        *current = None;
        let inputs = Arc::new(Inputs::open(choice, &self.names_index)?);
        *current = Some((choice.clone(), Arc::clone(&inputs)));
        Ok(inputs)
    }

    /// Lets go of the inputs open, so the next call opens them again: their files changed under
    /// the same choice.
    pub fn forget(&self) {
        *self.current.lock() = None;
    }
}
