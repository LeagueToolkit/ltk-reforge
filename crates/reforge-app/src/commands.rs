//! The commands the front end calls. Each is thin: it takes the app's state and answers an
//! [`IpcResult`], and the work is in the module it calls.

use tauri::State;

use crate::catalog::{self, FixupEntry};
use crate::error::IpcResult;
use crate::settings::{Settings, SettingsStore};

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
