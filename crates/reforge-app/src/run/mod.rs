//! A run of a mod, as both views drive it (#ADR-0050): a repair or a port, the archive, the
//! last stage to run, the settings the library has, and where the package is written. Each
//! request is a new `Run` from the archive, and no stage's result is kept between requests.
//!
//! The stages are run one at a time, as `Run::through` runs them, so a [`Watch`] hears each
//! one start and end, each finding as it is raised, and what each stage gave as it ends.

mod drawn;
mod views;

use std::time::Instant;

use camino::Utf8PathBuf;
use parking_lot::Mutex;
use reforge::diag::{Action, DiagnosticPolicy, Filter, Render, Stage};
use reforge::fixups::FixupSet;
use reforge::package::{CheckSelection, CheckSet, Modpkg, PackageWriter};
use reforge::port::{AnimReuse, Policy, TargetPolicy};
use reforge::{Porter, Repairer, Run, RunError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use self::drawn::Drawn;
pub use self::views::*;
use crate::error::{AppError, chain};
use crate::inputs::Inputs;
use crate::runlog::{Capture, LogLine};
use crate::selection::FixupChoice;
use crate::view::Finding;

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

/// What a run makes of a mod. A port unless the request says otherwise (#ADR-0052).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum RunKind {
    /// The mod as it is, at its own slots and paths, its bins fixed (#ADR-0011).
    Repair,
    /// The mod over the game's own skin at each unit's target.
    #[default]
    Port,
}

/// The settings a run takes, each one the library's (`docs/design/ROADMAP-UX.md`, "Two views
/// of one run"). The default is the standard set and the default policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct RunSettings {
    /// The fixups turned off and on, and the rules added.
    pub fixups: FixupChoice,
    /// Checks turned off.
    pub checks_off: Vec<String>,
    /// Carry stale copies as the mod's own files.
    pub keep_stale: bool,
    /// Filters of the findings that refuse the write: codes or areas and `key=value` terms.
    /// None blocks by default (#ADR-0003).
    pub block: Vec<String>,
    /// How a port chooses each unit's target: `auto`, `base`, `origin`, `slot` or a skin
    /// number.
    pub target: String,
    /// The animation reuse a port accepts: `block`, `same-joints`, `own-rig` or `allow`.
    pub anim_reuse: String,
    /// Cut a port's events from the game's bank only.
    pub game_events_only: bool,
    /// Ship each DDS as it is, put in TEX's container nowhere.
    pub keep_dds: bool,
}

impl Default for RunSettings {
    fn default() -> Self {
        let policy = Policy::default();
        Self {
            fixups: FixupChoice::default(),
            checks_off: Vec::new(),
            keep_stale: false,
            block: Vec::new(),
            target: policy.target.to_string(),
            anim_reuse: policy.anim_reuse.as_str().to_string(),
            game_events_only: false,
            keep_dds: false,
        }
    }
}

impl RunSettings {
    fn fixups(&self) -> Result<FixupSet, AppError> {
        let selection = self.fixups.selection()?;
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

    fn blocking(&self) -> Result<DiagnosticPolicy, AppError> {
        let mut policy = DiagnosticPolicy::default();
        for text in self.block.iter().filter(|text| !text.trim().is_empty()) {
            let filter = Filter::parse(text).map_err(|error| AppError::BadFilter {
                filter: text.clone(),
                detail: error.to_string(),
            })?;
            policy = policy.with(filter, Action::Block);
        }
        Ok(policy)
    }

    fn policy(&self) -> Result<Policy, AppError> {
        let bad = |detail: String| AppError::BadPolicy { detail };
        Ok(Policy {
            target: TargetPolicy::parse(&self.target).map_err(|e| bad(e.to_string()))?,
            anim_reuse: AnimReuse::parse(&self.anim_reuse).map_err(|e| bad(e.to_string()))?,
        })
    }
}

/// What the front end asks a run to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct RunRequest {
    /// A repair or a port.
    #[serde(default)]
    pub kind: RunKind,
    /// The mod archive: a `.fantome`, or a zip of the same layout.
    pub archive: String,
    /// The last stage to run. `write` runs every stage and writes the package.
    pub through: StageName,
    /// The settings.
    pub settings: RunSettings,
    /// The folder the `.modpkg` and its report are written in. Required to write.
    pub out: Option<String>,
}

/// A stage starting or ending, as a run tells its [`Watch`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum StageEvent {
    /// The stage started.
    Started,
    /// The stage ended, after this many milliseconds.
    Ended { millis: u64 },
}

/// What hears a run as it goes. Every method has a default that does nothing.
pub trait Watch {
    /// A stage started or ended.
    fn stage(&mut self, _stage: StageName, _event: StageEvent) {}

    /// Takes each finding as it is raised, its text as a run without names writes it.
    fn findings(&mut self) -> Option<Box<dyn FnMut(Finding) + Send>> {
        None
    }

    /// Takes each line of the run's details as it comes.
    fn log(&mut self) -> Option<Box<dyn Fn(LogLine) + Send + Sync>> {
        None
    }

    /// Whether the watch takes what each stage gave. A part is read from the run's report,
    /// which is built for it.
    fn takes_parts(&self) -> bool {
        false
    }

    /// What a stage gave the views, as it ends.
    fn part(&mut self, _part: PartView) {}
}

/// A watch that hears nothing.
pub struct Unwatched;

impl Watch for Unwatched {}

/// The stages of a run of the kind, in order, as the library names them.
pub fn stages_of(inputs: &Inputs, kind: RunKind) -> Vec<StageName> {
    let env = inputs.env();
    let stages = match kind {
        RunKind::Repair => Repairer::new(env).run().stages(),
        RunKind::Port => Porter::new(env).run().stages(),
    };
    stages.iter().map(|&stage| StageName::of(stage)).collect()
}

fn millis_since(at: Instant) -> u64 {
    u64::try_from(at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Runs a request against the inputs, telling `watch` how it goes.
pub fn run(
    inputs: &Inputs,
    request: &RunRequest,
    watch: &mut dyn Watch,
) -> Result<RunView, AppError> {
    let settings = &request.settings;
    let fixups = settings.fixups()?;
    let checks = settings.checks()?;
    let blocking = settings.blocking()?;
    let policy = settings.policy()?;
    let out = match (request.through, &request.out) {
        (StageName::Write, None) => return Err(AppError::NoOutput),
        (StageName::Write, Some(out)) => Some(Utf8PathBuf::from(out)),
        _ => None,
    };

    // The details are kept for the view and sent on to the watch.
    let lines: Arc<Mutex<Vec<LogLine>>> = Arc::default();
    let forward = watch.log();
    let kept = Arc::clone(&lines);
    let capture = Capture::start(move |line| {
        if let Some(forward) = &forward {
            forward(line.clone());
        }
        kept.lock().push(line);
    });

    let mut log = inputs.log();
    if let Some(mut listener) = watch.findings() {
        log.listen(move |diagnostic| listener(Finding::of(diagnostic, &Render::plain())));
    }

    let started = Instant::now();
    let env = inputs.env();
    let (repairer, porter);
    let mut run = match request.kind {
        RunKind::Repair => {
            repairer = Repairer::new(env)
                .fixups(fixups)
                .checks(checks)
                .keep_stale(settings.keep_stale)
                .keep_dds(settings.keep_dds);
            repairer.run()
        }
        RunKind::Port => {
            porter = Porter::new(env)
                .policy(policy)
                .compare(policy.other_levels())
                .fixups(fixups)
                .checks(checks)
                .keep_stale(settings.keep_stale)
                .game_events_only(settings.game_events_only)
                .keep_dds(settings.keep_dds);
            porter.run()
        }
    }
    .log_from(log);

    let mut timed = Vec::new();
    let mut fault = None;
    let stages = run.stages().iter().map(|&stage| StageName::of(stage));
    for stage in stages.filter(|&stage| stage <= request.through.min(StageName::Check)) {
        watch.stage(stage, StageEvent::Started);
        let at = Instant::now();
        let ran = run_stage(&mut run, stage, &request.archive);
        let millis = millis_since(at);
        watch.stage(stage, StageEvent::Ended { millis });
        timed.push((stage, millis));
        match ran {
            Ok(()) => {
                if watch.takes_parts()
                    && let Some(part) = Drawn::part(&run, stage)
                {
                    watch.part(part);
                }
            }
            Err(RunError::Halted(_)) => break,
            Err(error) => {
                fault = Some(chain(&error));
                break;
            }
        }
    }

    let mut written = None;
    let mut refused = false;
    if let (Some(out), Some(package), None) = (&out, run.package(), &fault) {
        watch.stage(StageName::Write, StageEvent::Started);
        let at = Instant::now();
        let modpkg = Modpkg::new(out);
        let path = modpkg.path_of(package);
        let writers: [&dyn PackageWriter; 1] = [&modpkg];
        match run.write(&writers, &blocking) {
            Ok(()) => written = Some(path.to_string()),
            Err(RunError::Refused(_)) => refused = true,
            Err(error) => fault = Some(chain(&error)),
        }
        let millis = millis_since(at);
        watch.stage(StageName::Write, StageEvent::Ended { millis });
        timed.push((StageName::Write, millis));
    }
    let millis = millis_since(started);
    drop(capture);

    let drawn = Drawn::of(&run);
    let log = std::mem::take(&mut *lines.lock());
    Ok(RunView {
        stages: stage_views(&run, &timed),
        meta: drawn.meta,
        halted: drawn.halted,
        findings: drawn.findings,
        changes: drawn.changes,
        objects: drawn.objects,
        targets: drawn.targets,
        package: drawn.package,
        names: drawn.names,
        written,
        refused,
        fault,
        millis,
        log,
    })
}

/// Runs one stage. `read` reads the archive from its path.
fn run_stage(run: &mut Run<'_>, stage: StageName, archive: &str) -> Result<(), RunError> {
    match stage {
        StageName::Read => run
            .read_file(Utf8PathBuf::from(archive).as_path())
            .map(drop),
        StageName::Load => run.load().map(drop),
        StageName::Fix => run.fix().map(drop),
        StageName::Facts => run.gather().map(drop),
        StageName::Decide => run.decide().map(drop),
        StageName::Plan => run.plan().map(drop),
        StageName::Build => run.build().map(drop),
        StageName::Check => run.check(),
        StageName::Write => Ok(()),
    }
}

/// Each stage of a run, in order, with how far it got and how long it took.
fn stage_views(run: &Run<'_>, timed: &[(StageName, u64)]) -> Vec<StageView> {
    let halted_at = run.halted_at();
    let stages = run.stages().iter();
    stages
        .map(|&stage| {
            let state = match halted_at {
                Some(at) if at == stage => StageState::Halted,
                _ if run.ran(stage) => StageState::Ran,
                _ => StageState::NotRun,
            };
            let stage = StageName::of(stage);
            let millis = timed
                .iter()
                .find(|(timed, _)| *timed == stage)
                .map(|(_, millis)| *millis)
                .filter(|_| state != StageState::NotRun);
            StageView {
                stage,
                state,
                millis,
            }
        })
        .collect()
}
