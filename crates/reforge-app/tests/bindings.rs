//! The TypeScript bindings the front end calls the commands through. They are committed, so the
//! front end builds without a Rust toolchain, and these tests hold them to the commands.
//! `pnpm generate:types` in `app/` writes them.

use std::path::Path;

use fs_err as fs;
use reforge_app::ipc::{self, BINDINGS};

fn committed() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(BINDINGS)
}

fn rendered() -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bindings.ts");
    ipc::export(&path).unwrap();
    fs::read_to_string(&path).unwrap()
}

#[test]
#[ignore = "writes the committed bindings; run by `pnpm generate:types`"]
fn write_bindings() {
    ipc::export(&committed()).unwrap();
}

#[test]
fn the_committed_bindings_are_the_commands_as_they_are() {
    let committed = fs::read_to_string(committed())
        .unwrap()
        .replace("\r\n", "\n");
    assert!(
        committed == rendered(),
        "{BINDINGS} differs from the commands: run `pnpm generate:types` in app/"
    );
}

#[test]
fn a_command_answers_the_envelope_the_front_end_narrows_on() {
    // Whole rather than one arm at a time: the front end's `Result<T>` narrows on `ok`, so an
    // arm that stops being a literal is what this fails on.
    const ENVELOPE: &str = concat!(
        "({ ok: true; value: Settings }) & { error?: never }",
        " | ({ ok: false; error: AppError }) & { value?: never }",
    );
    let bindings = rendered();
    assert!(
        bindings.contains(ENVELOPE),
        "the envelope changed:\n{bindings}"
    );
}
