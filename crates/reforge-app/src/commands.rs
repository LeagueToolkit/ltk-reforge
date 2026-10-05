//! The commands the front end calls. Each is thin: it takes the app's state and answers an
//! [`IpcResult`], and the work is in the module it calls. Work that reads the disk or a mod
//! runs through [`off_thread`].

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use parking_lot::Mutex;
use reforge::game::{meta_cache_dir, mimir_cache_dir};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use crate::catalog::{self, CheckEntry, FixupEntry};
use crate::census::{CENSUS, IndexUpdateProgress, UpdateStep, Updater};
use crate::error::{AppError, IpcResult};
use crate::hashes::{self, HashesCheck, HashesProgress, Release, TABLES};
use crate::index::{IndexReleaseView, InstallProgress, Installer, Web};
use crate::inputs::{self, Choice, InputsView};
use crate::run::{self, PartView, RunKind, RunRequest, RunView, StageEvent, StageName, Watch};
use crate::runlog::LogLine;
use crate::schema::{self, SchemaCheck, Wiki};
use crate::selection::FixupChoice;
use crate::settings::{Settings, SettingsStore};
use crate::state::{HashesJob, IndexJob, OpenInputs};
use crate::view::Finding;

/// Runs `work` on a blocking thread: the body of a command runs on the thread that draws the
/// window. A panic in it answers as an internal error.
async fn off_thread<T, F>(work: F) -> IpcResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| {
            Err(AppError::Internal {
                detail: error.to_string(),
            })
        })
        .into()
}

/// The settings as they stand.
#[tauri::command]
#[specta::specta]
pub fn get_settings(store: State<'_, SettingsStore>) -> IpcResult<Settings> {
    IpcResult::ok(store.get())
}

/// Saves the settings, and answers them as saved.
#[tauri::command]
#[specta::specta]
pub fn save_settings(store: State<'_, SettingsStore>, settings: Settings) -> IpcResult<Settings> {
    store.save(settings).into()
}

/// Every fixup of the library, in the order they run.
#[tauri::command]
#[specta::specta]
pub fn fixup_catalog() -> IpcResult<Vec<FixupEntry>> {
    IpcResult::ok(catalog::catalog())
}

/// Every check of the library, in the order they run.
#[tauri::command]
#[specta::specta]
pub fn check_catalog() -> IpcResult<Vec<CheckEntry>> {
    IpcResult::ok(catalog::checks())
}

/// The game install on this machine, found as LTK Manager finds it. None when none is found.
#[tauri::command]
#[specta::specta]
pub async fn find_game() -> IpcResult<Option<String>> {
    off_thread(|| Ok(inputs::find_game())).await
}

/// A stage of the run under way starting or ending.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct RunStage {
    /// The stage.
    pub stage: StageName,
    /// Whether it started or ended.
    pub event: StageEvent,
}

/// A finding of the run under way, as it is raised.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct RunFinding {
    /// The finding, its text as a run without names writes it.
    pub finding: Finding,
}

/// A line of the details of the run under way.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct RunLog {
    /// The line.
    pub line: LogLine,
}

/// What a stage of the run under way gave the views, as it ended.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct RunPart {
    /// The part.
    pub part: PartView,
}

/// Sends what a run hears to the window as events.
struct Window(AppHandle);

impl Watch for Window {
    fn stage(&mut self, stage: StageName, event: StageEvent) {
        let _ = RunStage { stage, event }.emit(&self.0);
    }

    fn findings(&mut self) -> Option<Box<dyn FnMut(Finding) + Send>> {
        let app = self.0.clone();
        Some(Box::new(move |finding| {
            let _ = RunFinding { finding }.emit(&app);
        }))
    }

    fn log(&mut self) -> Option<Box<dyn Fn(LogLine) + Send + Sync>> {
        let app = self.0.clone();
        Some(Box::new(move |line| {
            let _ = RunLog { line }.emit(&app);
        }))
    }

    fn takes_parts(&self) -> bool {
        true
    }

    fn part(&mut self, part: PartView) {
        let _ = RunPart { part }.emit(&self.0);
    }
}

/// A repair or a port of a mod, as far as the request asks, against the inputs the settings
/// choose. How it goes comes as `RunStage`, `RunFinding` and `RunLog` events.
#[tauri::command]
#[specta::specta]
pub async fn run_mod(app: AppHandle, request: RunRequest) -> IpcResult<RunView> {
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    off_thread(move || {
        let inputs = open.get(&choice)?;
        run::run(&inputs, &request, &mut Window(app))
    })
    .await
}

/// The stages of a run of the kind, in order, as the library names them.
#[tauri::command]
#[specta::specta]
pub async fn run_stages(app: AppHandle, kind: RunKind) -> IpcResult<Vec<StageName>> {
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    off_thread(move || {
        let inputs = open.get(&choice)?;
        Ok(run::stages_of(&inputs, kind))
    })
    .await
}

/// Saves a choice of fixups as a selection file the command reads with `--fixups`.
#[tauri::command]
#[specta::specta]
pub async fn save_selection(path: String, choice: FixupChoice) -> IpcResult<()> {
    off_thread(move || choice.save(Utf8Path::new(&path))).await
}

/// The choice of fixups a selection file holds.
#[tauri::command]
#[specta::specta]
pub async fn load_selection(path: String) -> IpcResult<FixupChoice> {
    off_thread(move || FixupChoice::open(Utf8Path::new(&path))).await
}

/// The newest release of the census history that holds an index. None when none does.
#[tauri::command]
#[specta::specta]
pub async fn index_release() -> IpcResult<Option<IndexReleaseView>> {
    off_thread(|| {
        let web = Web::for_checks()?;
        match web.latest()? {
            Some(release) => Ok(Some(release.view(&web)?)),
            None => Ok(None),
        }
    })
    .await
}

/// Downloads the newest index, installs it, and makes it the history the runs read. Answers
/// the settings as saved. Progress comes as `InstallProgress` events.
#[tauri::command]
#[specta::specta]
pub async fn install_index(app: AppHandle) -> IpcResult<Settings> {
    off_thread(move || {
        let job = app.state::<Arc<IndexJob>>();
        if !job.run.start() {
            return Err(AppError::IndexBusy);
        }
        let installed = download_index(&app, &job);
        job.run.end();
        let store = app.state::<SettingsStore>();
        let settings = Settings {
            history: Some(installed?.to_string()),
            ..store.get()
        };
        store.save(settings)
    })
    .await
}

fn download_index(app: &AppHandle, job: &IndexJob) -> Result<camino::Utf8PathBuf, AppError> {
    let web = Web::new()?;
    let release = web.latest()?.ok_or(AppError::NoIndexRelease)?;
    // An event per percent: a download of 900 MB reads some hundred thousand chunks.
    let mut shown = u64::MAX;
    let mut progress = |seen: InstallProgress| {
        let percent = seen.done * 100 / seen.total.max(1);
        if percent != shown {
            shown = percent;
            let _ = seen.emit(app);
        }
    };
    Installer::new(&web, &job.run.cancel, &mut progress).install(&release, &job.root)
}

/// Stops the download or the update of the index under way.
#[tauri::command]
#[specta::specta]
pub fn cancel_index(job: State<'_, Arc<IndexJob>>) -> IpcResult<()> {
    job.run.cancel.store(true, Ordering::Relaxed);
    IpcResult::ok(())
}

/// Whether the census history holds builds after the head of the index the settings choose.
#[tauri::command]
#[specta::specta]
pub async fn index_behind(app: AppHandle) -> IpcResult<bool> {
    let history = app.state::<SettingsStore>().get().history;
    let job = Arc::clone(&app.state::<Arc<IndexJob>>());
    off_thread(move || {
        let index = history.ok_or(AppError::NoIndex)?;
        let never = AtomicBool::new(false);
        Updater::new(CENSUS, &job.census, &job.root, &never).behind(Utf8Path::new(&index))
    })
    .await
}

/// Fetches the census history's builds after the head of the index the settings choose,
/// appends them to a copy of the index, and makes the copy the history the runs read. Answers
/// the settings as saved. Progress comes as `IndexUpdateProgress` events.
#[tauri::command]
#[specta::specta]
pub async fn update_index(app: AppHandle) -> IpcResult<Settings> {
    off_thread(move || {
        let job = app.state::<Arc<IndexJob>>();
        let store = app.state::<SettingsStore>();
        let index = store.get().history.ok_or(AppError::NoIndex)?;
        if !job.run.start() {
            return Err(AppError::IndexBusy);
        }
        let updated = update_from_census(&app, &job, Utf8Path::new(&index));
        job.run.end();
        match updated? {
            Some(dir) => store.save(Settings {
                history: Some(dir.to_string()),
                ..store.get()
            }),
            None => Ok(store.get()),
        }
    })
    .await
}

fn update_from_census(
    app: &AppHandle,
    job: &IndexJob,
    index: &Utf8Path,
) -> Result<Option<Utf8PathBuf>, AppError> {
    let app = app.clone();
    // An event per request and per 256 KB: a fetch reads in chunks of a few KB.
    let shown = Mutex::new((usize::MAX, 0u64));
    let heard = move |step: UpdateStep| {
        if let UpdateStep::Fetch { objects, bytes } = step {
            let mut shown = shown.lock();
            if objects == shown.0 && bytes < shown.1 + 256 * 1024 {
                return;
            }
            *shown = (objects, bytes);
        }
        let _ = IndexUpdateProgress { step }.emit(&app);
    };
    Updater::new(CENSUS, &job.census, &job.root, &job.run.cancel)
        .on_progress(heard)
        .update(index)
}

fn no_data_dir() -> AppError {
    AppError::NoDataDir {
        detail: "the platform names no data directory".to_string(),
    }
}

/// The hash cache the settings choose, else the one the League Toolkit programs share.
fn hashes_dir(settings: &Settings) -> Result<Utf8PathBuf, AppError> {
    let chosen = Choice::from(settings).hashes;
    chosen.or_else(mimir_cache_dir).ok_or_else(no_data_dir)
}

/// What an update of the hash tables the settings choose would download.
#[tauri::command]
#[specta::specta]
pub async fn hashes_check(app: AppHandle) -> IpcResult<HashesCheck> {
    let settings = app.state::<SettingsStore>().get();
    off_thread(move || {
        let dir = hashes_dir(&settings)?;
        let web = Web::for_checks()?;
        let never = AtomicBool::new(false);
        hashes::check(&dir, &Release::new(&web, TABLES, &never))
    })
    .await
}

/// Downloads the hash tables the cache lacks or holds older, and installs them for every
/// program that reads the cache. Progress comes as `HashesProgress` events.
#[tauri::command]
#[specta::specta]
pub async fn update_hashes(app: AppHandle) -> IpcResult<()> {
    off_thread(move || {
        let dir = hashes_dir(&app.state::<SettingsStore>().get())?;
        let job = app.state::<Arc<HashesJob>>();
        if !job.run.start() {
            return Err(AppError::HashesBusy);
        }
        let web = Web::new();
        let updated = web.and_then(|web| {
            // An event per percent: a table of 38 MB arrives in some hundreds of chunks.
            let shown = Mutex::new(u64::MAX);
            let heard = |seen: HashesProgress| {
                let percent = seen.done * 100 / seen.total.max(1);
                let mut shown = shown.lock();
                if percent != *shown {
                    *shown = percent;
                    let _ = seen.emit(&app);
                }
            };
            hashes::update(&dir, &Release::new(&web, TABLES, &job.run.cancel), &heard)
        });
        job.run.end();
        updated?;
        // The tables open are the old ones: the next run opens the new.
        app.state::<Arc<OpenInputs>>().forget();
        Ok(())
    })
    .await
}

/// Stops the download of the hash tables under way.
#[tauri::command]
#[specta::specta]
pub fn cancel_hashes(job: State<'_, Arc<HashesJob>>) -> IpcResult<()> {
    job.run.cancel.store(true, Ordering::Relaxed);
    IpcResult::ok(())
}

/// How the schema database the League Toolkit programs share stands against the published one.
#[tauri::command]
#[specta::specta]
pub async fn schema_check() -> IpcResult<SchemaCheck> {
    off_thread(|| {
        let dir = meta_cache_dir().ok_or_else(no_data_dir)?;
        let web = Web::for_checks()?;
        schema::check(&dir, &Wiki::new(&web, schema::PUBLISHED))
    })
    .await
}

/// Downloads the published schema database into the folder the League Toolkit programs share,
/// unless the copy there is the one published.
#[tauri::command]
#[specta::specta]
pub async fn update_schema(app: AppHandle) -> IpcResult<()> {
    off_thread(move || {
        let dir = meta_cache_dir().ok_or_else(no_data_dir)?;
        let web = Web::new()?;
        if schema::update(&dir, &Wiki::new(&web, schema::PUBLISHED))? {
            // The database open is the old one: the next run opens the new.
            app.state::<Arc<OpenInputs>>().forget();
        }
        Ok(())
    })
    .await
}

/// The inputs the settings choose, opened, and what opening them found.
#[tauri::command]
#[specta::specta]
pub async fn open_inputs(app: AppHandle) -> IpcResult<InputsView> {
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    off_thread(move || Ok(open.get(&choice)?.view())).await
}
