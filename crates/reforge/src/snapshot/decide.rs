//! The snapshot after `decide` (#ADR-0094): the targets as `decide` chose them. `decide`
//! changes no byte, so the mod's files are the fix snapshot's.

use serde::{Deserialize, Serialize};

use super::{Program, ResumeError, SNAPSHOT_FORMAT, Setting, StageSnapshot};
use crate::diag::{Logged, Stage};
use crate::model::ContentId;
use crate::port::{Policy, Targets, UnitTarget};

/// The settings `decide` reads (#ADR-0094). It reads no input: only the facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecideSettings {
    /// The policy each unit's target is chosen under.
    pub policy: Policy,
    /// The policies whose choices are recorded beside it.
    pub compared: Vec<Policy>,
    /// The targets given to units alone, as given (#ADR-0098).
    pub unit_targets: Vec<UnitTarget>,
}

impl DecideSettings {
    fn differs(&self, other: &Self) -> Option<Setting> {
        if self.policy != other.policy {
            Some(Setting::Policy)
        } else if self.compared != other.compared {
            Some(Setting::Compared)
        } else if self.unit_targets != other.unit_targets {
            Some(Setting::UnitTargets)
        } else {
            None
        }
    }
}

/// The snapshot after `decide` (#ADR-0094).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecideSnapshot {
    format: u32,
    program: Program,
    settings: DecideSettings,
    targets: Targets,
    findings: Vec<Logged>,
}

impl StageSnapshot for DecideSnapshot {
    const STAGE: Stage = Stage::Decide;
}

impl DecideSnapshot {
    /// The snapshot of the targets `decide` chose under `settings`, with the findings raised
    /// from the start of `read` to the end of `decide`.
    pub fn of(targets: Targets, settings: DecideSettings, findings: Vec<Logged>) -> Self {
        Self {
            format: SNAPSHOT_FORMAT,
            program: Program::this(),
            settings,
            targets,
            findings,
        }
    }

    /// The program that wrote it.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The settings `decide` read.
    pub fn settings(&self) -> &DecideSettings {
        &self.settings
    }

    /// The targets, with what the compared policies choose.
    pub fn targets(&self) -> &Targets {
        &self.targets
    }

    /// Every id it names: none, since `decide` changes no byte.
    pub fn ids(&self) -> impl Iterator<Item = ContentId> + '_ {
        std::iter::empty()
    }

    /// The findings from the start of `read` to the end of `decide`, in order.
    pub fn findings(&self) -> &[Logged] {
        &self.findings
    }

    /// Refuses a run whose program or `decide` settings are not the snapshot's.
    pub fn holds(&self, settings: &DecideSettings) -> Result<(), ResumeError> {
        if self.format != SNAPSHOT_FORMAT {
            return Err(ResumeError::Format { found: self.format });
        }
        self.program.resumes()?;
        if let Some(setting) = self.settings.differs(settings) {
            return Err(ResumeError::Setting(setting));
        }
        Ok(())
    }
}
