//! The commands the front end calls. Each is thin: it takes the app's state and answers an
//! [`IpcResult`], and the work is in the module it calls. Work that reads the disk or a mod
//! runs through [`off_thread`].

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use parking_lot::Mutex;
use reforge::game::{meta_cache_dir, mimir_cache_dir};
use reforge::model::ContentId;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use crate::archive::{ArchiveNote, ArchiveProgress, Bootstrap, CloneView};
use crate::batch::{self, BatchProgress, BatchRequest, BatchView};
use crate::catalog::{self, CheckEntry, FixupEntry};
use crate::census::{CENSUS, IndexUpdateProgress, UpdateStep, Updater};
use crate::error::{AppError, IpcResult};
use crate::explore::{self, ExploreView, ExploredMod};
use crate::guided::{self, Question};
use crate::hashes::{self, HashesCheck, HashesProgress, Release, TABLES};
use crate::index::{IndexReleaseView, InstallProgress, InstallStep, Installer, Web};
use crate::inputs::{Choice, InputsView};
use crate::installs::{self, InstallView};
use crate::lookup::{HistoryBuilds, Lookup, LookupView};
use crate::records::{self, RecordHead, RecordView};
use crate::reveal::{self, Place};
use crate::run::{
    self, ModPeek, PartView, RunKind, RunRequest, RunView, StageEvent, StageName, Watch,
};
use crate::schema::{self, SchemaCheck, Wiki};
use crate::selection::FixupChoice;
use crate::settings::{Settings, SettingsStore};
use crate::state::{Explored, HashesJob, IndexJob, OpenInputs, RunJob, RunStore, SchemaJob};
use crate::storage::StorageView;
use crate::store::{AppStore, CompareView, FileView, RunAgain, RunRow, StageFiles, StoreView};
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

/// Every record bundled into the app, in series order then number order.
#[tauri::command]
#[specta::specta]
pub fn records() -> IpcResult<Vec<RecordHead>> {
    IpcResult::ok(records::list())
}

/// A bundled record by its code, with or without its `#`. None for a code the bundle lacks.
#[tauri::command]
#[specta::specta]
pub fn record(code: String) -> IpcResult<Option<RecordView>> {
    IpcResult::ok(records::record(&code))
}

/// The guided view's questions about a run, from the view the window holds of it (#ADR-0090).
/// The window passes the view back, so the app keeps no state of the guided view.
#[tauri::command]
#[specta::specta]
pub fn guided_questions(view: RunView) -> IpcResult<Vec<Question>> {
    IpcResult::ok(guided::questions(&view))
}

/// The game installs detected on this machine and the ones added, the chosen one first
/// (#ADR-0101).
#[tauri::command]
#[specta::specta]
pub async fn installs(app: AppHandle) -> IpcResult<Vec<InstallView>> {
    off_thread(move || {
        let settings = app.state::<SettingsStore>().get();
        Ok(installs::list(installs::detect(), &settings))
    })
    .await
}

/// Adds a folder to the installs, chosen when none is, and answers the list. Refused for a
/// folder that is no install.
#[tauri::command]
#[specta::specta]
pub async fn add_install(app: AppHandle, path: String) -> IpcResult<Vec<InstallView>> {
    off_thread(move || installs::add(&app.state::<SettingsStore>(), installs::detect(), &path))
        .await
}

/// Takes an added install off the list, and answers the list. A detected install stays.
#[tauri::command]
#[specta::specta]
pub async fn remove_install(app: AppHandle, path: String) -> IpcResult<Vec<InstallView>> {
    off_thread(move || installs::remove(&app.state::<SettingsStore>(), installs::detect(), &path))
        .await
}

/// Chooses a listed install as the one a run reads, and answers the settings as saved.
#[tauri::command]
#[specta::specta]
pub async fn choose_install(app: AppHandle, path: String) -> IpcResult<Settings> {
    off_thread(move || installs::choose(&app.state::<SettingsStore>(), installs::detect(), &path))
        .await
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

    fn takes_parts(&self) -> bool {
        true
    }

    fn part(&mut self, part: PartView) {
        let _ = RunPart { part }.emit(&self.0);
    }
}

/// A repair or a port of a mod, as far as the request asks, against the inputs the settings
/// choose, one run at a time. How it goes comes as `RunStage`, `RunFinding` and `RunPart`
/// events.
#[tauri::command]
#[specta::specta]
pub async fn run_mod(app: AppHandle, request: RunRequest) -> IpcResult<RunView> {
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    // A store that did not open leaves the run unkept.
    let store = app.state::<RunStore>().get().ok();
    off_thread(move || {
        let job = Arc::clone(&app.state::<Arc<RunJob>>());
        let cancel = job.start().ok_or(AppError::RunBusy)?;
        let ran = open.get(&choice).and_then(|inputs| {
            let mut watch = Window(app);
            run::run(
                &inputs,
                &request,
                store.as_deref(),
                &mut watch,
                Some(&cancel),
            )
        });
        if let Ok(view) = &ran {
            job.set_written(view.written.as_deref().map(Utf8PathBuf::from));
        }
        job.run.end();
        ran
    })
    .await
}

/// Runs a batch of mods through `write`, one after another against the inputs the settings
/// choose, as one job with `run_mod`'s (#ADR-0091). How it goes comes as `BatchProgress`
/// events, and no run's events are sent.
#[tauri::command]
#[specta::specta]
pub async fn run_batch(app: AppHandle, request: BatchRequest) -> IpcResult<BatchView> {
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    // A store that did not open leaves the runs unkept.
    let store = app.state::<RunStore>().get().ok();
    off_thread(move || {
        let job = Arc::clone(&app.state::<Arc<RunJob>>());
        let cancel = job.start().ok_or(AppError::RunBusy)?;
        let inputs = batch::Chosen {
            open: &open,
            choice: &choice,
        };
        let mut progress = |seen: BatchProgress| {
            let _ = seen.emit(&app);
        };
        let ran = batch::run_batch(&inputs, &request, store.as_deref(), &cancel, &mut progress);
        job.run.end();
        ran
    })
    .await
}

/// Stops the run under way: the stage under way ends after its step (#ADR-0097), and the run
/// ends with its report, writes nothing and is not kept.
#[tauri::command]
#[specta::specta]
pub fn cancel_run(job: State<'_, Arc<RunJob>>) -> IpcResult<()> {
    job.cancel();
    IpcResult::ok(())
}

/// The runs the app's store keeps, newest first (#ADR-0096).
#[tauri::command]
#[specta::specta]
pub async fn list_runs(app: AppHandle) -> IpcResult<Vec<RunRow>> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.list()).await
}

/// The report of a kept run, as JSON. None for a number no run has.
#[tauri::command]
#[specta::specta]
pub async fn run_report(app: AppHandle, id: u64) -> IpcResult<Option<String>> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.report(id)).await
}

/// What the views drew of a kept run as it ended. None for a number no run has.
#[tauri::command]
#[specta::specta]
pub async fn run_view(app: AppHandle, id: u64) -> IpcResult<Option<RunView>> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.run_view(id)).await
}

/// The files of a kept run at a stage: `load`, `fix`, `plan` or `build`. None for a number no
/// run has.
#[tauri::command]
#[specta::specta]
pub async fn run_files(app: AppHandle, id: u64, stage: StageName) -> IpcResult<Option<StageFiles>> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.stage_files(id, stage)).await
}

/// Two kept runs compared, `a` against `b` (#ADR-0091). None when either number names no run.
#[tauri::command]
#[specta::specta]
pub async fn compare_runs(app: AppHandle, a: u64, b: u64) -> IpcResult<Option<CompareView>> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.compare(a, b)).await
}

/// The files of kept run `b` at a stage, each marked against kept run `a`'s at the same stage.
/// None when either number names no run.
#[tauri::command]
#[specta::specta]
pub async fn compare_files(
    app: AppHandle,
    a: u64,
    b: u64,
    stage: StageName,
) -> IpcResult<Option<StageFiles>> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.compare_files(a, b, stage)).await
}

/// One file of a kept run, by the id of its bytes as 64 hex digits, shown by its kind. A
/// bin's names are spelled through the run's names and the inputs' tables, when they open.
#[tauri::command]
#[specta::specta]
pub async fn run_file(app: AppHandle, id: u64, content: String) -> IpcResult<FileView> {
    let store = app.state::<RunStore>().get();
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    off_thread(move || {
        let store = store?;
        let content = content_of(&content)?;
        // Inputs that do not open leave the run's own names.
        let inputs = open.get(&choice).ok();
        let names = inputs.as_deref().map(|inputs| inputs.env().names());
        store.file_view(id, content, names)
    })
    .await
}

/// A content id as 64 hex digits, as the front end names a file's bytes.
fn content_of(hex: &str) -> Result<ContentId, AppError> {
    ContentId::parse(hex).ok_or_else(|| AppError::Internal {
        detail: format!("{hex:?} is no content id"),
    })
}

/// What a kept run was run on and with, for the run page to run it again. None for a number
/// no run has.
#[tauri::command]
#[specta::specta]
pub async fn run_settings(app: AppHandle, id: u64) -> IpcResult<Option<RunAgain>> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.run_again(id)).await
}

/// Writes a kept run as a project file at `path` (#ADR-0096), and answers the file's size.
/// None for a number no run has.
#[tauri::command]
#[specta::specta]
pub async fn export_project(app: AppHandle, id: u64, path: String) -> IpcResult<Option<u64>> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.export_project(id, Utf8Path::new(&path))).await
}

/// Imports a project file as a kept run (#ADR-0096), and answers its number.
#[tauri::command]
#[specta::specta]
pub async fn import_project(app: AppHandle, path: String) -> IpcResult<u64> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.import_project(Utf8Path::new(&path))).await
}

/// Opens a package's embedded evidence as a kept run (#ADR-0041, #ADR-0096), and answers its
/// number.
#[tauri::command]
#[specta::specta]
pub async fn open_package(app: AppHandle, path: String) -> IpcResult<u64> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.open_package(Utf8Path::new(&path))).await
}

/// Marks a kept run so that the store's bound does not remove it, or unmarks it.
#[tauri::command]
#[specta::specta]
pub async fn keep_run(app: AppHandle, id: u64, kept: bool) -> IpcResult<()> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.keep(id, kept)).await
}

/// The store, unless a run is under way: a removal waits for none.
fn idle_store(app: &AppHandle) -> Result<Arc<AppStore>, AppError> {
    if app.state::<Arc<RunJob>>().run.running() {
        return Err(AppError::RunBusy);
    }
    app.state::<RunStore>().get()
}

/// Removes a kept run, and what no other run names.
#[tauri::command]
#[specta::specta]
pub async fn remove_run(app: AppHandle, id: u64) -> IpcResult<()> {
    off_thread(move || idle_store(&app)?.remove_run(id)).await
}

/// Removes every kept run of the mod whose archive has this SHA-256, as hex.
#[tauri::command]
#[specta::specta]
pub async fn remove_mod_runs(app: AppHandle, sha256: String) -> IpcResult<()> {
    off_thread(move || {
        let store = idle_store(&app)?;
        store.remove_mod(sha256_of(&sha256)?)
    })
    .await
}

/// A SHA-256 as hex, as the front end names an archive.
fn sha256_of(hex: &str) -> Result<ContentId, AppError> {
    ContentId::parse(hex).ok_or_else(|| AppError::Internal {
        detail: format!("{hex:?} is no SHA-256"),
    })
}

/// The bytes the ingest tier holds of the mod whose archive has this SHA-256, as hex: what
/// clearing its archive data frees at most.
#[tauri::command]
#[specta::specta]
pub async fn mod_ingest_bytes(app: AppHandle, sha256: String) -> IpcResult<u64> {
    let store = app.state::<RunStore>().get();
    off_thread(move || Ok(store?.mod_ingest_bytes(sha256_of(&sha256)?))).await
}

/// Clears the archive data of the mod whose archive has this SHA-256, as hex, from the ingest
/// tier. Its kept runs stay, read-only until the archive is read again.
#[tauri::command]
#[specta::specta]
pub async fn clear_mod_ingest(app: AppHandle, sha256: String) -> IpcResult<()> {
    off_thread(move || {
        let store = idle_store(&app)?;
        store.clear_ingest(sha256_of(&sha256)?).map(drop)
    })
    .await
}

/// Clears the archive data of every mod no kept run is of from the ingest tier.
#[tauri::command]
#[specta::specta]
pub async fn clear_unused_ingest(app: AppHandle) -> IpcResult<()> {
    off_thread(move || idle_store(&app)?.clear_unused_ingest().map(drop)).await
}

/// Removes every kept run.
#[tauri::command]
#[specta::specta]
pub async fn remove_all_runs(app: AppHandle) -> IpcResult<()> {
    off_thread(move || idle_store(&app)?.remove_all()).await
}

/// What the app's store holds and its bytes on disk, for the settings page.
#[tauri::command]
#[specta::specta]
pub async fn store_view(app: AppHandle) -> IpcResult<StoreView> {
    let store = app.state::<RunStore>().get();
    off_thread(move || store?.view()).await
}

/// A mod archive read alone: what it says of itself, its characters and its thumbnail, for
/// the window to show when the mod is picked.
#[tauri::command]
#[specta::specta]
pub async fn peek_mod(archive: String) -> IpcResult<ModPeek> {
    off_thread(move || run::peek(Utf8Path::new(&archive))).await
}

/// The mod archives of a folder, `.fantome` and `.zip`, sorted by name. Its subfolders are not
/// read.
#[tauri::command]
#[specta::specta]
pub async fn list_archives(folder: String) -> IpcResult<Vec<String>> {
    off_thread(move || batch::list_archives(Utf8Path::new(&folder))).await
}

/// A mod archive explored through `load` (#ADR-0091), against the inputs the settings choose,
/// and held until the next explore. It runs as `run_mod` does, one run at a time, and keeps
/// nothing in the store's rows.
#[tauri::command]
#[specta::specta]
pub async fn explore_mod(app: AppHandle, archive: String) -> IpcResult<ExploreView> {
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    // A store that did not open leaves the run reading the archive into memory.
    let store = app.state::<RunStore>().get().ok();
    off_thread(move || {
        let job = Arc::clone(&app.state::<Arc<RunJob>>());
        if !job.run.start() {
            return Err(AppError::RunBusy);
        }
        let path = Utf8Path::new(&archive);
        let explored = open
            .get(&choice)
            .map(|inputs| explore::explore(&inputs, path, store.as_deref()));
        job.run.end();
        let explored = explored?;
        let view = explored.view().clone();
        app.state::<Arc<Explored>>().set(explored);
        Ok(view)
    })
    .await
}

/// The explored mod's files at `load`, every mark `same`. None when no mod is explored or its
/// `load` did not end.
#[tauri::command]
#[specta::specta]
pub async fn explore_files(app: AppHandle) -> IpcResult<Option<StageFiles>> {
    let store = app.state::<RunStore>().get().ok();
    let explored = Arc::clone(&app.state::<Arc<Explored>>());
    off_thread(move || {
        Ok(explored.with(|explored| explored.and_then(|e| e.files(store.as_deref()))))
    })
    .await
}

/// One file of the explored mod, by the id of its bytes as 64 hex digits, shown by its kind.
/// A bin's names are spelled through the load's names and the inputs' tables, when they open.
#[tauri::command]
#[specta::specta]
pub async fn explore_file(app: AppHandle, content: String) -> IpcResult<FileView> {
    let store = app.state::<RunStore>().get().ok();
    let explored = Arc::clone(&app.state::<Arc<Explored>>());
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    off_thread(move || {
        let content = content_of(&content)?;
        // Inputs that do not open leave the load's own names.
        let inputs = open.get(&choice).ok();
        let names = inputs.as_deref().map(|inputs| inputs.env().names());
        explored.with(|explored| explore::explored_file(explored, store.as_deref(), content, names))
    })
    .await
}

/// The explored mod's archive and the standard settings, for the run page to run it. None when
/// no mod is explored.
#[tauri::command]
#[specta::specta]
pub async fn explore_run(app: AppHandle) -> IpcResult<Option<RunAgain>> {
    let explored = Arc::clone(&app.state::<Arc<Explored>>());
    off_thread(move || Ok(explored.with(|explored| explored.map(ExploredMod::run_again)))).await
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

/// The newest release of the census history that holds an index of the app's format. None when
/// none does.
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
    // An event per percent of each step: a download of 800 MB reads some hundred thousand chunks.
    let mut shown = (InstallStep::Download, u64::MAX);
    let mut progress = |seen: InstallProgress| {
        let percent = seen.done * 100 / seen.total.max(1);
        if (seen.step, percent) != shown {
            shown = (seen.step, percent);
            let _ = seen.emit(app);
        }
    };
    Installer::new(&web, &job.run.cancel, &mut progress).install(
        &release,
        &job.root,
        &job.downloads,
    )
}

/// The app's clone of the census history: none, fetched as far as appends read it, or whole.
#[tauri::command]
#[specta::specta]
pub fn census_clone(job: State<'_, Arc<IndexJob>>) -> IpcResult<CloneView> {
    IpcResult::ok(job.clones.view())
}

/// Downloads the newest release's archive of the census history and makes the app's clone of
/// it, in place of the clone held (#ADR-0060). Answers the clone as made. Progress comes as
/// `ArchiveProgress` events.
#[tauri::command]
#[specta::specta]
pub async fn bootstrap_census(app: AppHandle) -> IpcResult<CloneView> {
    off_thread(move || {
        let job = app.state::<Arc<IndexJob>>();
        if !job.run.start() {
            return Err(AppError::IndexBusy);
        }
        let made = bootstrap(&app, &job);
        job.run.end();
        made?;
        Ok(job.clones.view())
    })
    .await
}

fn bootstrap(app: &AppHandle, job: &IndexJob) -> Result<ArchiveNote, AppError> {
    let web = Web::new()?;
    let release = web.latest()?.ok_or(AppError::NoIndexRelease)?;
    // An event per percent of each step: a download of 1.8 GB reads some hundred thousand chunks.
    let mut shown = (InstallStep::Download, u64::MAX);
    let mut progress = |seen: ArchiveProgress| {
        let percent = seen.done * 100 / seen.total.max(1);
        if (seen.step, percent) != shown {
            shown = (seen.step, percent);
            let _ = seen.emit(app);
        }
    };
    Bootstrap::new(&web, &job.run.cancel, &mut progress).run(&release, &job.clones, &job.downloads)
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
        Updater::new(CENSUS, job.clones.for_updates(), &job.root, &never)
            .behind(Utf8Path::new(&index))
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
    Updater::new(CENSUS, job.clones.for_updates(), &job.root, &job.run.cancel)
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
        let job = app.state::<Arc<SchemaJob>>();
        if !job.run.start() {
            return Err(AppError::SchemaBusy);
        }
        let updated =
            Web::new().and_then(|web| schema::update(&dir, &Wiki::new(&web, schema::PUBLISHED)));
        job.run.end();
        if updated? {
            // The database open is the old one: the next run opens the new.
            app.state::<Arc<OpenInputs>>().forget();
        }
        Ok(())
    })
    .await
}

/// What the app keeps on disk for the history: the index, the clones and the downloads kept.
#[tauri::command]
#[specta::specta]
pub async fn storage(app: AppHandle) -> IpcResult<StorageView> {
    let history = app.state::<SettingsStore>().get().history;
    let job = Arc::clone(&app.state::<Arc<IndexJob>>());
    off_thread(move || Ok(job.storage().view(history.as_deref().map(Utf8Path::new)))).await
}

/// Removes every index the app installed (#ADR-0061). The settings choose none after when
/// the index they chose was one of them, and are answered as saved.
#[tauri::command]
#[specta::specta]
pub async fn remove_index(app: AppHandle) -> IpcResult<Settings> {
    off_thread(move || {
        let job = app.state::<Arc<IndexJob>>();
        if !job.run.start() {
            return Err(AppError::IndexBusy);
        }
        let removed = remove_indexes(&app, &job);
        job.run.end();
        removed
    })
    .await
}

fn remove_indexes(app: &AppHandle, job: &IndexJob) -> Result<Settings, AppError> {
    let store = app.state::<SettingsStore>();
    let mut settings = store.get();
    let chosen = settings.history.as_deref().map(Utf8Path::new);
    if chosen.is_some_and(|dir| job.storage().owns(dir)) {
        settings = store.save(Settings {
            history: None,
            ..settings
        })?;
    }
    // The index open is one of them, and a file held open is not removed.
    app.state::<Arc<OpenInputs>>().forget();
    job.storage().remove_indexes()?;
    Ok(settings)
}

/// Removes the app's clones of the census history (#ADR-0061), and answers the clone as it
/// stands.
#[tauri::command]
#[specta::specta]
pub async fn remove_census(app: AppHandle) -> IpcResult<CloneView> {
    off_thread(move || {
        let job = app.state::<Arc<IndexJob>>();
        if !job.run.start() {
            return Err(AppError::IndexBusy);
        }
        let removed = job.storage().remove_clones();
        job.run.end();
        removed?;
        Ok(job.clones.view())
    })
    .await
}

/// Removes the downloads kept to resume (#ADR-0061).
#[tauri::command]
#[specta::specta]
pub async fn remove_downloads(app: AppHandle) -> IpcResult<()> {
    off_thread(move || {
        let job = app.state::<Arc<IndexJob>>();
        if !job.run.start() {
            return Err(AppError::IndexBusy);
        }
        let removed = job.storage().remove_downloads();
        job.run.end();
        removed
    })
    .await
}

/// Shows a place in the system's file manager: a folder of the inputs, or the package the
/// last run or a kept run wrote, selected in its folder.
#[tauri::command]
#[specta::specta]
pub async fn reveal_place(app: AppHandle, place: Place) -> IpcResult<()> {
    let settings = app.state::<SettingsStore>().get();
    let job = Arc::clone(&app.state::<Arc<IndexJob>>());
    let runs = Arc::clone(&app.state::<Arc<RunJob>>());
    let store = app.state::<RunStore>().get();
    off_thread(move || {
        let chosen = Choice::from(&settings);
        let none_written = || AppError::NotShown {
            path: String::new(),
            detail: "no package was written".to_string(),
        };
        let dir = match place {
            Place::Written => {
                let file = runs.written().ok_or_else(none_written)?;
                return reveal::reveal_file(&file);
            }
            Place::Run { id } => {
                let file = store?.package_of(id)?.ok_or_else(none_written)?;
                return reveal::reveal_file(Utf8Path::new(&file));
            }
            Place::Install { path } => {
                if !installs::is_listed(installs::detect(), &settings, &path) {
                    return Err(AppError::NotShown {
                        path,
                        detail: "not a listed install".to_string(),
                    });
                }
                Some(Utf8PathBuf::from(path))
            }
            Place::Hashes => Some(hashes_dir(&settings)?),
            Place::Schema => match chosen.schema {
                Some(file) => file.parent().map(Utf8Path::to_path_buf),
                None => meta_cache_dir(),
            },
            Place::Index => chosen.history,
            Place::Census => Some(job.clones.for_updates().to_path_buf()),
        };
        let dir = dir.ok_or_else(|| AppError::NotShown {
            path: String::new(),
            detail: "no folder is chosen".to_string(),
        })?;
        reveal::reveal(&dir)
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

/// Asks the history of the inputs the settings choose, off the window's thread. Refused when
/// they hold no history.
async fn ask_history<T, F>(app: &AppHandle, ask: F) -> IpcResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Lookup<'_>) -> Result<T, AppError> + Send + 'static,
{
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    off_thread(move || {
        let inputs = open.get(&choice)?;
        ask(&Lookup::of(inputs.env())?)
    })
    .await
}

/// The builds of the history the settings choose, in position order, and its head.
#[tauri::command]
#[specta::specta]
pub async fn history_builds(app: AppHandle) -> IpcResult<HistoryBuilds> {
    ask_history(&app, |lookup| Ok(lookup.builds())).await
}

/// A path, or its path hash as sixteen hex digits, looked up in the history the settings
/// choose: its life, and at the build at position `at`, the head when none is given, the WADs
/// that held it, the objects that named it and the bin it was.
#[tauri::command]
#[specta::specta]
pub async fn history_lookup(
    app: AppHandle,
    query: String,
    at: Option<u16>,
) -> IpcResult<LookupView> {
    ask_history(&app, move |lookup| lookup.lookup(&query, at)).await
}

/// The paths the hash tables spell that start with a prefix, for the History page's search.
#[tauri::command]
#[specta::specta]
pub async fn history_suggest(app: AppHandle, prefix: String) -> IpcResult<Vec<String>> {
    ask_history(&app, move |lookup| Ok(lookup.suggest(&prefix))).await
}
