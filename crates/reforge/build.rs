//! The commit the program is built from, as `REFORGE_COMMIT`, for the snapshots a run writes
//! (#ADR-0094). A `REFORGE_COMMIT` in the build's environment is taken as given: CI builds a
//! tarball of the commit, with no `.git` to ask. Else git says, and a build outside a git
//! checkout has none.

use std::path::Path;
use std::process::Command;

/// What git prints for `args`, trimmed. None when git is absent or the command fails.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    output.status.success().then(|| text.trim().to_string())
}

fn main() {
    println!("cargo:rerun-if-env-changed=REFORGE_COMMIT");
    if let Ok(commit) = std::env::var("REFORGE_COMMIT") {
        let commit = commit.trim();
        if !commit.is_empty() {
            println!("cargo:rustc-env=REFORGE_COMMIT={commit}");
            return;
        }
    }
    // A file that does not exist would run this script on every build.
    let watch = |name: &str| {
        let path = git(&["rev-parse", "--git-path", name])?;
        Path::new(&path)
            .exists()
            .then(|| println!("cargo:rerun-if-changed={path}"))
    };
    watch("HEAD");
    watch("packed-refs");
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watch(&branch);
    }
    if let Some(commit) = git(&["rev-parse", "HEAD"]) {
        println!("cargo:rustc-env=REFORGE_COMMIT={commit}");
    }
}
