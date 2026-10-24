//! LTK Manager's mod library, read for the Batch page (#ADR-0106): `library.json` in the
//! manager's data folder, each mod's config under `mods/<slug>/`, and the archive a mod stored
//! as one keeps beside its folder. Read only: nothing under the folder is written or locked.
//!
//! The manager's files are another program's and may be of a shape this reader does not know:
//! a document it cannot read lists nothing, a mod it cannot place has no archive. A mod's name
//! is the manager's text and is cleaned.

use std::io::Read as _;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::model::clean;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// The manager's folder under the platform's data directory: its Tauri identifier.
pub const MANAGER_FOLDER: &str = "dev.leaguetoolkit.manager";

/// The most of an archive's metadata that is read to tell its layout: `META/info.json` is a
/// small document, and an archive is untrusted.
const INFO_BYTES: u64 = 1 << 20;

/// The form a mod's archive takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum ManagerFormat {
    /// A `.fantome`.
    Fantome,
    /// A `.modpkg`.
    Modpkg,
    /// A form this reader does not name.
    Other,
}

impl ManagerFormat {
    fn extension(self) -> Option<&'static str> {
        match self {
            ManagerFormat::Fantome => Some("fantome"),
            ManagerFormat::Modpkg => Some("modpkg"),
            ManagerFormat::Other => None,
        }
    }
}

/// One mod of the manager's library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ManagerMod {
    /// The manager's name for the mod's folder.
    pub slug: String,
    /// The mod's display name, cleaned; the slug when its config does not say.
    pub name: String,
    /// The mod's version, as its config says.
    pub version: Option<String>,
    /// The form its archive takes.
    pub format: ManagerFormat,
    /// The archive the mod keeps. None for a mod stored as an unpacked project, or one whose
    /// archive is not where the layout puts it.
    pub archive: Option<String>,
    /// Whether the archive carries league-mod's layer directories, game data or hash tables
    /// (#MOD-ARTIFACT-0040). False for a plain Fantome, and for a mod with no archive.
    pub extended: bool,
    /// When the manager installed it, as its index spells the time.
    pub installed_at: String,
    /// The champions the manager found the mod's WADs to be of, cleaned. Empty when the
    /// manager has no report of the mod.
    pub champions: Vec<String>,
    /// The maps the manager found the mod's WADs to be of, cleaned.
    pub maps: Vec<String>,
    /// The manager's tags for the mod, `champion-skin` among them, cleaned.
    pub tags: Vec<String>,
}

/// The manager's library as read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ManagerLibrary {
    /// The manager's data folder.
    pub folder: String,
    /// Every mod the index lists, in its order.
    pub mods: Vec<ManagerMod>,
}

/// `library.json` as far as this reader needs it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Index {
    mods: Vec<IndexEntry>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct IndexEntry {
    id: String,
    slug: String,
    format: String,
    storage: String,
    installed_at: String,
}

/// `wad-reports.json` as far as this reader needs it: what the manager derived of each mod
/// from the game's WADs it touches, by the mod's id.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Reports {
    reports: std::collections::HashMap<String, Report>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Report {
    derived: Derived,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Derived {
    champions: Vec<String>,
    maps: Vec<String>,
    tags: Vec<String>,
}

/// `mod.config.json` as far as this reader needs it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Config {
    name: String,
    display_name: String,
    version: String,
}

/// Where the manager keeps its library under the platform's data directory.
pub fn library_folder(data_dir: &Utf8Path) -> Utf8PathBuf {
    data_dir.join(MANAGER_FOLDER)
}

/// The library in the manager's data folder.
///
/// # Errors
///
/// `ManagerAbsent` when the folder holds no `library.json`, `FolderUnreadable` when the
/// index does not read as the manager writes it.
pub fn read_library(folder: &Utf8Path) -> Result<ManagerLibrary, AppError> {
    let index = folder.join("library.json");
    if !index.is_file() {
        return Err(AppError::ManagerAbsent {
            path: folder.to_string(),
        });
    }
    let unreadable = |detail: String| AppError::FolderUnreadable {
        path: folder.to_string(),
        detail,
    };
    let text = fs::read_to_string(&index).map_err(|error| unreadable(error.to_string()))?;
    let index: Index =
        serde_json::from_str(&text).map_err(|error| unreadable(error.to_string()))?;
    // The reports are the manager's analysis and may lag the index: none is no fault.
    let reports: Reports = fs::read_to_string(folder.join("wad-reports.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let mods = index
        .mods
        .iter()
        .map(|entry| {
            let derived = reports.reports.get(&entry.id).map(|report| &report.derived);
            read_mod(folder, entry, derived)
        })
        .collect();
    Ok(ManagerLibrary {
        folder: folder.to_string(),
        mods,
    })
}

fn read_mod(folder: &Utf8Path, entry: &IndexEntry, derived: Option<&Derived>) -> ManagerMod {
    let cleaned = |names: &[String]| names.iter().map(|name| clean(name).into_owned()).collect();
    let (champions, maps, tags) = derived.map_or((Vec::new(), Vec::new(), Vec::new()), |d| {
        (cleaned(&d.champions), cleaned(&d.maps), cleaned(&d.tags))
    });
    let format = match entry.format.as_str() {
        "fantome" => ManagerFormat::Fantome,
        "modpkg" => ManagerFormat::Modpkg,
        _ => ManagerFormat::Other,
    };
    let config = read_config(
        &folder
            .join("mods")
            .join(&entry.slug)
            .join("mod.config.json"),
    );
    let name = [&config.display_name, &config.name, &entry.slug]
        .into_iter()
        .find(|name| !name.trim().is_empty())
        .map_or_else(String::new, |name| clean(name).into_owned());
    let version = Some(config.version)
        .filter(|version| !version.trim().is_empty())
        .map(|version| clean(&version).into_owned());
    let archive = archive_of(folder, entry, format);
    let extended = archive.as_deref().is_some_and(is_extended);
    ManagerMod {
        slug: clean(&entry.slug).into_owned(),
        name,
        version,
        format,
        archive: archive.map(Utf8PathBuf::into_string),
        extended,
        installed_at: clean(&entry.installed_at).into_owned(),
        champions,
        maps,
        tags,
    }
}

fn read_config(path: &Utf8Path) -> Config {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// The archive of a mod stored as one: beside its folder under `mods/`, else under the older
/// layout's `archives/` by id. None for a mod stored as a project.
fn archive_of(folder: &Utf8Path, entry: &IndexEntry, format: ManagerFormat) -> Option<Utf8PathBuf> {
    if entry.storage != "archive" {
        return None;
    }
    let extension = format.extension()?;
    let places = [
        folder
            .join("mods")
            .join(format!("{}.{extension}", entry.slug)),
        folder
            .join("archives")
            .join(format!("{}.{extension}", entry.id)),
    ];
    places.into_iter().find(|place| place.is_file())
}

/// Whether an archive carries league-mod's extensions to the Fantome layout
/// (#MOD-ARTIFACT-0040): a layer directory, game data or hash tables among its entries, or
/// a `Generator` in its metadata. An archive that does not open is not extended.
fn is_extended(path: &Utf8Path) -> bool {
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let extended_entry = |name: &str| {
        let lower = name.to_ascii_lowercase();
        lower.starts_with("wad_")
            || lower.starts_with("meta/game_data/")
            || lower.starts_with("meta/hashes/")
    };
    if archive.file_names().any(extended_entry) {
        return true;
    }
    let Ok(info) = archive.by_name("META/info.json") else {
        return false;
    };
    let mut text = String::new();
    if info.take(INFO_BYTES).read_to_string(&mut text).is_err() {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .is_some_and(|info| info.get("Generator").is_some())
}
