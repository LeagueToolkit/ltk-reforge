//! The snapshot after `check` (#ADR-0094): what applying each patch came to, as the report
//! holds it, with the patched bin's bytes by content id, and the findings of the checks.
//! `check` changes no byte of the package, so the package is the build snapshot's. A run
//! starts from it and the snapshots before it, and `write` runs next.

use serde::{Deserialize, Serialize};

use super::{Program, ResumeError, SNAPSHOT_FORMAT, Setting, StageSnapshot};
use crate::diag::{Logged, Stage};
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

/// The snapshot after `check` (#ADR-0094).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSnapshot {
    format: u32,
    program: Program,
    settings: CheckSettings,
    applied: Vec<Option<AppliedRow>>,
    bins: Vec<Option<ContentId>>,
    findings: Vec<Logged>,
}

impl StageSnapshot for CheckSnapshot {
    const STAGE: Stage = Stage::Check;
}

impl CheckSnapshot {
    /// The snapshot of the checks under `settings`, with what applying each patch of the
    /// package came to in the package's order (`applied`, made from `patched`), the bins
    /// after the patches put into `store`, and the findings raised from the start of `read`
    /// to the end of `check`.
    pub fn of(
        applied: Vec<Option<AppliedRow>>,
        patched: &[Option<&AppliedPatch>],
        (settings, findings): (CheckSettings, Vec<Logged>),
        store: &dyn BlobStore,
    ) -> Self {
        let bins = patched
            .iter()
            .map(|patched| match &patched.as_ref()?.outcome {
                Outcome::Applied(bin) => Some(store.put(bin.bytes.clone().into())),
                Outcome::NoGameBin | Outcome::Failed { .. } => None,
            });
        Self {
            format: SNAPSHOT_FORMAT,
            program: Program::this(),
            settings,
            applied,
            bins: bins.collect(),
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

    /// What applying each patch came to, in the package's order, as the report holds it:
    /// none for a patch the run did not apply.
    pub fn applied(&self) -> &[Option<AppliedRow>] {
        &self.applied
    }

    /// The id of each patched bin's bytes, in the package's order: none for a patch that did
    /// not apply.
    pub fn bins(&self) -> &[Option<ContentId>] {
        &self.bins
    }

    /// The findings from the start of `read` to the end of `check`, in order.
    pub fn findings(&self) -> &[Logged] {
        &self.findings
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
