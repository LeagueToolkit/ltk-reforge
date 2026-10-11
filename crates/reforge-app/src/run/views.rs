//! A run as the views show it, read from its report (`crate::view`): each stage's state, the
//! mod, the findings, what each fixup changed, the decisions of a port, and the package.

use std::collections::BTreeMap;

use reforge::Run;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use super::StageName;
use crate::view::{Finding, read_clean};

/// How far a stage got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum StageState {
    /// It ran to its end.
    Ran,
    /// A fatal finding ended the run in it.
    Halted,
    /// The run did not reach it.
    NotRun,
}

/// A stage of the run and how far it got.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct StageView {
    /// The stage.
    pub stage: StageName,
    /// How far it got.
    pub state: StageState,
    /// How long it took, in milliseconds, when it ran.
    pub millis: Option<u64>,
    /// Whether the run took it from a kept run's snapshots in place of running it. A kept
    /// view without the field reads as false.
    #[serde(default)]
    pub resumed: bool,
}

/// The mod, as its archive and `META/info.json` name it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ModView {
    /// The archive's file name.
    pub file_name: String,
    /// The first twelve hex digits of the archive's SHA-256.
    pub mod_id: String,
    /// Its name.
    #[serde(default)]
    pub name: Option<String>,
    /// Its author.
    #[serde(default)]
    pub author: Option<String>,
    /// Its version.
    #[serde(default)]
    pub version: Option<String>,
    /// Its description.
    #[serde(default)]
    pub description: Option<String>,
}

/// A value on one line, with its kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ValueView {
    /// The value's kind, as the meta schema names kinds.
    pub kind: String,
    /// The value on one line.
    pub text: String,
}

/// What a fixup left where a value was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "as", rename_all = "kebab-case")]
pub enum AfterView {
    /// The value in the form the target build reads.
    Value {
        /// Its kind.
        kind: String,
        /// The value on one line.
        text: String,
    },
    /// The same value under another field or in another struct.
    Moved {
        /// Where it is now.
        to: String,
    },
    /// Removed, with nothing lost.
    Removed,
    /// Removed, and the mod's own build read it.
    Lost,
    /// Left as it was: no conversion to the kind the target build reads.
    Unconvertible {
        /// The kind the target build reads there.
        wants: String,
    },
}

/// Which `fixup.*` finding a change counts under: its code is `fixup.` and this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum OutcomeView {
    /// A value converted to the form the target build reads.
    Changed,
    /// A value removed with nothing lost.
    Removed,
    /// A value removed that the mod's own build read.
    Lost,
    /// A value with no conversion, left as it is.
    Unconvertible,
}

/// What a fixup did at one place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ChangeView {
    /// The object the value is in, by its id in the report: `%o14`.
    pub object: String,
    /// The object's entry hash, as eight hex digits.
    pub entry: String,
    /// The class and the field the value was in, or for a value added the field it is in:
    /// `Class.field`, each spelled where a name is known. Empty for a change at the object
    /// itself.
    pub field: String,
    /// The finding the change counts under.
    pub outcome: OutcomeView,
    /// Whether a unit's skin entry reaches the object.
    pub reached: bool,
    /// Where the value was, its fields spelled.
    pub path: String,
    /// The value before. None for a value the fixup added.
    pub before: Option<ValueView>,
    /// What is there after.
    pub after: AfterView,
}

/// What one fixup did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct FixupChanges {
    /// The fixup's id.
    pub fixup: String,
    /// Values converted to the form the target build reads.
    pub changed: usize,
    /// Values removed with nothing lost.
    pub removed: usize,
    /// Values removed that the mod's own build read.
    pub lost: usize,
    /// Values with no conversion, left as they are.
    pub unconvertible: usize,
    /// Its changes, in the order made.
    pub changes: Vec<ChangeView>,
}

/// An object a fixup changed, as the run spells it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ObjectView {
    /// Its id in the report: `%o14`.
    pub id: String,
    /// Its entry name, else its hash as eight hex digits.
    pub entry: String,
    /// Its class's name, else the hash as eight hex digits.
    pub class: String,
    /// The bin it is in: the bin's path, else its hash as sixteen hex digits.
    pub bin: String,
}

/// What a stage gave the views, sent to the window as the stage ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "part", rename_all = "kebab-case")]
pub enum PartView {
    /// The names of the report's tables, after `load`: what a finding raised from then on
    /// names by id (`%w0`, `%f12`, `%c0`, `%u0`).
    Names {
        /// A name for each id, as `RunView::names` holds them.
        names: BTreeMap<String, String>,
    },
    /// What the fixups changed, after `fix`.
    Changes {
        /// What each fixup did, in the order they ran.
        changes: Vec<FixupChanges>,
        /// The objects they changed.
        objects: Vec<ObjectView>,
    },
    /// The decisions, after `decide`.
    Targets {
        /// The decisions.
        targets: TargetsView,
    },
    /// The package, after `build`.
    Package {
        /// The package.
        package: PackageView,
    },
}

/// A policy: how a target is chosen, and how much animation reuse is accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct PolicyView {
    /// `auto`, `base`, `origin`, `slot` or a skin number.
    pub target: String,
    /// `block`, `same-joints`, `own-rig` or `allow`.
    pub anim_reuse: String,
}

/// Where a unit's work comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct OriginView {
    /// `base`, `premium`, `custom` or `unknown`.
    pub verdict: String,
    /// The premium skin, for a premium origin.
    #[serde(default)]
    pub slot: Option<u16>,
}

/// One check of a candidate slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CheckView {
    /// What is checked: `mesh`, `submeshes`, `skeleton`, `clips`.
    pub aspect: String,
    /// How it fits: `fits`, or what rules the slot out.
    pub fit: String,
    /// One line for a person.
    pub detail: String,
}

/// A slot weighed for a unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CandidateView {
    /// The slot.
    pub slot: u16,
    /// Why it was weighed.
    pub why: String,
    /// The rig checks, then the clips.
    pub checks: Vec<CheckView>,
    /// Whether no check rules it out.
    pub feasible: bool,
}

/// What another policy chooses for a unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct AlternativeView {
    /// The policy.
    pub policy: PolicyView,
    /// Its target. None: the unit is left to the game.
    pub chosen: Option<u16>,
    /// Its reason.
    pub reason: String,
}

/// The target chosen for one unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct DecisionView {
    /// The unit, by its id in the report: `%u0`.
    pub unit: String,
    /// Its character, by its id: `%c0`.
    pub character: String,
    /// The character's name, filled from the report's characters.
    #[serde(default)]
    pub character_name: String,
    /// The slot its bin sits in.
    pub source: u16,
    /// The target. None: the unit is left to the game.
    pub chosen: Option<u16>,
    /// Why, as the library names it.
    pub reason: String,
    /// The unit's origin.
    pub origin: OriginView,
    /// The slots weighed, in the order weighed.
    pub candidates: Vec<CandidateView>,
    /// What the compared policies choose.
    pub alternatives: Vec<AlternativeView>,
}

/// The decisions of a port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TargetsView {
    /// The policy the run decided under.
    pub policy: PolicyView,
    /// One decision per unit.
    pub decisions: Vec<DecisionView>,
}

/// The package a run built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct PackageView {
    /// The project's name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// How many chunks it holds.
    pub chunks: usize,
    /// How many patches it holds.
    pub patches: usize,
    /// Its thumbnail as a `data:` URL: the WebP the library encoded from the mod's image,
    /// never the archive's own bytes (#ADR-0051).
    pub thumbnail: Option<String>,
}

/// A run as the views show it. The store keeps a saved run's, which reads back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct RunView {
    /// Each stage of the run's kind, in order, and how far it got.
    pub stages: Vec<StageView>,
    /// The mod, once it was read.
    pub meta: Option<ModView>,
    /// The fatal finding that ended the run, by its id.
    pub halted: Option<u32>,
    /// Every finding, in the order raised: the inputs' first.
    pub findings: Vec<Finding>,
    /// What each fixup did, in the order they ran.
    pub changes: Vec<FixupChanges>,
    /// The objects the fixups changed.
    pub objects: Vec<ObjectView>,
    /// The decisions, for a port that reached `decide`.
    pub targets: Option<TargetsView>,
    /// The package, once it was built.
    pub package: Option<PackageView>,
    /// The `.modpkg` written.
    pub written: Option<String>,
    /// The run's number in the app's store, once a write kept it there (#ADR-0096).
    pub saved: Option<u64>,
    /// Whether a finding the settings block refused the write.
    pub refused: bool,
    /// An input that did not answer, which ended the run.
    pub fault: Option<String>,
    /// The stage a cancel ended, when one did (#ADR-0097). A kept view without the field
    /// reads as none.
    pub cancelled: Option<StageName>,
    /// How long the run took, in milliseconds. Shown, never written to a report.
    pub millis: u64,
    /// A name for each id of the report's tables and of the objects the fixups changed: what
    /// a finding's fields name by id (`%w0`, `%f12`, `%c0`, `%u0`, `%o14`).
    pub names: BTreeMap<String, String>,
}

#[derive(Deserialize)]
pub(crate) struct WadRow {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
pub(crate) struct FileRow {
    pub id: String,
    #[serde(default)]
    pub path: Option<String>,
    pub hash: String,
}

#[derive(Deserialize)]
pub(crate) struct UnitRow {
    pub id: String,
    pub character_name: String,
    pub slot: u16,
}

#[derive(Deserialize)]
pub(crate) struct PackageRow {
    pub name: String,
    pub version: String,
    pub chunks: Vec<IgnoredAny>,
    // The report leaves an empty list of patches out.
    #[serde(default)]
    pub patches: Vec<IgnoredAny>,
}

/// What a fixup did at one place, as the report holds it.
#[derive(Deserialize)]
pub(crate) struct ChangeRow {
    pub object: String,
    pub entry: String,
    pub reached: bool,
    pub path: String,
    #[serde(default)]
    pub before: Option<ValueView>,
    pub after: AfterView,
}

/// What one fixup did, as the report holds it.
#[derive(Deserialize)]
pub(crate) struct FixupRow {
    pub fixup: String,
    pub changed: usize,
    pub removed: usize,
    pub lost: usize,
    pub unconvertible: usize,
    pub changes: Vec<ChangeRow>,
}

#[derive(Deserialize)]
pub(crate) struct CharacterRow {
    pub id: String,
    pub spelled: String,
}

/// The parts of the report the views draw.
#[derive(Default, Deserialize)]
pub(crate) struct ReportParts {
    #[serde(default, rename = "mod")]
    pub meta: Option<ModView>,
    #[serde(default)]
    pub halted: Option<u32>,
    #[serde(default)]
    pub wads: Vec<WadRow>,
    #[serde(default)]
    pub files: Vec<FileRow>,
    #[serde(default)]
    pub characters: Vec<CharacterRow>,
    #[serde(default)]
    pub units: Vec<UnitRow>,
    #[serde(default)]
    pub changes: Option<Vec<FixupRow>>,
    #[serde(default)]
    pub targets: Option<TargetsView>,
    #[serde(default)]
    pub package: Option<PackageRow>,
    pub diagnostics: Vec<Finding>,
}

impl ReportParts {
    /// A name for each id of the tables: a WAD's name, a file's path else its hash, a
    /// character's name, and a unit's character and slot as a finding's subject names it.
    pub fn names(&self) -> BTreeMap<String, String> {
        let wads = self
            .wads
            .iter()
            .map(|wad| (wad.id.clone(), wad.name.clone()));
        let files = self.files.iter().map(|file| {
            let name = file.path.clone().unwrap_or_else(|| file.hash.clone());
            (file.id.clone(), name)
        });
        let characters = (self.characters.iter()).map(|c| (c.id.clone(), c.spelled.clone()));
        let units = self.units.iter().map(|unit| {
            let name = format!("{} skin{}", unit.character_name, unit.slot);
            (unit.id.clone(), name)
        });
        wads.chain(files).chain(characters).chain(units).collect()
    }

    /// The names of a run's tables as its report stands: all four tables once `load` ended.
    pub fn names_of(run: &Run<'_>) -> BTreeMap<String, String> {
        let parts: ReportParts = read_clean(&run.report().to_json());
        parts.names()
    }

    /// The decisions, each with its character's name.
    pub fn named_targets(&mut self) -> Option<TargetsView> {
        let mut targets = self.targets.take()?;
        for decision in &mut targets.decisions {
            let name = self.characters.iter().find(|c| c.id == decision.character);
            decision.character_name = name.map_or_else(String::new, |c| c.spelled.clone());
        }
        Some(targets)
    }
}
