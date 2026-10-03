//! Champion reworks (#GAME-CHANGE-0001), as far as the file set shows them: the mod's files at a
//! reworked champion's own paths, counted against the build of its rework. A mod that restores
//! the model from before the rework ships files on both sides of it, which `load` has to know
//! before it calls any file a stale copy.

use std::collections::BTreeMap;

use crate::history::{History, HistoryError, Position};
use crate::model::Mod;
use crate::model::entity::{FileId, FileSource};

/// #GAME-CHANGE-0001: each reworked champion and the patch its new model shipped in.
const REWORKS: &[(&str, &str)] = &[
    ("ahri", "13.3"),
    ("caitlyn", "11.23"),
    ("drmundo", "11.12"),
    ("fiddlesticks", "10.7"),
    ("jax", "13.20"),
    ("leblanc", "15.7"),
    ("leesin", "14.9"),
    ("mordekaiser", "9.12"),
    ("pantheon", "9.16"),
    ("shyvana", "16.6"),
    ("skarner", "14.7"),
    ("teemo", "14.20"),
    ("udyr", "12.16"),
    ("viktor", "14.24"),
    ("volibear", "10.11"),
    ("xinzhao", "15.17"),
];

/// The champions whose model was replaced, each with the patch the new model shipped in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReworkTable {
    patches: BTreeMap<String, String>,
}

impl ReworkTable {
    /// The reworks within the history (#GAME-CHANGE-0001).
    pub fn standard() -> Self {
        Self::new(REWORKS.iter().copied())
    }

    /// A table of these champions and patches, `("ahri", "13.3")`.
    pub fn new<C: AsRef<str>, P: Into<String>>(rows: impl IntoIterator<Item = (C, P)>) -> Self {
        let lowered = rows
            .into_iter()
            .map(|(champion, patch)| (champion.as_ref().to_ascii_lowercase(), patch.into()));
        Self {
            patches: lowered.collect(),
        }
    }

    /// The patch a champion's new model shipped in.
    pub fn patch(&self, champion: &str) -> Option<&str> {
        self.patches
            .get(&champion.to_ascii_lowercase())
            .map(String::as_str)
    }
}

impl Default for ReworkTable {
    fn default() -> Self {
        Self::standard()
    }
}

/// A reworked champion's files in the mod, against the rework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Around {
    /// The patch the new model shipped in.
    pub patch: String,
    /// The position of the rework's first build.
    pub at: Position,
    /// Files at the champion's paths the game dropped before the rework.
    pub older: u32,
    /// Files at the champion's paths the game shipped only from the rework on.
    pub newer: u32,
    /// One of the older files.
    pub example: Option<FileId>,
}

impl Around {
    /// The archive's files at a champion's own paths, counted against its rework. `None` when
    /// the champion has no rework, or the history holds no build of the rework's patch.
    ///
    /// A file a fixup made and a bin taken from the game are not the archive's and are not
    /// counted.
    pub fn of(
        m: &Mod,
        history: &dyn History,
        table: &ReworkTable,
        champion: &str,
    ) -> Result<Option<Self>, HistoryError> {
        let name = champion.to_ascii_lowercase();
        let Some(patch) = table.patch(&name) else {
            return Ok(None);
        };
        let Some(at) = history.first_of_patch(patch) else {
            return Ok(None);
        };
        let folders = [
            format!("assets/characters/{name}/"),
            format!("data/characters/{name}/"),
        ];
        let mut around = Self {
            patch: patch.to_string(),
            at,
            older: 0,
            newer: 0,
            example: None,
        };
        for (id, file) in m.files.iter() {
            let own = file.source == FileSource::Archive;
            let Some(path) = file.path.as_ref().filter(|_| own) else {
                continue;
            };
            if !folders
                .iter()
                .any(|folder| path.as_str().starts_with(folder))
            {
                continue;
            }
            let Some(hit) = history.path(file.hash)? else {
                continue;
            };
            if hit.life.last < at {
                around.older += 1;
                around.example.get_or_insert(id);
            } else if hit.life.first >= at {
                around.newer += 1;
            }
        }
        Ok(Some(around))
    }

    /// Whether the mod restores the old model after the rework: files at paths the rework
    /// dropped beside files at paths that exist only since. Work made before the rework has no
    /// file of the second kind.
    pub fn restores(&self) -> bool {
        self.older > 0 && self.newer > 0
    }
}
