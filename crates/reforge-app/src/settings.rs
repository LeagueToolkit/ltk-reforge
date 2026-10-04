//! The app's settings: one JSON file in the app's data directory, a key per setting id. A
//! setting id is dotted and named for its domain (`appearance.theme`), as LTK Manager's
//! settings are, and stays the same when the screen that shows it moves.

use std::path::{Path, PathBuf};

use fs_err as fs;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// The app's settings, as `settings.json` holds them.
///
/// A setting the file lacks takes its default, so a file an older app wrote still reads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Settings {
    /// The colour scheme.
    #[serde(rename = "appearance.theme")]
    pub theme: Theme,
    /// The game install a run is judged against: its folder, its `Game` folder or
    /// `DATA/FINAL`. None until it is found or picked.
    #[serde(rename = "inputs.game")]
    pub game: Option<String>,
    /// The hash cache's folder. None: the one LTK Manager keeps on this machine (#ADR-0048).
    #[serde(rename = "inputs.hashes")]
    pub hashes: Option<String>,
    /// The meta schema database. None: the copy LTK Manager keeps on this machine.
    #[serde(rename = "inputs.schema")]
    pub schema: Option<String>,
    /// The history index's folder. None: a run is judged against the install alone
    /// (#ADR-0047).
    #[serde(rename = "inputs.history")]
    pub history: Option<String>,
    /// The folder a package is written in. None: the mod's own folder.
    #[serde(rename = "output.folder")]
    pub output: Option<String>,
    /// Run with an input absent, the history index or the hash tables, without asking first.
    #[serde(rename = "prompts.runWithLess")]
    pub run_with_less: bool,
}

/// A colour scheme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    /// The system's.
    #[default]
    System,
    /// Dark.
    Dark,
    /// Light.
    Light,
}

/// The settings of a running app and the file they are saved in.
pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// The settings saved at `path`.
    ///
    /// A file that is absent or does not read gives the defaults: a person who cannot start
    /// the app cannot fix its settings. A file that does not read is overwritten at the next
    /// save.
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let current = Mutex::new(read(&path));
        Self { path, current }
    }

    /// The settings as they stand.
    pub fn get(&self) -> Settings {
        self.current.lock().clone()
    }

    /// Saves `settings` and makes them the current ones. They stay as they were when the file
    /// does not write.
    pub fn save(&self, settings: Settings) -> Result<Settings, AppError> {
        let mut current = self.current.lock();
        write(&self.path, &settings).map_err(|error| AppError::SettingsUnwritable {
            detail: error.to_string(),
        })?;
        *current = settings.clone();
        Ok(settings)
    }
}

fn read(path: &Path) -> Settings {
    if !path.exists() {
        return Settings::default();
    }
    let read = fs::read_to_string(path).map_err(|error| error.to_string());
    match read.and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string())) {
        Ok(settings) => settings,
        Err(error) => {
            tracing::warn!(%error, "the settings did not read; the defaults stand");
            Settings::default()
        }
    }
}

fn write(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(settings)?;
    // Written beside and renamed, so a crash mid-write leaves the last saved file.
    let partial = path.with_extension("json.partial");
    fs::write(&partial, text + "\n")?;
    fs::rename(&partial, path)
}
