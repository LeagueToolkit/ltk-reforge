//! Whether a champion's work was made for the model Riot replaced in a rework
//! (#GAME-CHANGE-0001, #ADR-0019).
//!
//! The evidence is the mod's files at the champion's own paths against the history's path
//! lives, and the mod's dating. A path the game shipped only from the rework on, or a dating
//! at or after it, says the mod is newer. Otherwise a path the game dropped before the rework
//! says it is older. Without a history nothing is judged.

use std::collections::BTreeMap;

use serde::Serialize;

use super::FactsError;
use crate::fixups::FixedMod;
use crate::history::History;
use crate::load::{Around, Dating, ReworkTable};
use crate::model::Mod;
use crate::model::entity::{CharId, CharKind, FileId};

/// When a mod was made, against a champion's rework.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Made {
    /// For the model before the rework.
    Before,
    /// After the rework.
    After,
    /// Nothing says.
    Unknown,
}

impl Made {
    /// The verdict on a champion's files around its rework and the mod's dating.
    pub fn judge(around: &Around, dating: &Dating) -> Self {
        let dated_after = dating.position().is_some_and(|at| at >= around.at);
        if around.newer > 0 || dated_after {
            Made::After
        } else if around.older > 0 {
            Made::Before
        } else {
            Made::Unknown
        }
    }
}

/// One reworked champion of the mod, judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Judged {
    /// The champion.
    pub character: CharId,
    /// The patch its new model shipped in.
    pub rework: String,
    /// When the mod was made.
    pub made: Made,
    /// The mod's files at the champion's paths the game dropped before the rework.
    pub older: u32,
    /// The mod's files at the champion's paths the game shipped only from the rework on.
    pub newer: u32,
    /// One of the older files.
    pub example: Option<FileId>,
}

/// The mod's champions that have a rework within the history, each judged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reworks {
    champions: BTreeMap<CharId, Judged>,
}

impl Reworks {
    /// The reworked champions of a mod, each judged against its rework.
    pub fn find(
        fixed: &FixedMod,
        history: &dyn History,
        dating: &Dating,
        table: &ReworkTable,
    ) -> Result<Self, FactsError> {
        let m: &Mod = fixed;
        let mut out = Self::default();
        let champions = m
            .characters
            .iter()
            .filter(|(_, c)| c.kind == CharKind::Champion);
        for (id, champion) in champions {
            let Some(around) = Around::of(m, history, table, &champion.name)? else {
                continue;
            };
            let judged = Judged {
                character: id,
                made: Made::judge(&around, dating),
                rework: around.patch,
                older: around.older,
                newer: around.newer,
                example: around.example,
            };
            out.champions.insert(id, judged);
        }
        Ok(out)
    }

    /// A champion's judgement, when the mod was made for its model before the rework.
    pub fn before(&self, champion: CharId) -> Option<&Judged> {
        self.champions
            .get(&champion)
            .filter(|judged| judged.made == Made::Before)
    }

    /// Every judged champion, in character order.
    pub fn iter(&self) -> impl Iterator<Item = &Judged> {
        self.champions.values()
    }
}
