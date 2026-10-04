//! The fixups and the checks as the front end lists them. The library's types carry no
//! `specta::Type`, since `reforge` holds nothing a UI needs alone (#ADR-0046), so these mirror
//! them on the wire.

use reforge::fixups::FixupSet;
use reforge::model::{FixupInfo, Phase};
use reforge::package::{CheckInfo, CheckSet};
use serde::Serialize;

/// A fixup, as the list of fixups shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct FixupEntry {
    /// The name it is turned on and off by.
    pub id: String,
    /// One line for a person: what it does.
    pub title: String,
    /// The records it implements, by their codes: `#GAME-CHANGE-0005`.
    pub records: Vec<String>,
    /// Where it runs.
    pub phase: FixupPhase,
    /// Whether the standard set holds it.
    pub default: bool,
}

/// Where a fixup runs, as `reforge`'s `Phase` serialises.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "name", rename_all = "kebab-case")]
pub enum FixupPhase {
    /// On the file set, before characters and units are found.
    Files,
    /// A tool's damage taken out.
    Undo,
    /// A change of Riot's followed, from the build named on.
    Layout { build: String },
    /// Values brought to the target schema.
    Conform,
    /// What the target build does not declare, removed.
    Strip,
}

impl From<Phase> for FixupPhase {
    fn from(phase: Phase) -> Self {
        match phase {
            Phase::Files => Self::Files,
            Phase::Undo => Self::Undo,
            Phase::Layout { build } => Self::Layout {
                build: build.to_string(),
            },
            Phase::Conform => Self::Conform,
            Phase::Strip => Self::Strip,
        }
    }
}

impl From<&FixupInfo> for FixupEntry {
    fn from(info: &FixupInfo) -> Self {
        Self {
            id: info.id.to_string(),
            title: info.title.to_string(),
            records: info.records.iter().map(ToString::to_string).collect(),
            phase: info.phase.into(),
            default: info.default,
        }
    }
}

/// Every fixup of the library, in the order they run.
pub fn catalog() -> Vec<FixupEntry> {
    FixupSet::catalog().iter().map(FixupEntry::from).collect()
}

/// A check of the package, as the list of checks shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct CheckEntry {
    /// The name it is turned on and off by.
    pub id: String,
    /// One line for a person: what it checks.
    pub title: String,
    /// The record it rests on, by its code.
    pub record: Option<String>,
    /// Whether the standard set holds it.
    pub default: bool,
}

impl From<&CheckInfo> for CheckEntry {
    fn from(info: &CheckInfo) -> Self {
        Self {
            id: info.id.to_string(),
            title: info.title.to_string(),
            record: info.record.map(|record| record.to_string()),
            default: info.default,
        }
    }
}

/// Every check of the library, in the order they run.
pub fn checks() -> Vec<CheckEntry> {
    CheckSet::catalog().iter().map(CheckEntry::from).collect()
}
