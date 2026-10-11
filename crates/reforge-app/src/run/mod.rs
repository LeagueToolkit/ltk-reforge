//! A run of a mod, as both views drive it (#ADR-0050): a repair or a port, the archive, the
//! last stage to run, the settings the library has, and where the package is written. Each
//! request is a new `Run`: resumed from a kept run's snapshots where one of its load holds,
//! else from the raw snapshot the app's store holds of the archive, else from the archive; a
//! run is kept in the store when it writes its package (#ADR-0094, #ADR-0096).
//!
//! The stages are run one at a time, as `Run::through` runs them, so a [`Watch`] hears each
//! one start and end, or taken from a kept run, each finding as it is raised, and what each
//! stage gave as it ends.

mod drawn;
mod peek;
mod views;

use std::collections::BTreeMap;
use std::time::Instant;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::archive::{Caps, RawSnapshot, file_name_of};
use reforge::diag::{Action, DiagnosticPolicy, Filter, Render, Stage};
use reforge::fixups::FixupSet;
use reforge::model::{ContentId, Spelled};
use reforge::package::{CheckSelection, CheckSet, EvidenceDoc, Modpkg, PackageWriter};
use reforge::port::{AnimReuse, Policy, TargetPolicy};
use reforge::snapshot::{LoadInputs, Program, Snapshots, StageSettings};
use reforge::{Porter, Repairer, Run, RunError, Settings, Start};
use serde::{Deserialize, Serialize};

use self::drawn::Drawn;
pub use self::peek::{CharacterPeek, ModPeek, peek};
pub use self::views::*;
use crate::error::{AppError, chain};
use crate::inputs::Inputs;
use crate::selection::FixupChoice;
use crate::store::{AppStore, Saved, load_key};
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
    /// The run took the stage from a kept run's snapshots, and did not run it.
    Resumed,
}

/// What hears a run as it goes. Every method has a default that does nothing.
pub trait Watch {
    /// A stage started or ended.
    fn stage(&mut self, _stage: StageName, _event: StageEvent) {}

    /// Takes each finding as it is raised, its text as a run without names writes it.
    fn findings(&mut self) -> Option<Box<dyn FnMut(Finding) + Send>> {
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

/// The porter or the repairer a request's run is made with.
enum Maker<'a> {
    Repair(Repairer<'a>),
    Port(Porter<'a>),
}

impl Maker<'_> {
    fn settings(&self) -> &Settings {
        match self {
            Maker::Repair(repairer) => repairer.settings(),
            Maker::Port(porter) => porter.settings(),
        }
    }

    /// A new run, its log the inputs' with `watch` listening, reading through the store's
    /// tiers when there is one (#ADR-0093).
    fn start(&self, inputs: &Inputs, store: Option<&AppStore>, watch: &mut dyn Watch) -> Run<'_> {
        let mut log = inputs.log();
        if let Some(mut listener) = watch.findings() {
            log.listen(move |diagnostic| listener(Finding::of(diagnostic, &Render::plain())));
        }
        let run = match self {
            Maker::Repair(repairer) => repairer.run(),
            Maker::Port(porter) => porter.run(),
        }
        .log_from(log);
        match store {
            None => run,
            Some(store) => run
                .in_store(store.tier().clone())
                .in_cache(store.ingest().clone()),
        }
    }
}

/// The raw snapshot the store's ingest tier holds of the archive at `path`, read under `caps`
/// (#ADR-0095), under the name a read of the path gives it. None when the file does not read:
/// `read` then says why.
fn ingested(store: &AppStore, path: &Utf8Path, caps: &Caps) -> Option<RawSnapshot> {
    let file = fs::File::open(path).ok()?;
    let sha256 = ContentId::of_reader(file).ok()?;
    let mut raw = store.ingest().get(sha256, caps)?;
    raw.meta.file_name = Spelled::new(file_name_of(path));
    Some(raw)
}

/// Runs a request against the inputs, telling `watch` how it goes. With a store, the run
/// reads through its tiers and starts from the raw snapshot it holds of the archive, and a
/// run that wrote its package is kept in it (#ADR-0096).
pub fn run(
    inputs: &Inputs,
    request: &RunRequest,
    store: Option<&AppStore>,
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

    let started = Instant::now();
    let env = inputs.env();
    let maker = match request.kind {
        RunKind::Repair => Maker::Repair(
            Repairer::new(env)
                .fixups(fixups)
                .checks(checks)
                .keep_stale(settings.keep_stale)
                .keep_dds(settings.keep_dds),
        ),
        RunKind::Port => Maker::Port(
            Porter::new(env)
                .policy(policy)
                .compare(policy.other_levels())
                .fixups(fixups)
                .checks(checks)
                .keep_stale(settings.keep_stale)
                .game_events_only(settings.game_events_only)
                .keep_dds(settings.keep_dds),
        ),
    };
    let archive = Utf8PathBuf::from(&request.archive);
    let caps = maker.settings().caps();
    let raw = store.and_then(|store| ingested(store, &archive, caps));
    let mut run = match (store, &raw) {
        (Some(store), Some(raw)) => resumed(&maker, inputs, store, raw, request, watch),
        _ => maker.start(inputs, store, watch),
    };

    let mut timed = Vec::new();
    let mut fault = None;
    let through = request.through.min(StageName::Check);
    for &stage in run.stages() {
        let name = StageName::of(stage);
        if name > through {
            continue;
        }
        if stage != Stage::Read && run.ran(stage) {
            watch.stage(name, StageEvent::Resumed);
            send_parts(&run, name, watch);
            continue;
        }
        watch.stage(name, StageEvent::Started);
        let at = Instant::now();
        let ran = run_stage(&mut run, name, &archive);
        let millis = millis_since(at);
        watch.stage(name, StageEvent::Ended { millis });
        timed.push((name, millis));
        match ran {
            Ok(()) => send_parts(&run, name, watch),
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

    let drawn = Drawn::of(&run);
    let stages = stage_views(&run, &timed);
    let mut view = RunView {
        stages,
        saved: None,
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
    };
    if let (Some(store), Some(package)) = (store, view.written.clone()) {
        let kept = Kept {
            request,
            raw,
            package: &package,
            view: &view,
        };
        view.saved = save(store, run, inputs, kept);
    }
    Ok(view)
}

/// What a stage gave the views, sent to a watch that takes it, as the stage ends or is taken
/// from a kept run's snapshots.
fn send_parts(run: &Run<'_>, stage: StageName, watch: &mut dyn Watch) {
    if !watch.takes_parts() {
        return;
    }
    // The tables a finding names by id are the mod's, which `load` makes.
    if stage == StageName::Load {
        let names = ReportParts::names_of(run);
        watch.part(PartView::Names { names });
    }
    if let Some(part) = Drawn::part(run, stage) {
        watch.part(part);
    }
}

/// The stages a rerun may start after, latest first: those after which a run keeps a
/// snapshot.
const STARTS: [Stage; 7] = [
    Stage::Check,
    Stage::Build,
    Stage::Plan,
    Stage::Decide,
    Stage::Facts,
    Stage::Fix,
    Stage::Load,
];

/// The start after `stage` from a kept run's snapshots, when they hold its snapshot.
fn start_after(snapshots: &Snapshots, stage: Stage) -> Option<Start> {
    let all = || Box::new(snapshots.clone());
    match stage {
        Stage::Load => Some(Start::Load(Box::new(snapshots.load.clone()))),
        Stage::Fix => snapshots.fix.as_ref().map(|_| Start::Fix(all())),
        Stage::Facts => snapshots.facts.as_ref().map(|_| Start::Facts(all())),
        Stage::Decide => snapshots.decide.as_ref().map(|_| Start::Decide(all())),
        Stage::Plan => snapshots.plan.as_ref().map(|_| Start::Plan(all())),
        Stage::Build => snapshots.build.as_ref().map(|_| Start::Build(all())),
        Stage::Check => snapshots.check.as_ref().map(|_| Start::Check(all())),
        Stage::Read | Stage::Write => None,
    }
}

/// A run of an archive the ingest tier holds, resumed from the newest kept run of the
/// request's kind and load where one holds (#ADR-0094, #ADR-0096), else from the raw snapshot,
/// else from nothing, `read` then reading the archive.
fn resumed<'m>(
    maker: &'m Maker<'_>,
    inputs: &Inputs,
    store: &AppStore,
    raw: &RawSnapshot,
    request: &RunRequest,
    watch: &mut dyn Watch,
) -> Run<'m> {
    if let Some(run) = from_kept(maker, inputs, store, raw, request, watch) {
        return run;
    }
    let run = maker.start(inputs, Some(store), watch);
    run.resume(raw.clone()).unwrap_or_else(|error| {
        tracing::warn!(%error, "a run does not start from the ingest tier's raw snapshot");
        maker.start(inputs, Some(store), watch)
    })
}

/// A run resumed from the newest kept run of the request's kind whose load is the one this
/// run would make: of the same raw snapshot, program, load inputs and load settings. None
/// when the store keeps none, or every start is refused.
///
/// The start is the latest stage the snapshots hold that is at or before the request's
/// `through`, so a resume never runs past the request; each start refused, by a setting, an
/// input or the program that differs, falls back to the one before it, down to `load`.
fn from_kept<'m>(
    maker: &'m Maker<'_>,
    inputs: &Inputs,
    store: &AppStore,
    raw: &RawSnapshot,
    request: &RunRequest,
    watch: &mut dyn Watch,
) -> Option<Run<'m>> {
    let declared = maker.settings().declared(Stage::Load);
    let StageSettings::Load(load_settings) = &declared else {
        panic!("the settings load declares are {declared:?}");
    };
    let load_inputs = LoadInputs::of(inputs.env());
    let key = load_key(raw, &Program::this(), &load_inputs, load_settings);
    let kept = store.kept_runs_of(&key);
    let kept = kept
        .inspect_err(|error| tracing::warn!(%error, "the kept runs of a load do not read"))
        .ok()?;
    // A repair's snapshots do not resume a port.
    let newest = kept.into_iter().find(|kept| kept.kind == request.kind)?;
    let starts = STARTS.into_iter();
    let starts = starts.filter(|&stage| StageName::of(stage) <= request.through);
    for stage in starts {
        let Some(start) = start_after(&newest.snapshots, stage) else {
            continue;
        };
        match maker.start(inputs, Some(store), watch).resume(start) {
            Ok(run) => return Some(run),
            Err(error) => {
                tracing::debug!(run = newest.id, %stage, %error, "a kept run does not resume");
            }
        }
    }
    None
}

/// What a run that wrote its package keeps beside the run itself.
struct Kept<'a> {
    request: &'a RunRequest,
    /// The raw snapshot the ingest tier held of the archive, which a run resumed from a kept
    /// run's later snapshots does not hold.
    raw: Option<RawSnapshot>,
    package: &'a str,
    view: &'a RunView,
}

/// Keeps a run that wrote its package in the store with the view it ended with, and answers
/// its number. A run with no snapshots, one judged with an earlier run's evidence, is not
/// kept. A store that does not write leaves the run as it ran, unkept, and says so in the log.
fn save(store: &AppStore, run: Run<'_>, inputs: &Inputs, kept: Kept<'_>) -> Option<u64> {
    let Kept {
        request,
        raw,
        package,
        view,
    } = kept;
    let snapshots = run.snapshots()?;
    let raw = run.archive().cloned().or(raw)?;
    let saved = Saved {
        raw,
        path: Utf8PathBuf::from(&request.archive),
        snapshots,
        report: run.report(),
        view: view.clone(),
        kind: request.kind,
        settings: request.settings.clone(),
        schema_build: inputs
            .env()
            .schema()
            .map(|schema| schema.latest().to_string()),
        package: Some(package.to_string()),
        layer: run.into_layer(),
    };
    let kept = store.save(saved);
    kept.inspect_err(|error| tracing::warn!(%error, "a run that wrote its package is not kept"))
        .ok()
}

/// Runs one stage. `read` reads the archive from its path, unless the run started from its
/// raw snapshot.
fn run_stage(run: &mut Run<'_>, stage: StageName, archive: &Utf8Path) -> Result<(), RunError> {
    match stage {
        StageName::Read if run.ran(Stage::Read) => Ok(()),
        StageName::Read => run.read_file(archive).map(drop),
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

/// The stages of a run of the kind, in order, as the library's `Run::stages` names them: what
/// a view with no run behind it lists. A repair has no `facts`, `decide` or `plan`
/// (#ADR-0011).
pub fn stages_of_kind(kind: RunKind) -> &'static [StageName] {
    use StageName::*;
    match kind {
        RunKind::Repair => &[Read, Load, Fix, Build, Check, Write],
        RunKind::Port => &[Read, Load, Fix, Facts, Decide, Plan, Build, Check, Write],
    }
}

/// A view with no run behind it: every stage of the kind not run, and what was drawn.
fn view_not_run(kind: RunKind, drawn: Drawn) -> RunView {
    let stages = stages_of_kind(kind).iter().map(|&stage| StageView {
        stage,
        state: StageState::NotRun,
        millis: None,
        resumed: false,
    });
    RunView {
        stages: stages.collect(),
        meta: drawn.meta,
        halted: drawn.halted,
        findings: drawn.findings,
        changes: drawn.changes,
        objects: drawn.objects,
        targets: drawn.targets,
        package: drawn.package,
        written: None,
        saved: None,
        refused: false,
        fault: None,
        millis: 0,
        names: drawn.names,
    }
}

/// What a package's report gives the views, for a package opened as a run (#ADR-0096). None
/// for a report the views do not read.
pub fn view_of_report(kind: RunKind, report: &str) -> Option<RunView> {
    Drawn::of_report(report).map(|drawn| view_not_run(kind, drawn))
}

/// What a package's evidence alone gives the views (#ADR-0041, #ADR-0096): the mod, and for
/// a port each unit that ships with the slot it ships at and why.
pub fn view_of_evidence(kind: RunKind, evidence: &EvidenceDoc, meta: ModView) -> RunView {
    let targets = evidence.port.as_ref().map(|port| {
        let decisions = port.units.iter().map(|unit| DecisionView {
            // The evidence names a unit by its character and slot, not by a report's id.
            unit: format!("{} skin{}", unit.character, unit.source),
            character: unit.character.clone(),
            character_name: unit.character.clone(),
            source: unit.source,
            chosen: Some(unit.target),
            reason: unit.reason.clone(),
            origin: OriginView {
                verdict: "unknown".to_string(),
                slot: None,
            },
            candidates: Vec::new(),
            alternatives: Vec::new(),
        });
        TargetsView {
            policy: PolicyView {
                target: port.policy.target.clone(),
                anim_reuse: port.policy.anim_reuse.clone(),
            },
            decisions: decisions.collect(),
        }
    });
    let drawn = Drawn {
        meta: Some(meta),
        halted: None,
        findings: Vec::new(),
        changes: Vec::new(),
        objects: Vec::new(),
        targets,
        package: None,
        names: BTreeMap::new(),
    };
    view_not_run(kind, drawn)
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
                resumed: state == StageState::Ran && millis.is_none(),
            }
        })
        .collect()
}
