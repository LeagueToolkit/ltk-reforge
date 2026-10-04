//! The numbered records under `docs/`, as code names them: `#GAME-CHANGE-0001`.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A series of numbered records under `docs/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Series {
    /// A change Riot made to the game.
    GameChange,
    /// How the game client behaves.
    GameBehavior,
    /// What tools, mod managers and authors' practices put in mods.
    ModArtifact,
}

/// A numbered record, as a comment or a doc cites it: `#GAME-CHANGE-0001`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Record {
    /// The series the record is in.
    pub series: Series,
    /// Its number in the series.
    pub number: u16,
}

impl Record {
    /// A game change, by its number.
    pub const fn game_change(number: u16) -> Self {
        Self {
            series: Series::GameChange,
            number,
        }
    }

    /// A game behavior, by its number.
    pub const fn game_behavior(number: u16) -> Self {
        Self {
            series: Series::GameBehavior,
            number,
        }
    }

    /// A mod artifact, by its number.
    pub const fn mod_artifact(number: u16) -> Self {
        Self {
            series: Series::ModArtifact,
            number,
        }
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let series = match self.series {
            Series::GameChange => "GAME-CHANGE",
            Series::GameBehavior => "GAME-BEHAVIOR",
            Series::ModArtifact => "MOD-ARTIFACT",
        };
        write!(f, "#{series}-{:04}", self.number)
    }
}
