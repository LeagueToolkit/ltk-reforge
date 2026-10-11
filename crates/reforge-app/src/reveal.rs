//! A folder of the inputs, or a package written, shown in the system's file manager. The
//! window names which place, never a path: the path comes from the settings, the app's own
//! folders, the last run and the store's rows. A game install is named by its path, and only
//! a path the list of installs holds is shown.

use std::path::PathBuf;
use std::process::Command;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// A place the window can show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum Place {
    /// The package the last run wrote, selected in its folder.
    Written,
    /// The package a kept run wrote, selected in its folder.
    Run {
        /// The run's number in the store.
        id: u64,
    },
    /// A game install of the list, detected or added (#ADR-0101).
    Install {
        /// Its root, as the list gives it.
        path: String,
    },
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
    Command::new(FILE_MANAGER)
        .arg(native(dir))
        .spawn()
        .map(drop)
        .map_err(|error| not_shown(error.to_string()))
}

/// Opens the folder of `file` in the system's file manager, with the file selected where the
/// file manager takes a selection.
pub fn reveal_file(file: &Utf8Path) -> Result<(), AppError> {
    let not_shown = |detail: String| AppError::NotShown {
        path: file.to_string(),
        detail,
    };
    if !file.is_file() {
        return Err(not_shown("no such file".to_string()));
    }
    let mut command = Command::new(FILE_MANAGER);
    if cfg!(windows) {
        // One argument: Explorer reads `/select,` and the path split apart as two folders.
        let mut select = std::ffi::OsString::from("/select,");
        select.push(native(file));
        command.arg(select);
    } else if cfg!(target_os = "macos") {
        command.arg("-R").arg(native(file));
    } else {
        let dir = file
            .parent()
            .ok_or_else(|| not_shown("no folder".to_string()))?;
        command.arg(native(dir));
    }
    command
        .spawn()
        .map(drop)
        .map_err(|error| not_shown(error.to_string()))
}

/// The path as the platform spells it: Explorer reads a path with `/` as no path and opens
/// the user's documents.
fn native(path: &Utf8Path) -> PathBuf {
    path.as_std_path().components().collect()
}
