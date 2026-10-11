//! The game installs on this machine and the ones a person added, one of them chosen
//! (#ADR-0101). An install is listed by its build alone: none is opened to list it.

use std::cmp::Ordering;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::game::Install;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, chain};
use crate::settings::{Settings, SettingsStore};

/// The folder the Riot client installs the live game in.
const LIVE_FOLDER: &str = "League of Legends";

/// The folder the Riot client installs the PBE in.
const PBE_FOLDER: &str = "League of Legends (PBE)";

/// Where an install was found, in the order the list shows them.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "kebab-case")]
pub enum InstallSource {
    /// The live game, in the Riot client's list or the usual folders.
    Live,
    /// The PBE, in the Riot client's list or the usual folders.
    Pbe,
    /// The install of a running client, in a folder of another name.
    Running,
    /// A folder a person added.
    Added,
}

impl InstallSource {
    /// The source an install's folder name says: the live game's, the PBE's, or neither.
    fn of_folder(root: &Utf8Path) -> Option<Self> {
        let name = root.file_name()?;
        if name.eq_ignore_ascii_case(LIVE_FOLDER) {
            Some(Self::Live)
        } else if name.eq_ignore_ascii_case(PBE_FOLDER) {
            Some(Self::Pbe)
        } else {
            None
        }
    }
}

/// One install as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct InstallView {
    /// The install's root, as the settings hold it.
    pub path: String,
    /// Where it was found.
    pub source: InstallSource,
    /// The build it says it is. None when it does not open or does not say.
    pub build: Option<String>,
    /// Whether it is an install: its data folder is there.
    pub readable: bool,
    /// Whether it is the install a run reads.
    pub chosen: bool,
}

impl InstallView {
    /// The install at `path`, its build read without opening it.
    fn peek(path: Utf8PathBuf, source: InstallSource) -> Self {
        let peeked = Install::peek_build(&path);
        Self {
            build: peeked
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .map(ToString::to_string),
            readable: peeked.is_ok(),
            path: path.into_string(),
            source,
            chosen: false,
        }
    }
}

/// An install found, before its build is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The install's root, normalised.
    pub path: Utf8PathBuf,
    /// Where it was found.
    pub source: InstallSource,
}

/// The path as the system writes it, without a trailing separator. A drive's or the file
/// system's root keeps its separator.
pub fn normalise(path: &str) -> Utf8PathBuf {
    let native = if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.to_owned()
    };
    let trimmed = native.trim_end_matches(std::path::MAIN_SEPARATOR);
    if trimmed.is_empty() || trimmed.ends_with(':') {
        return Utf8PathBuf::from(native);
    }
    Utf8PathBuf::from(trimmed)
}

/// Whether two paths name one folder: normalised, and on Windows regardless of case.
pub fn same_path(a: &str, b: &str) -> bool {
    let (a, b) = (normalise(a), normalise(b));
    if cfg!(windows) {
        a.as_str().eq_ignore_ascii_case(b.as_str())
    } else {
        a == b
    }
}

/// Whether `<root>/Game/League of Legends.exe` exists, as the Riot client lays an install out.
fn has_game(root: &Utf8Path) -> bool {
    root.join("Game").join("League of Legends.exe").is_file()
}

/// The live and PBE installs the Riot client lists in `file`, its `RiotClientInstalls.json`.
/// An entry of another product, or with no game in it, is skipped. A file that is absent or
/// does not read lists none.
pub fn riot_client_installs(file: &Utf8Path) -> Vec<Found> {
    #[derive(Deserialize)]
    struct Installs {
        #[serde(default)]
        associated_client: serde_json::Map<String, serde_json::Value>,
    }

    let Ok(text) = fs::read_to_string(file) else {
        return Vec::new();
    };
    let Ok(installs) = serde_json::from_str::<Installs>(&text) else {
        return Vec::new();
    };
    installs
        .associated_client
        .keys()
        .filter_map(|key| {
            let path = normalise(key);
            let source = InstallSource::of_folder(&path)?;
            has_game(&path).then_some(Found { path, source })
        })
        .collect()
}

/// The Riot client's list of installs on this machine.
fn riot_client_file() -> Option<Utf8PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    Some(
        Utf8PathBuf::from(format!("{drive}\\"))
            .join("ProgramData")
            .join("Riot Games")
            .join("RiotClientInstalls.json"),
    )
}

/// The install `ltk_mod_core` finds: a running client's, when the Riot client's list names
/// none. Live or PBE by its folder's name, else running.
fn running() -> Option<Found> {
    let exe = ltk_mod_core::auto_detect_league_path()?;
    // The detection names `<install>/Game/League of Legends.exe`.
    let path = normalise(exe.parent()?.parent()?.as_str());
    let source = InstallSource::of_folder(&path).unwrap_or(InstallSource::Running);
    Some(Found { path, source })
}

/// The installs in the usual folders on every drive: `Riot Games` at the drive's root and
/// under `Program Files` and `Program Files (x86)`.
fn usual_folders() -> Vec<Found> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut found = Vec::new();
    for disk in disks.list() {
        let Some(drive) = disk.mount_point().to_str() else {
            continue;
        };
        let drive = Utf8Path::new(drive);
        for parent in [
            drive.to_owned(),
            drive.join("Program Files"),
            drive.join("Program Files (x86)"),
        ] {
            for (folder, source) in [
                (LIVE_FOLDER, InstallSource::Live),
                (PBE_FOLDER, InstallSource::Pbe),
            ] {
                let path = normalise(parent.join("Riot Games").join(folder).as_str());
                if has_game(&path) {
                    found.push(Found { path, source });
                }
            }
        }
    }
    found
}

/// Installs, each listed once, in the order first found.
#[derive(Default)]
struct Listing(Vec<Found>);

impl Listing {
    fn contains(&self, path: &str) -> bool {
        self.0
            .iter()
            .any(|found| same_path(found.path.as_str(), path))
    }

    fn push(&mut self, found: Found) {
        if !self.contains(found.path.as_str()) {
            self.0.push(found);
        }
    }

    fn views(self) -> Vec<InstallView> {
        self.0
            .into_iter()
            .map(|found| InstallView::peek(found.path, found.source))
            .collect()
    }
}

/// The installs detected on this machine: the Riot client's list, a running client, then the
/// usual folders on every drive. None is chosen.
pub fn detect() -> Vec<InstallView> {
    let mut listing = Listing::default();
    for found in riot_client_file()
        .map(|file| riot_client_installs(&file))
        .unwrap_or_default()
    {
        listing.push(found);
    }
    if let Some(found) = running() {
        listing.push(found);
    }
    for found in usual_folders() {
        listing.push(found);
    }
    listing.views()
}

/// The order of the list: the chosen install first, then by source, then by path.
fn order(a: &InstallView, b: &InstallView) -> Ordering {
    b.chosen
        .cmp(&a.chosen)
        .then(a.source.cmp(&b.source))
        .then_with(|| a.path.cmp(&b.path))
}

/// The detected installs with the ones the settings list as added, the chosen one marked, in
/// the list's order.
///
/// An added install that was detected keeps its detected source. A chosen install no source
/// lists, as a settings file written before the list holds, is listed as added.
pub fn list(detected: Vec<InstallView>, settings: &Settings) -> Vec<InstallView> {
    let mut views = detected;
    let listed = |views: &[InstallView], path: &str| views.iter().any(|v| same_path(&v.path, path));
    for path in settings.installs.iter().chain(&settings.game) {
        if !listed(&views, path) {
            views.push(InstallView::peek(normalise(path), InstallSource::Added));
        }
    }
    for view in &mut views {
        view.chosen = settings
            .game
            .as_deref()
            .is_some_and(|game| same_path(&view.path, game));
    }
    views.sort_by(order);
    views
}

/// Whether `path` is one of the listed installs.
pub fn is_listed(detected: Vec<InstallView>, settings: &Settings, path: &str) -> bool {
    list(detected, settings)
        .iter()
        .any(|view| same_path(&view.path, path))
}

/// Adds the folder at `path` to the installs added by hand, and chooses it when none is
/// chosen. Refused with [`AppError::GameUnreadable`] when the folder is no install.
pub fn add(
    store: &SettingsStore,
    detected: Vec<InstallView>,
    path: &str,
) -> Result<Vec<InstallView>, AppError> {
    let path = normalise(path);
    Install::peek_build(&path).map_err(|error| AppError::GameUnreadable {
        path: path.to_string(),
        detail: chain(&error),
    })?;
    let mut settings = store.get();
    if !settings
        .installs
        .iter()
        .any(|added| same_path(added, path.as_str()))
    {
        settings.installs.push(path.to_string());
    }
    if settings.game.is_none() {
        settings.game = Some(path.into_string());
    }
    let settings = store.save(settings)?;
    Ok(list(detected, &settings))
}

/// Takes the install at `path` off the installs added by hand. A detected install stays. When
/// the install taken off was the chosen one, the first of the list is chosen, or none.
pub fn remove(
    store: &SettingsStore,
    detected: Vec<InstallView>,
    path: &str,
) -> Result<Vec<InstallView>, AppError> {
    let mut settings = store.get();
    if detected.iter().any(|view| same_path(&view.path, path)) {
        return Ok(list(detected, &settings));
    }
    settings.installs.retain(|added| !same_path(added, path));
    if settings
        .game
        .as_deref()
        .is_some_and(|game| same_path(game, path))
    {
        let rest = list(
            detected.clone(),
            &Settings {
                game: None,
                ..settings.clone()
            },
        );
        settings.game = rest.into_iter().next().map(|view| view.path);
    }
    let settings = store.save(settings)?;
    Ok(list(detected, &settings))
}

/// Chooses the listed install at `path` as the one a run reads. Refused with
/// [`AppError::GameUnreadable`] for a path the list does not hold.
pub fn choose(
    store: &SettingsStore,
    detected: Vec<InstallView>,
    path: &str,
) -> Result<Settings, AppError> {
    let mut settings = store.get();
    let Some(view) = list(detected, &settings)
        .into_iter()
        .find(|view| same_path(&view.path, path))
    else {
        return Err(AppError::GameUnreadable {
            path: path.to_string(),
            detail: "not a listed install".to_string(),
        });
    };
    settings.game = Some(view.path);
    store.save(settings)
}
