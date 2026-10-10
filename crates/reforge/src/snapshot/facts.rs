//! The snapshot after `facts` (#ADR-0094): what the facts found, as `Facts` holds it. `facts`
//! changes no byte, so the mod's files are the fix snapshot's. A run starts from it and the
//! snapshots before it, and asks the game for the catalogs again.

use serde::{Deserialize, Serialize};

use super::{Program, ResumeError, SNAPSHOT_FORMAT, Setting, StageSnapshot};
use crate::diag::{Logged, Stage};
use crate::facts::{Facts, FactsFound};
use crate::load::ReworkTable;

/// The settings `facts` reads (#ADR-0094). It reads no input `load` does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactsSettings {
    /// The champions whose model was replaced, and when: a unit made for a model before its
    /// rework is judged against it.
    pub reworks: ReworkTable,
}

impl FactsSettings {
    fn differs(&self, other: &Self) -> Option<Setting> {
        (self.reworks != other.reworks).then_some(Setting::Reworks)
    }
}

/// The snapshot after `facts` (#ADR-0094).
#[derive(Clone, Serialize, Deserialize)]
pub struct FactsSnapshot {
    format: u32,
    program: Program,
    settings: FactsSettings,
    facts: FactsFound,
    findings: Vec<Logged>,
}

impl StageSnapshot for FactsSnapshot {
    const STAGE: Stage = Stage::Facts;
}

impl FactsSnapshot {
    /// The snapshot of what `facts` found under `settings`, with the findings raised from the
    /// start of `read` to the end of `facts`.
    pub fn of(facts: &Facts, settings: FactsSettings, findings: Vec<Logged>) -> Self {
        Self {
            format: SNAPSHOT_FORMAT,
            program: Program::this(),
            settings,
            facts: facts.found(),
            findings,
        }
    }

    /// The program that wrote it.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The settings `facts` read.
    pub fn settings(&self) -> &FactsSettings {
        &self.settings
    }

    /// What the facts found.
    pub fn found(&self) -> &FactsFound {
        &self.facts
    }

    /// The findings from the start of `read` to the end of `facts`, in order.
    pub fn findings(&self) -> &[Logged] {
        &self.findings
    }

    /// Refuses a run whose program or `facts` settings are not the snapshot's.
    pub fn holds(&self, settings: &FactsSettings) -> Result<(), ResumeError> {
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

impl std::fmt::Debug for FactsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut shown = f.debug_struct("FactsSnapshot");
        shown.field("program", &self.program);
        shown.field("findings", &self.findings.len());
        shown.finish_non_exhaustive()
    }
}
