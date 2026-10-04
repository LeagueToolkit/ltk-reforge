//! The command table, and the TypeScript bindings generated from it. The front end calls the
//! commands through the bindings alone, so a type and its view do not drift (#ADR-0046).

use std::path::Path;

use specta_typescript::Typescript;
use tauri::Wry;
use tauri_specta::{Builder, collect_commands};

use crate::commands;

/// Where the bindings are kept, relative to this crate.
pub const BINDINGS: &str = "../../app/src/lib/bindings.gen.ts";

/// The builder the bindings are generated from and the handler is built from.
pub fn builder() -> Builder<Wry> {
    Builder::<Wry>::new().commands(collect_commands![
        commands::get_settings,
        commands::save_settings,
        commands::fixup_catalog,
    ])
}

/// Writes the bindings to `path`.
pub fn export(path: &Path) -> Result<(), specta_typescript::Error> {
    builder().export(Typescript::default(), path)
}
