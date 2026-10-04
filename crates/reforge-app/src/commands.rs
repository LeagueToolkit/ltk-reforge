//! The commands the front end calls. Each is thin: it takes the app's state and answers an
//! [`IpcResult`], and the work is in the module it calls. Work that reads the disk or a mod
//! runs through [`off_thread`].

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use crate::index::{IndexReleaseView, InstallProgress, Installer, Web};
use crate::state::IndexJob;

use crate::catalog::{self, CheckEntry, FixupEntry};
use crate::error::{AppError, IpcResult};
use crate::inputs::{self, Choice, InputsView};
use crate::run::{self, PartView, RunKind, RunRequest, RunView, StageEvent, StageName, Watch};
use crate::runlog::LogLine;
use crate::selection::FixupChoice;
use crate::settings::{Settings, SettingsStore};
use crate::state::OpenInputs;
use crate::view::Finding;
use camino::Utf8Path;
use serde::{Deserialize, Serialize};

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
        let web = Web::new()?;
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
        if !job.start() {
            return Err(AppError::IndexBusy);
        }
        let installed = download_index(&app, &job);
        job.end();
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
    Installer::new(&web, &job.cancel, &mut progress).install(&release, &job.root)
}

/// Stops the download of the index under way.
#[tauri::command]
#[specta::specta]
pub fn cancel_index(job: State<'_, Arc<IndexJob>>) -> IpcResult<()> {
    job.cancel.store(true, Ordering::Relaxed);
    IpcResult::ok(())
}

/// The inputs the settings choose, opened, and what opening them found.
#[tauri::command]
#[specta::specta]
pub async fn open_inputs(app: AppHandle) -> IpcResult<InputsView> {
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    off_thread(move || Ok(open.get(&choice)?.view())).await
}
