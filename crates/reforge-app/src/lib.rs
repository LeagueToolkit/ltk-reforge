//! The reforge desktop app: a caller of `reforge`, as the command is (#ADR-0046). It finds the
//! inputs, drives a run and shows each stage's result, and holds no rule of its own about a
//! mod. `docs/design/ROADMAP-UX.md` holds the plan.

pub mod catalog;
pub mod commands;
pub mod error;
pub mod ipc;
pub mod settings;

use tauri::Manager;

use crate::settings::SettingsStore;

/// Starts the app and returns when its window closes.
pub fn run() -> tauri::Result<()> {
    let ipc = ipc::builder();
    tauri::Builder::default()
        .invoke_handler(ipc.invoke_handler())
        .setup(move |app| {
            ipc.mount_events(app);
            let settings = app.path().app_data_dir()?.join("settings.json");
            app.manage(SettingsStore::open(settings));
            Ok(())
        })
        .run(tauri::generate_context!())
}
