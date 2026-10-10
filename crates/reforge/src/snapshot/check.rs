//! The snapshot after `check` (#ADR-0094): what applying each patch came to, as the report
//! holds it, with the patched bin's bytes by content id, and the findings of the checks.
//! `check` changes no byte of the package, so the package is the build snapshot's. A run
//! starts from it and the snapshots before it, and `write` runs next.

use serde::{Deserialize, Serialize};

use super::{Program, ResumeError, SNAPSHOT_FORMAT, Setting, from_json, to_json};
use crate::diag::Logged;
use crate::model::ContentId;
use crate::package::{AppliedPatch, CheckId, CheckSet, Outcome};
use crate::report::AppliedRow;
use crate::store::BlobStore;

/// The settings `check` reads (#ADR-0094). It reads no input `load` and `fix` do not: the
/// install's bins the patches apply to and every bin a patched bin loads, the shader bin, and
/// the target build's types, from the schema and the install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckSettings {
    /// The checks, by id, in the order they run.
    pub checks: Vec<CheckId>,
}

impl CheckSettings {
    /// The settings of a run under a set of checks.
    pub fn of(set: &CheckSet) -> Self {
        let infos = set.infos().into_iter();
        Self {
            checks: infos.map(|info| info.id.clone()).collect(),
        }
    }

    fn differs(&self, other: &Self) -> Option<Setting> {
        (self.checks != other.checks).then_some(Setting::Checks)
    }
}

/// What applying a patch came to, and the bin after it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppliedRecord {
    row: AppliedRow,
    /// The patched bin's bytes, when the patch applied.
    bin: Option<ContentId>,
}

/// The snapshot after `check` (#ADR-0094).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSnapshot {
    format: u32,
    program: Program,
    settings: CheckSettings,
    applied: Vec<Option<AppliedRecord>>,
    findings: Vec<Logged>,
}

impl CheckSnapshot {
    /// The snapshot of the checks under `settings`, with what applying each patch of the
    /// package came to in the package's order (`rows`, made from `patched`), the bins after
    /// the patches put into `store`, and the findings raised from the start of `read` to the
    /// end of `check`.
    pub fn of(
        rows: &[Option<AppliedRow>],
        patched: &[Option<&AppliedPatch>],
        (settings, findings): (CheckSettings, Vec<Logged>),
        store: &dyn BlobStore,
    ) -> Self {
        let applied = rows.iter().zip(patched).map(|(row, patched)| {
            let row = row.clone()?;
            let bin = match &patched.as_ref()?.outcome {
                Outcome::Applied(bin) => Some(store.put(bin.bytes.clone().into())),
                Outcome::NoGameBin | Outcome::Failed { .. } => None,
            };
            Some(AppliedRecord { row, bin })
        });
        Self {
            format: SNAPSHOT_FORMAT,
            program: Program::this(),
            settings,
            applied: applied.collect(),
            findings,
        }
    }

    /// The program that wrote it.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The settings `check` read.
    pub fn settings(&self) -> &CheckSettings {
        &self.settings
    }

    /// What applying each patch came to, in the package's order, as the report holds it.
    pub fn applied(&self) -> Vec<Option<AppliedRow>> {
        let applied = self.applied.iter();
        applied
            .map(|record| record.as_ref().map(|record| record.row.clone()))
            .collect()
    }

    /// The id of each patched bin's bytes, in the package's order: none for a patch that did
    /// not apply.
    pub fn bins(&self) -> Vec<Option<ContentId>> {
        let applied = self.applied.iter();
        applied
            .map(|record| record.as_ref().and_then(|record| record.bin))
            .collect()
    }

    /// The findings from the start of `read` to the end of `check`, in order.
    pub fn findings(&self) -> &[Logged] {
        &self.findings
    }

    /// The snapshot as JSON, every character a terminal acts on written as a JSON escape.
    pub fn to_json(&self) -> String {
        to_json(self, "check")
    }

    /// A snapshot from its JSON.
    pub fn from_json(json: &str) -> Result<Self, ResumeError> {
        from_json(json)
    }

    /// Refuses a run whose program or `check` settings are not the snapshot's.
    pub fn holds(&self, settings: &CheckSettings) -> Result<(), ResumeError> {
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
