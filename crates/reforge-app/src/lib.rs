//! The reforge desktop app: a caller of `reforge`, as the command is (#ADR-0046). It finds the
//! inputs, drives a run and shows each stage's result, and holds no rule of its own about a
//! mod. `docs/design/ROADMAP-UX.md` holds the plan.

pub mod catalog;
pub mod census;
pub mod commands;
pub mod error;
pub mod hashes;
pub mod index;
pub mod inputs;
pub mod ipc;
pub mod run;
pub mod runlog;
pub mod schema;
pub mod selection;
pub mod settings;
pub mod state;
pub mod view;

use std::sync::Arc;

use camino::Utf8PathBuf;
use tauri::Manager;

use crate::settings::SettingsStore;
use crate::state::{HashesJob, IndexJob, OpenInputs};

/// Starts the app and returns when its window closes.
pub fn run() -> tauri::Result<()> {
    let ipc = ipc::builder();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(ipc.invoke_handler())
        .setup(move |app| {
            ipc.mount_events(app);
            let settings = app.path().app_data_dir()?.join("settings.json");
            app.manage(SettingsStore::open(settings));
            let cache = app.path().app_cache_dir()?.join("names-index");
            let cache = Utf8PathBuf::from_path_buf(cache).map_err(|path| {
                format!("the app's cache folder is not UTF-8: {}", path.display())
            })?;
            app.manage(Arc::new(OpenInputs::new(cache)));

            let data = app.path().app_data_dir()?;
            let data = Utf8PathBuf::from_path_buf(data).map_err(|path| {
                format!("the app's data folder is not UTF-8: {}", path.display())
            })?;
            let indexes = data.join("history-indexes");
            // Before any run opens one: an index the settings no longer name is an older
            // release's, and nothing holds it open yet.
            let current = app
                .state::<SettingsStore>()
                .get()
                .history
                .map(Utf8PathBuf::from);
            if let Some(current) = current.filter(|path| path.starts_with(&indexes)) {
                index::remove_others(&indexes, &current);
            }
            app.manage(Arc::new(IndexJob::new(
                indexes,
                data.join("census-history"),
            )));
            app.manage(Arc::new(HashesJob::default()));
            Ok(())
        })
        .run(tauri::generate_context!())
}
