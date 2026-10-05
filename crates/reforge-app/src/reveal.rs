//! A folder of the inputs shown in the system's file manager. The window names which folder,
//! never a path: the path comes from the settings and the app's own folders.

use std::path::PathBuf;
use std::process::Command;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// A folder the inputs page can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum Place {
    /// The game install.
    Game,
    /// The hash cache.
    Hashes,
    /// The folder of the schema database.
    Schema,
    /// The history index.
    Index,
    /// The app's clone of the census history.
    Census,
}

#[cfg(windows)]
const FILE_MANAGER: &str = "explorer.exe";
#[cfg(target_os = "macos")]
const FILE_MANAGER: &str = "open";
#[cfg(not(any(windows, target_os = "macos")))]
const FILE_MANAGER: &str = "xdg-open";

/// Opens the folder at `dir` in the system's file manager.
pub fn reveal(dir: &Utf8Path) -> Result<(), AppError> {
    let not_shown = |detail: String| AppError::NotShown {
        path: dir.to_string(),
        detail,
    };
    if !dir.is_dir() {
        return Err(not_shown("no such folder".to_string()));
    }
    // Explorer reads a path with `/` as no path and opens the user's documents.
    let native: PathBuf = dir.as_std_path().components().collect();
    Command::new(FILE_MANAGER)
        .arg(native)
        .spawn()
        .map(drop)
        .map_err(|error| not_shown(error.to_string()))
}
