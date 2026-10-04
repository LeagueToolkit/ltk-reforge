//! A run of a mod, as both views drive it (#ADR-0050): the archive, the last stage to run, the
//! settings the library has, and where the package is written. Each request is a new `Run`
//! from the archive. No stage's result is kept between requests.

use std::time::Instant;

use camino::Utf8PathBuf;
use reforge::diag::{Action, DiagnosticPolicy, Filter, Stage};
use reforge::fixups::{FixupSelection, FixupSet};
use reforge::package::{CheckSelection, CheckSet, Modpkg, PackageWriter};
use reforge::{Repairer, Run, RunError};
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, chain};
use crate::inputs::Inputs;
use crate::view::{Finding, read_clean, through_clean};

/// A stage of a run, in the order they run.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "kebab-case")]
pub enum StageName {
    /// The archive read under its caps.
    Read,
    /// The WADs, files, bins and units found.
    Load,
    /// The values brought to the target build.
    Fix,
    /// What is true of the mod. A port's alone.
    Facts,
    /// The target chosen. A port's alone.
    Decide,
    /// The units placed. A port's alone.
    Plan,
    /// The package built.
    Build,
    /// The package checked as the game would apply it.
    Check,
    /// The package written.
    Write,
}

impl StageName {
    /// The stages of a repair, in order (#ADR-0011).
    pub const REPAIR: [StageName; 6] = [
        StageName::Read,
        StageName::Load,
        StageName::Fix,
        StageName::Build,
        StageName::Check,
        StageName::Write,
    ];

    fn stage(self) -> Stage {
        match self {
            StageName::Read => Stage::Read,
            StageName::Load => Stage::Load,
            StageName::Fix => Stage::Fix,
            StageName::Facts => Stage::Facts,
            StageName::Decide => Stage::Decide,
            StageName::Plan => Stage::Plan,
            StageName::Build => Stage::Build,
            StageName::Check => Stage::Check,
            StageName::Write => Stage::Write,
        }
    }

    fn of(stage: Stage) -> Self {
        match stage {
            Stage::Read => StageName::Read,
            Stage::Load => StageName::Load,
            Stage::Fix => StageName::Fix,
            Stage::Facts => StageName::Facts,
            Stage::Decide => StageName::Decide,
            Stage::Plan => StageName::Plan,
            Stage::Build => StageName::Build,
            Stage::Check => StageName::Check,
            Stage::Write => StageName::Write,
        }
    }
}

/// The settings a run takes, each one the library's (`docs/design/ROADMAP-UX.md`, "Two views
/// of one run"). The default is the standard set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct RunSettings {
    /// Fixups turned off.
    pub fixups_off: Vec<String>,
    /// Fixups turned on that the standard set leaves out.
    pub fixups_on: Vec<String>,
    /// Checks turned off.
    pub checks_off: Vec<String>,
    /// Carry stale copies as the mod's own files.
    pub keep_stale: bool,
    /// Filters of the findings that refuse the write: codes or areas and `key=value` terms.
    /// None blocks by default (#ADR-0003).
    pub block: Vec<String>,
}

impl RunSettings {
    fn fixups(&self) -> Result<FixupSet, AppError> {
        let mut selection = FixupSelection::standard();
        for id in &self.fixups_off {
            selection = selection.without(id.as_str());
        }
        for id in &self.fixups_on {
            selection = selection.with(id.as_str());
        }
        if let Some(unknown) = selection.unknown().first() {
            return Err(AppError::UnknownFixup {
                id: unknown.to_string(),
            });
        }
        Ok(FixupSet::from(&selection))
    }

    fn checks(&self) -> Result<CheckSet, AppError> {
        let mut selection = CheckSelection::standard();
        for id in &self.checks_off {
            selection = selection.without(id.as_str());
        }
        if let Some(unknown) = selection.unknown().first() {
            return Err(AppError::UnknownCheck {
                id: unknown.to_string(),
            });
        }
        Ok(CheckSet::from(&selection))
    }

    fn policy(&self) -> Result<DiagnosticPolicy, AppError> {
        let mut policy = DiagnosticPolicy::default();
        for text in &self.block {
            let filter = Filter::parse(text).map_err(|error| AppError::BadFilter {
                filter: text.clone(),
                detail: error.to_string(),
            })?;
            policy = policy.with(filter, Action::Block);
        }
        Ok(policy)
    }
}

/// What the front end asks a run to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct RunRequest {
    /// The mod archive: a `.fantome`, or a zip of the same layout.
    pub archive: String,
    /// The last stage to run. `write` runs every stage and writes the package.
    pub through: StageName,
    /// The settings.
    pub settings: RunSettings,
    /// The folder the `.modpkg` and its report are written in. Required to write.
    pub out: Option<String>,
}

/// How far a stage got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct StageView {
    /// The stage.
    pub stage: StageName,
    /// How far it got.
    pub state: StageState,
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

/// What a fixup did at one place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ChangeView {
    /// The object's entry hash, as eight hex digits.
    pub entry: String,
    /// Whether a unit's skin entry reaches the object.
    pub reached: bool,
    /// Where the value was, its fields spelled.
    pub path: String,
    /// The value before. None for a value the fixup added.
    #[serde(default)]
    pub before: Option<ValueView>,
    /// What is there after.
    pub after: AfterView,
}

/// What one fixup did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct FixupChanges {
    /// The fixup's id.
    pub fixup: String,
    /// Values kept, in the form the target build reads.
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

/// The package a run built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct PackageView {
    /// The project's name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// How many chunks it holds.
    pub chunks: usize,
    /// How many patches it holds.
    pub patches: usize,
}

/// A run as the views show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
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
    /// The package, once it was built.
    pub package: Option<PackageView>,
    /// The `.modpkg` written.
    pub written: Option<String>,
    /// Whether a finding the settings block refused the write.
    pub refused: bool,
    /// An input that did not answer, which ended the run.
    pub fault: Option<String>,
    /// How long the run took, in milliseconds. Shown, never written to a report.
    pub millis: u64,
}

#[derive(Deserialize)]
struct PackageRow {
    name: String,
    version: String,
    chunks: Vec<IgnoredAny>,
    // The report leaves an empty list of patches out.
    #[serde(default)]
    patches: Vec<IgnoredAny>,
}

/// The parts of the report the views draw.
#[derive(Deserialize)]
struct ReportParts {
    #[serde(default, rename = "mod")]
    meta: Option<ModView>,
    #[serde(default)]
    halted: Option<u32>,
    #[serde(default)]
    changes: Option<Vec<FixupChanges>>,
    #[serde(default)]
    package: Option<PackageRow>,
    diagnostics: Vec<Finding>,
}

/// Runs a repair of the archive a request names, against the inputs.
pub fn repair(inputs: &Inputs, request: &RunRequest) -> Result<RunView, AppError> {
    let fixups = request.settings.fixups()?;
    let checks = request.settings.checks()?;
    let policy = request.settings.policy()?;
    let out = match (request.through, &request.out) {
        (StageName::Write, None) => return Err(AppError::NoOutput),
        (StageName::Write, Some(out)) => Some(Utf8PathBuf::from(out)),
        _ => None,
    };

    let started = Instant::now();
    let repairer = Repairer::new(inputs.env())
        .fixups(fixups)
        .checks(checks)
        .keep_stale(request.settings.keep_stale);
    let mut run = repairer.run().log_from(inputs.log());

    let last = request.through.min(StageName::Check).stage();
    let mut fault = match run.read_file(Utf8PathBuf::from(&request.archive).as_path()) {
        Ok(_) | Err(RunError::Halted(_)) => None,
        Err(error) => Some(chain(&error)),
    };
    let read = fault.is_none();
    if fault.is_none() && run.halted().is_none() {
        fault = run.through(last).err().map(|error| chain(&error));
    }

    let mut written = None;
    let mut refused = false;
    if let (Some(out), Some(package), None) = (&out, run.package(), &fault) {
        let modpkg = Modpkg::new(out);
        let path = modpkg.path_of(package);
        let writers: [&dyn PackageWriter; 1] = [&modpkg];
        match run.write(&writers, &policy) {
            Ok(()) => written = Some(path.to_string()),
            Err(RunError::Refused(_)) => refused = true,
            Err(error) => fault = Some(chain(&error)),
        }
    }
    let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    let parts: ReportParts = read_clean(&run.report().to_json());
    // The report names the mod once `load` made it; a run stopped after `read` has the
    // archive's word for it.
    let meta = parts
        .meta
        .or_else(|| run.archive().map(|archive| through_clean(&archive.meta)));
    let stages = stages(&run, request.through, read, written.is_some());
    Ok(RunView {
        stages,
        meta,
        halted: parts.halted,
        findings: parts.diagnostics,
        changes: parts.changes.unwrap_or_default(),
        package: parts.package.map(|row| PackageView {
            name: row.name,
            version: row.version,
            chunks: row.chunks.len(),
            patches: row.patches.len(),
        }),
        written,
        refused,
        fault,
        millis,
    })
}

/// How far each stage of a repair got. A stage after the one a fatal finding ended did not
/// run, and `write` ran only when a package was written. The run gives the archive up to
/// `load`, so whether `read` ran is the caller's to say.
fn stages(run: &Run<'_>, through: StageName, read: bool, written: bool) -> Vec<StageView> {
    let halted_at = run
        .halted()
        .and_then(|halted| run.log().get(halted.0))
        .map(|finding| StageName::of(finding.stage));
    let reached = [
        read,
        run.module().is_some(),
        run.fixed().is_some(),
        run.package().is_some(),
        run.package().is_some() && through >= StageName::Check,
        written,
    ];
    StageName::REPAIR
        .into_iter()
        .zip(reached)
        .map(|(stage, reached)| {
            let state = match halted_at {
                Some(at) if at == stage => StageState::Halted,
                _ if reached && stage <= through => StageState::Ran,
                _ => StageState::NotRun,
            };
            StageView { stage, state }
        })
        .collect()
}
