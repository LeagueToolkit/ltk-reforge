//! What the app holds while it runs, beside its settings: the inputs opened for the settings
//! as they stand, the downloads under way, the store of runs, the run page's last run and the
//! last mod explored.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use parking_lot::Mutex;
use reforge::diag::Cancel;

use crate::archive::Clones;
use crate::error::AppError;
use crate::explore::ExploredMod;
use crate::inputs::{Choice, Inputs};
use crate::run::HeldRun;
use crate::storage::Storage;
use crate::store::{AppStore, StoreOpen, StoreState, remove_rows};

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
        if self.running.swap(true, Ordering::AcqRel) {
            // A start refused leaves the stop asked of the work under way in place.
            return false;
        }
        self.cancel.store(false, Ordering::Relaxed);
        true
    }

    /// Marks the work ended.
    pub fn end(&self) {
        self.running.store(false, Ordering::Release);
    }

    /// Whether work is under way.
    pub fn running(&self) -> bool {
        self.running.load(Ordering::Acquire)
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

/// The schema database's download: one at a time, since each writes the shared folder.
#[derive(Default)]
pub struct SchemaJob {
    /// The download under way.
    pub run: Job,
}

/// The run of a mod, or a batch of them: one at a time, since the window follows one run's
/// events.
#[derive(Default)]
pub struct RunJob {
    /// The run under way.
    pub run: Job,
    /// The cancel of the run under way. A cancel once set stays set, so each start makes one.
    cancel: Mutex<Cancel>,
    /// The package the last run wrote, for the window to show in its folder without naming a
    /// path (`docs/design/ROADMAP-UX.md`, "The inputs page").
    written: Mutex<Option<Utf8PathBuf>>,
}

impl RunJob {
    /// Marks a run started, unless one is under way, and answers the cancel it reads.
    pub fn start(&self) -> Option<Cancel> {
        if !self.run.start() {
            return None;
        }
        let cancel = Cancel::new();
        *self.cancel.lock() = cancel.clone();
        Some(cancel)
    }

    /// Sets the cancel of the run under way (#ADR-0097).
    pub fn cancel(&self) {
        self.cancel.lock().cancel();
    }

    /// Records what a run that ended wrote: its package, or nothing.
    pub fn set_written(&self, path: Option<Utf8PathBuf>) {
        *self.written.lock() = path;
    }

    /// The package the last run wrote, while one did.
    pub fn written(&self) -> Option<Utf8PathBuf> {
        self.written.lock().clone()
    }
}

/// The last mod explored (#ADR-0091), replaced by the next.
#[derive(Default)]
pub struct Explored(Mutex<Option<ExploredMod>>);

impl Explored {
    /// Holds a mod explored in place of the one held.
    pub fn set(&self, explored: ExploredMod) {
        *self.0.lock() = Some(explored);
    }

    /// What `read` gives of the mod held, or of none.
    pub fn with<T>(&self, read: impl FnOnce(Option<&ExploredMod>) -> T) -> T {
        read(self.0.lock().as_ref())
    }
}

/// The run page's last run, replaced by the next. A command reading it takes its own handle,
/// so a slow read holds no lock while the next run replaces it.
#[derive(Default)]
pub struct LastRun(Mutex<Option<Arc<HeldRun>>>);

impl LastRun {
    /// Holds a run that ended in place of the one held.
    pub fn set(&self, held: HeldRun) {
        *self.0.lock() = Some(Arc::new(held));
    }

    /// The run held, if any.
    pub fn get(&self) -> Option<Arc<HeldRun>> {
        self.0.lock().clone()
    }
}

/// The app's store of runs (#ADR-0096), opened at start, or why it did not open. A store that
/// does not open, such as one another instance of the app holds or one another build made,
/// leaves the runs unkept and the app running.
pub struct RunStore {
    dir: Utf8PathBuf,
    /// Held through a reset, so no caller sees the store half made.
    held: Mutex<Result<Arc<AppStore>, StoreOpen>>,
}

impl RunStore {
    /// The store in `dir`.
    pub fn open(dir: &Utf8Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            held: Mutex::new(opened(dir)),
        }
    }

    /// Removes the rows' file and opens the store anew, whatever its state: outdated, open or
    /// failed. The open sweeps every blob of the run tier, which no row names then; the ingest
    /// tier stays, since an archive's data is still good. Answers the state after.
    pub fn reset(&self) -> StoreState {
        let mut held = self.held.lock();
        // The store held is let go first: its rows' file and its tier close with the last
        // handle, and a file held open is not removed. No caller sees the stand-in, since the
        // lock is held until the store is open again.
        *held = Err(StoreOpen::Failed(AppError::StoreOutdated));
        let reset = remove_rows(&self.dir).map_err(StoreOpen::Failed);
        *held = reset.and_then(|()| opened(&self.dir));
        state_of(&held)
    }

    /// The store, or why it did not open.
    pub fn get(&self) -> Result<Arc<AppStore>, AppError> {
        match &*self.held.lock() {
            Ok(store) => Ok(Arc::clone(store)),
            Err(StoreOpen::Outdated(_)) => Err(AppError::StoreOutdated),
            Err(StoreOpen::Failed(error)) => Err(error.clone()),
        }
    }

    /// Whether the store is open, and what a reset loses when it is not.
    pub fn state(&self) -> StoreState {
        state_of(&self.held.lock())
    }
}

fn state_of(held: &Result<Arc<AppStore>, StoreOpen>) -> StoreState {
    match held {
        Ok(_) => StoreState::Open,
        Err(error) => StoreState::of(error),
    }
}

/// The store in `dir`, or why it did not open, logged.
fn opened(dir: &Utf8Path) -> Result<Arc<AppStore>, StoreOpen> {
    let opened = AppStore::open(dir).map(Arc::new);
    match &opened {
        Err(StoreOpen::Failed(error)) => {
            tracing::warn!(%dir, %error, "the store of runs does not open");
        }
        Err(StoreOpen::Outdated(outdated)) => {
            tracing::warn!(%dir, ?outdated, "the store of runs is another build's");
        }
        Ok(_) => {}
    }
    opened
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_start_refused_keeps_the_stop_asked_of_the_work_under_way() {
        let job = Job::new();
        assert!(job.start());
        job.cancel.store(true, Ordering::Relaxed);
        assert!(!job.start());
        assert!(job.cancel.load(Ordering::Relaxed));
        job.end();
        assert!(job.start());
        assert!(!job.cancel.load(Ordering::Relaxed));
    }
}
