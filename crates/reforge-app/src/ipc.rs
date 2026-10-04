//! The command table, and the TypeScript bindings generated from it. The front end calls the
//! commands through the bindings alone, so a type and its view do not drift (#ADR-0046).

use std::path::Path;

use specta_typescript::Typescript;
use tauri::Wry;
use tauri_specta::{Builder, collect_commands, collect_events};

use crate::commands;
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
            commands::open_inputs,
            commands::run_mod,
            commands::run_stages,
            commands::save_selection,
            commands::load_selection,
            commands::index_release,
            commands::install_index,
            commands::cancel_index,
        ])
        .events(collect_events![
            InstallProgress,
            commands::RunStage,
            commands::RunFinding,
            commands::RunLog,
            commands::RunPart
        ])
}

/// Writes the bindings to `path`.
pub fn export(path: &Path) -> Result<(), specta_typescript::Error> {
    builder().export(Typescript::default(), path)
}
