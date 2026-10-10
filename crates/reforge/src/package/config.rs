//! The project's config: what a mod project says of the mod it holds, made from what the
//! archive says of itself.

use ltk_mod_project::{ModProject, ModProjectAuthor, ModProjectHashtable, ModProjectLayer};
use serde::{Deserialize, Serialize};

use super::Layer;
use crate::model::entity::{CharKind, ModMeta};
use crate::model::{ChunkPath, Mod, Spelled};

/// The longest project name. The name is a directory's and, with the version and an extension
/// after it, a file's: this leaves room for both under the 255 bytes a file name may have.
pub const MAX_PROJECT_NAME: usize = 120;

/// Where the table of chunk paths is, in a project.
pub const PATHS_TABLE: &str = "hashes/game.hashes.txt";
/// Where the table of entry names is, in a project.
pub const ENTRIES_TABLE: &str = "hashes/binentries.hashes.txt";

/// What a run did to the mod it packages. Its name in a package's evidence is the kind of run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Made {
    /// The mod as it is, fixed for the target build.
    #[serde(rename = "repair")]
    Repaired,
    /// The mod over the game's own skin.
    #[serde(rename = "port")]
    Ported,
}

impl Made {
    fn verb(self) -> &'static str {
        match self {
            Made::Repaired => "Repaired",
            Made::Ported => "Ported",
        }
    }
}

/// What a mod project says of the mod it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// The project's name: letters, digits, `_` and `-`. A writer names a directory and a
    /// file by it.
    pub name: String,
    /// The mod's name for a person.
    pub display_name: Spelled,
    /// The version, `major.minor.patch`.
    pub version: String,
    /// The mod's description, then what made the project and from which archive.
    pub description: Spelled,
    /// The mod's author, when the archive names one.
    pub authors: Vec<Spelled>,
    /// The champions the mod is for, as entry names spell them.
    pub champions: Vec<String>,
}

/// The letters, digits, `_` and `-` of a text, when any is left.
fn slug(text: Option<&Spelled>) -> Option<String> {
    let kept = |c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-';
    let slug: String = text?.raw().chars().filter(kept).collect();
    (!slug.is_empty()).then_some(slug)
}

/// The project's name: the mod's name and author, else the mod id.
fn project_name(meta: &ModMeta) -> String {
    let name = slug(meta.name.as_ref()).unwrap_or_else(|| meta.mod_id.clone());
    let mut name = match slug(meta.author.as_ref()) {
        Some(author) => format!("{name}-{author}"),
        None => name,
    };
    name.truncate(MAX_PROJECT_NAME);
    // A device name of Windows (`con`, `nul`) is no directory a writer can make.
    match ChunkPath::parse(&name) {
        Ok(_) => name,
        Err(_) => meta.mod_id.clone(),
    }
}

/// The archive's version when it is `major.minor.patch`, else `1.0.0`. Each number is written
/// without leading zeros, as a package's version must be.
fn version(meta: &ModMeta) -> String {
    let text = meta.version.as_ref().map_or("", Spelled::raw).trim();
    let text = text.trim_start_matches('v');
    let numbers: Option<Vec<u64>> = text
        .split('.')
        .map(|part| {
            let digits = !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
            digits.then(|| part.parse().ok()).flatten()
        })
        .collect();
    match numbers.as_deref() {
        Some([major, minor, patch]) => format!("{major}.{minor}.{patch}"),
        _ => "1.0.0".to_string(),
    }
}

impl ProjectConfig {
    /// The config of a mod's package, from what the archive says of itself.
    pub fn of(m: &Mod, made: Made) -> Self {
        let meta = &m.meta;
        let origin = format!(
            "{} by reforge from {} ({}).",
            made.verb(),
            meta.file_name.raw(),
            meta.mod_id
        );
        let own = meta.description.as_ref().map(|text| text.raw().trim());
        let description = match own.filter(|text| !text.is_empty()) {
            Some(text) => format!("{text}\n\n{origin}"),
            None => origin,
        };
        let champions = m.characters.values();
        let champions = champions.filter(|character| character.kind == CharKind::Champion);
        Self {
            name: project_name(meta),
            display_name: meta.name.clone().unwrap_or_else(|| meta.file_name.clone()),
            version: version(meta),
            description: Spelled::new(description),
            authors: meta.author.iter().cloned().collect(),
            champions: champions.map(|c| c.spelled.clone()).collect(),
        }
    }

    /// The file name of the packed mod: `<name>_<version>.modpkg`.
    pub fn modpkg_name(&self) -> String {
        format!("{}_{}.modpkg", self.name, self.version)
    }

    /// The config as a mod project holds it, with the layers of a package after the base
    /// layer in their order, the two hash tables, and the thumbnail's file when the project
    /// has one.
    pub fn to_project<'a>(
        &self,
        layers: impl Iterator<Item = &'a Layer>,
        thumbnail: Option<&str>,
    ) -> ModProject {
        let others = layers.filter(|layer| **layer != Layer::Base);
        let others = others.enumerate().map(|(i, layer)| ModProjectLayer {
            name: layer.name(),
            display_name: Some(layer.title()),
            priority: i32::try_from(i + 1).unwrap_or(i32::MAX),
            description: Some(layer.description()),
            ..Default::default()
        });
        let author = |author: &Spelled| ModProjectAuthor::Name(author.raw().to_string());
        ModProject {
            name: self.name.clone(),
            display_name: self.display_name.raw().to_string(),
            version: self.version.clone(),
            description: self.description.raw().to_string(),
            authors: self.authors.iter().map(author).collect(),
            license: None,
            tags: Vec::new(),
            champions: self.champions.clone(),
            maps: Vec::new(),
            transformers: Vec::new(),
            layers: std::iter::once(ModProjectLayer::base())
                .chain(others)
                .collect(),
            thumbnail: thumbnail.map(str::to_string),
            hashtables: vec![
                ModProjectHashtable {
                    path: PATHS_TABLE.into(),
                    category: ltk_hashtable::Category::Game,
                    algorithm: ltk_hashtable::Algorithm::Xxh64,
                    bits: 64,
                },
                ModProjectHashtable {
                    path: ENTRIES_TABLE.into(),
                    category: ltk_hashtable::Category::BinEntries,
                    algorithm: ltk_hashtable::Algorithm::Fnv1a32,
                    bits: 32,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ContentId;

    fn meta(name: Option<&str>, author: Option<&str>, version: Option<&str>) -> ModMeta {
        ModMeta {
            file_name: Spelled::new("mod.fantome"),
            sha256: ContentId([0; 32]),
            mod_id: "0123456789ab".to_string(),
            name: name.map(Spelled::new),
            author: author.map(Spelled::new),
            version: version.map(Spelled::new),
            description: None,
        }
    }

    #[test]
    fn a_project_is_named_by_the_letters_of_the_mods_name_and_author() {
        let name = |name, author| project_name(&meta(name, author, None));
        assert_eq!(
            name(Some("High Noon: Lux!"), Some("an author")),
            "HighNoonLux-anauthor"
        );
        assert_eq!(name(Some("Lux"), None), "Lux");
        assert_eq!(name(Some("../.."), Some("***")), "0123456789ab");
        assert_eq!(name(None, Some("someone")), "0123456789ab-someone");
        assert_eq!(name(Some("NUL"), None), "0123456789ab");
        assert_eq!(name(Some(&"x".repeat(400)), None).len(), MAX_PROJECT_NAME);
    }

    #[test]
    fn a_version_is_three_numbers_or_the_first() {
        let of = |text| version(&meta(None, None, text));
        assert_eq!(of(Some("2.10.3")), "2.10.3");
        assert_eq!(of(Some(" v1.2.3 ")), "1.2.3");
        assert_eq!(of(Some("01.02.03")), "1.2.3");
        assert_eq!(of(Some("1.2")), "1.0.0");
        assert_eq!(of(Some("1.2.x")), "1.0.0");
        assert_eq!(of(Some("1.2.+3")), "1.0.0");
        assert_eq!(of(None), "1.0.0");
    }
}
