//! The command table, and the TypeScript bindings generated from it. The front end calls the
//! commands through the bindings alone, so a type and its view do not drift (#ADR-0046).

use std::path::Path;

use specta_typescript::Typescript;
use tauri::Wry;
use tauri_specta::{Builder, collect_commands, collect_events};

use crate::archive::ArchiveProgress;
use crate::census::IndexUpdateProgress;
use crate::commands;
use crate::hashes::HashesProgress;
use crate::index::InstallProgress;

/// Where the bindings are kept, relative to this crate.
pub const BINDINGS: &str = "../../app/src/lib/bindings.gen.ts";

/// The builder the bindings are generated from and the handler is built from.
pub fn builder() -> Builder<Wry> {
    // A count or a size as a TypeScript `number`, as LTK Manager writes them: none comes near
    // 2^53.
    let builder = Builder::<Wry>::new().dangerously_cast_bigints_to_number();
    builder
        .commands(collect_commands![
            commands::get_settings,
            commands::save_settings,
            commands::fixup_catalog,
            commands::check_catalog,
            commands::find_game,
            commands::installs,
            commands::add_install,
            commands::remove_install,
            commands::choose_install,
            commands::open_inputs,
            commands::run_mod,
            commands::peek_mod,
            commands::run_stages,
            commands::list_runs,
            commands::run_report,
            commands::run_view,
            commands::run_files,
            commands::run_file,
            commands::run_settings,
            commands::compare_runs,
            commands::compare_files,
            commands::keep_run,
            commands::remove_run,
            commands::remove_mod_runs,
            commands::remove_all_runs,
            commands::mod_ingest_bytes,
            commands::clear_mod_ingest,
            commands::clear_unused_ingest,
            commands::store_view,
            commands::save_selection,
            commands::load_selection,
            commands::index_release,
            commands::install_index,
            commands::cancel_index,
            commands::index_behind,
            commands::update_index,
            commands::census_clone,
            commands::bootstrap_census,
            commands::storage,
            commands::remove_index,
            commands::remove_census,
            commands::remove_downloads,
            commands::reveal_place,
            commands::hashes_check,
            commands::update_hashes,
            commands::cancel_hashes,
            commands::schema_check,
            commands::update_schema,
        ])
        .events(collect_events![
            InstallProgress,
            ArchiveProgress,
            IndexUpdateProgress,
            HashesProgress,
            commands::RunStage,
            commands::RunFinding,
            commands::RunPart
        ])
}

/// Writes the bindings to `path`.
pub fn export(path: &Path) -> Result<(), specta_typescript::Error> {
    builder().export(Typescript::default(), path)
}
