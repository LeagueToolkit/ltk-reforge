//! The commands the front end calls. Each is thin: it takes the app's state and answers an
//! [`IpcResult`], and the work is in the module it calls. Work that reads the disk or a mod
//! runs through [`off_thread`].

use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::catalog::{self, CheckEntry, FixupEntry};
use crate::error::{AppError, IpcResult};
use crate::inputs::{self, Choice, InputsView};
use crate::run::{self, RunRequest, RunView};
use crate::settings::{Settings, SettingsStore};
use crate::state::OpenInputs;

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

/// A repair of a mod, as far as the request asks, against the inputs the settings choose.
#[tauri::command]
#[specta::specta]
pub async fn run_repair(app: AppHandle, request: RunRequest) -> IpcResult<RunView> {
    let choice = Choice::from(&app.state::<SettingsStore>().get());
    let open = Arc::clone(&app.state::<Arc<OpenInputs>>());
    off_thread(move || {
        let inputs = open.get(&choice)?;
        run::repair(&inputs, &request)
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
