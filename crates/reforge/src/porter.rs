//! The stages driven in order. A [`Porter`] or a [`Repairer`] holds what a caller chose, and a
//! [`Run`] takes one archive through the stages with it: one method per stage, each handing
//! its stage the stage's sink and options.
//!
//! The run owns what passes between the stages: the log, the names learnt, each stage's
//! snapshot (#ADR-0094), and the parsed objects beside them, a cache of the snapshots. A stage
//! ran when the run holds its snapshot, and `Run::through` folds the stages over them. A
//! caller stops after any stage and reads what the run holds. The report is written from the
//! run as far as it went.
//!
//! `docs/design/ARCHITECTURE.md`, "Shape".

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Write as _};

use camino::Utf8Path;
use zip::CompressionMethod;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;

use crate::archive::{self, Caps, RawSnapshot};
use crate::diag::{
    Cancel, Cancelled, Code, DiagnosticPolicy, Diagnostics, Halted, Parts, Render, Stage, Step,
    Steps,
};
use crate::env::Env;
use crate::facts::{FactInputs, Facts, FactsError};
use crate::fixups::{Changes, FixError, FixInputs, FixedMod, FixupSet, ObjectSet, TargetTypes};
use crate::game::{GameError, NameBook, Speller};
use crate::load::{
    Dating, Fault, FileProvenance, Inventory, KeptFile, LOAD_PARTS, LoadError, LoadOptions, Loader,
    ReworkTable, against,
};
use crate::model::{ByteView, ContentId, Mod, Spelled};
use crate::package::{
    BuildError, CheckCx, CheckSet, EvidenceDoc, InPlaceBuilder, Made, Overlay, Package,
    PackageWriter, PortBuilder, Refused, WriteError, WriterKind, permit,
};
use crate::port::{PlanError, Policy, Port, PortInputs, PortOptions, Targets, UnitTarget};
use crate::report::{AgainRow, AppliedRow, PackageRow, Report};
use crate::snapshot::{
    self, BuildSettings, BuildSnapshot, CheckSettings, CheckSnapshot, DecideSettings,
    DecideSnapshot, FactsSettings, FactsSnapshot, FixSettings, FixSnapshot, LoadSettings,
    LoadSnapshot, LoadState, PlanSettings, PlanSnapshot, RawCache, ResumeError, Setting, Snapshots,
    StageSettings,
};
use crate::store::{BlobStore, Layered, MemoryStore};

/// The name the second run of a repair reads the first run's package under.
const REPAIRED_NAME: &str = "repaired.fantome";

/// Every diagnostic code a run can raise, in the order of the modules that declare them.
/// `docs/design/CODES.md` lists the same codes.
pub fn codes() -> impl Iterator<Item = Code> {
    [
        archive::CODES,
        crate::game::CODES,
        crate::history::CODES,
        crate::load::CODES,
        crate::load::legacy::CODES,
        crate::load::moved::CODES,
        crate::load::dating::CODES,
        crate::load::inventory::CODES,
        crate::load::provenance::CODES,
        crate::load::bank::CODES,
        crate::fixups::CODES,
        crate::facts::CODES,
        crate::port::CODES,
        crate::package::CODES,
    ]
    .into_iter()
    .flatten()
    .copied()
}

/// Why a stage of a run gives no result.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// A fatal diagnostic ended the run, at this stage or an earlier one. The log holds it,
    /// and the report is written all the same.
    #[error(transparent)]
    Halted(#[from] Halted),
    /// The run's cancel ended a stage between two of its steps, this one or an earlier one
    /// (#ADR-0097). The report is written all the same.
    #[error(transparent)]
    Cancelled(#[from] Cancelled),
    /// The stage reads the result of a stage the run did not reach.
    #[error("{stage} reads what {needs} gives, and the run did not reach {needs}")]
    NotReached {
        /// The stage asked for.
        stage: Stage,
        /// The stage whose result it reads.
        needs: Stage,
    },
    /// A repair has no such stage (#ADR-0011).
    #[error("a repair has no stage {0}")]
    NotInARepair(Stage),
    /// Only a repair's package is taken through the run again (#ADR-0011).
    #[error("a port is not repaired again")]
    NotARepair,
    /// An input did not answer during `load`.
    #[error("an input of the run did not answer while the mod was loaded")]
    Load(#[source] LoadError),
    /// An input did not answer during `fix`.
    #[error("an input of the run did not answer while the mod was fixed")]
    Fix(#[source] FixError),
    /// An input did not answer during `facts`.
    #[error("an input of the run did not answer while the facts were gathered")]
    Facts(#[source] FactsError),
    /// An input did not answer during `plan`.
    #[error("an input of the run did not answer while the port was planned")]
    Plan(#[source] PlanError),
    /// An input did not answer during `build`.
    #[error("an input of the run did not answer while the package was built")]
    Build(#[source] Fault),
    /// The game did not read during `check`.
    #[error("the game did not read while the package was checked")]
    Check(#[source] GameError),
    /// The caller's policy refused the write. The log holds `write.blocked`.
    #[error(transparent)]
    Refused(#[from] Refused),
    /// A writer did not put the package on disk.
    #[error("the {} did not write", kind.as_str())]
    Write {
        /// What the writer writes.
        kind: WriterKind,
        /// Why it did not.
        #[source]
        source: WriteError,
    },
    /// The package of a repair did not pack as an archive in memory, for its second run.
    #[error("the package did not pack as an archive in memory")]
    Again(#[source] ZipError),
}

impl RunError {
    /// What the build's own error is to a run: its fatal diagnostic, its cancel, else the
    /// fault.
    fn of_build(error: BuildError) -> Self {
        match error {
            BuildError::Halted(halted) => RunError::Halted(halted),
            BuildError::Cancelled(cancelled) => RunError::Cancelled(cancelled),
            BuildError::Fault(fault) => RunError::Build(fault),
        }
    }

    /// What load's own error is to a run: its fatal diagnostic, its cancel, else the fault.
    fn of_load(error: LoadError) -> Self {
        match error {
            LoadError::Halted(halted) => RunError::Halted(halted),
            LoadError::Cancelled(cancelled) => RunError::Cancelled(cancelled),
            fault => RunError::Load(fault),
        }
    }

    /// What fix's own error is to a run: its cancel, else the fault.
    fn of_fix(error: FixError) -> Self {
        match error {
            FixError::Cancelled(cancelled) => RunError::Cancelled(cancelled),
            fault => RunError::Fix(fault),
        }
    }

    /// What the facts' own error is to a run: its cancel, else the fault.
    fn of_facts(error: FactsError) -> Self {
        match error {
            FactsError::Cancelled(cancelled) => RunError::Cancelled(cancelled),
            fault => RunError::Facts(fault),
        }
    }

    /// What the plan's own error is to a run: its cancel, else the fault.
    fn of_plan(error: PlanError) -> Self {
        match error {
            PlanError::Cancelled(cancelled) => RunError::Cancelled(cancelled),
            fault => RunError::Plan(fault),
        }
    }
}

/// What an earlier run of the same mod established and its package does not show again
/// (#ADR-0041).
///
/// An object a fixup brought to the target build is the game's own object of that build, so a
/// repaired mod dates later by itself. A restoration is known by its files at paths the rework
/// dropped, which a package leaves out where nothing names them (#ADR-0039).
#[derive(Debug, Clone)]
pub struct Evidence {
    /// The dating of the mod.
    pub dating: Dating,
    /// The WADs that restore a model from before its rework, by name, in lowercase.
    pub restored: BTreeSet<String>,
}

/// What a caller chose for the stages of a run, as data: each stage declares the part it
/// reads (#ADR-0094). The default of each set and table is its standard one.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    mode: Mode,
    fixups: FixupSet,
    checks: CheckSet,
    caps: Caps,
    keep_stale: bool,
    kept_files: Vec<KeptFile>,
    reworks: ReworkTable,
    policy: Policy,
    compared: Vec<Policy>,
    unit_targets: Vec<UnitTarget>,
    game_events_only: bool,
    keep_dds: bool,
    keep_other_slots: bool,
}

/// A snapshot a run starts from (#ADR-0094).
#[derive(Debug, Clone)]
pub enum Start {
    /// The raw snapshot: the run starts at `load`.
    Raw(Box<RawSnapshot>),
    /// The load snapshot: the run starts at `fix`.
    Load(Box<LoadSnapshot>),
    /// The snapshots through `fix`: the run starts at `facts` in a port, at `build` in a
    /// repair.
    Fix(Box<Snapshots>),
    /// The snapshots through `facts`: the run starts at `decide`. A port's alone.
    Facts(Box<Snapshots>),
    /// The snapshots through `decide`: the run starts at `plan`. A port's alone.
    Decide(Box<Snapshots>),
    /// The snapshots through `plan`: the run starts at `build`. A port's alone.
    Plan(Box<Snapshots>),
    /// The snapshots through `build`: the run starts at `check`.
    Build(Box<Snapshots>),
    /// The snapshots through `check`: `write` runs next.
    Check(Box<Snapshots>),
}

impl From<RawSnapshot> for Start {
    fn from(raw: RawSnapshot) -> Self {
        Start::Raw(Box::new(raw))
    }
}

impl From<LoadSnapshot> for Start {
    fn from(load: LoadSnapshot) -> Self {
        Start::Load(Box::new(load))
    }
}

impl Settings {
    /// The options `load` runs under, and a repair's textures are judged under.
    fn load_options(&self) -> LoadOptions {
        LoadOptions::new()
            .keep_stale(self.keep_stale)
            .keep_files(self.kept_files.clone())
            .reworks(self.reworks.clone())
            .file_fixups(self.fixups.file_fixups())
    }

    fn port_options(&self) -> PortOptions {
        PortOptions {
            keep_stale: self.keep_stale,
            kept_files: self.kept_files.clone(),
            game_events_only: self.game_events_only,
            keep_other_slots: self.keep_other_slots,
        }
    }

    /// The settings `load` reads.
    fn load_settings(&self) -> LoadSettings {
        LoadSettings::of(self.mode.snapshot(), &self.load_options())
    }

    /// The settings `fix` reads.
    fn fix_settings(&self) -> FixSettings {
        FixSettings::of(&self.fixups)
    }

    /// The settings `facts` reads.
    fn facts_settings(&self) -> FactsSettings {
        FactsSettings {
            reworks: self.reworks.clone(),
        }
    }

    /// The settings `decide` reads.
    fn decide_settings(&self) -> DecideSettings {
        DecideSettings {
            policy: self.policy,
            compared: self.compared.clone(),
            unit_targets: self.unit_targets.clone(),
        }
    }

    /// The settings `plan` reads.
    fn plan_settings(&self) -> PlanSettings {
        PlanSettings::of(&self.port_options())
    }

    /// The settings `build` reads.
    fn build_settings(&self) -> BuildSettings {
        BuildSettings {
            keep_dds: self.keep_dds,
            keep_stale: self.keep_stale,
            kept_files: self.kept_files.clone(),
        }
    }

    /// The settings `check` reads.
    fn check_settings(&self) -> CheckSettings {
        CheckSettings::of(&self.checks)
    }

    /// The settings a stage reads, as its snapshot declares them (#ADR-0094).
    pub fn declared(&self, stage: Stage) -> StageSettings {
        match stage {
            Stage::Read => StageSettings::Read(self.caps),
            Stage::Load => StageSettings::Load(self.load_settings()),
            Stage::Fix => StageSettings::Fix(self.fix_settings()),
            Stage::Facts => StageSettings::Facts(self.facts_settings()),
            Stage::Decide => StageSettings::Decide(self.decide_settings()),
            Stage::Plan => StageSettings::Plan(self.plan_settings()),
            Stage::Build => StageSettings::Build(self.build_settings()),
            Stage::Check => StageSettings::Check(self.check_settings()),
            Stage::Write => StageSettings::Write,
        }
    }

    /// The first stage of a run under these settings, in run order, whose declared settings
    /// differ from `other`'s: the stage a rerun under `other` resumes from, after the
    /// snapshot before it (#ADR-0094). None when every stage reads the same.
    pub fn earliest_changed(&self, other: &Self) -> Option<Stage> {
        let mut stages = self.mode.stages().iter().copied();
        stages.find(|&stage| self.declared(stage) != other.declared(stage))
    }
}

/// What a run builds its package from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    /// The plan of a port.
    #[default]
    Port,
    /// The fixed mod, in place (#ADR-0011).
    InPlace,
}

impl Mode {
    /// The mode as a snapshot names it.
    fn snapshot(self) -> snapshot::Mode {
        match self {
            Mode::Port => snapshot::Mode::Port,
            Mode::InPlace => snapshot::Mode::Repair,
        }
    }

    /// The stages of a run of the mode, in order.
    fn stages(self) -> &'static [Stage] {
        match self {
            Mode::Port => &[
                Stage::Read,
                Stage::Load,
                Stage::Fix,
                Stage::Facts,
                Stage::Decide,
                Stage::Plan,
                Stage::Build,
                Stage::Check,
                Stage::Write,
            ],
            Mode::InPlace => &[
                Stage::Read,
                Stage::Load,
                Stage::Fix,
                Stage::Build,
                Stage::Check,
                Stage::Write,
            ],
        }
    }

    /// What a run of the mode does to the mod it packages.
    fn made(self) -> Made {
        match self {
            Mode::Port => Made::Ported,
            Mode::InPlace => Made::Repaired,
        }
    }
}

/// Ports mods against one environment: each unit the author made, over the game's own skin at
/// its target. The stages of its runs are read, load, fix, facts, decide, plan, build, check
/// and write.
#[derive(Debug, Clone)]
pub struct Porter<'a> {
    env: &'a Env,
    settings: Settings,
    cancel: Option<Cancel>,
}

impl<'a> Porter<'a> {
    /// A porter with the standard fixups and checks, the default policy and caps, the reworks
    /// of #GAME-CHANGE-0001, and stale copies left out.
    pub fn new(env: &'a Env) -> Self {
        Self {
            env,
            settings: Settings::default(),
            cancel: None,
        }
    }

    /// Hands this cancel to each of its runs ([`Run::cancel`]). A cancel is no setting: no
    /// snapshot names it.
    #[must_use]
    pub fn cancel(mut self, cancel: Cancel) -> Self {
        self.cancel = Some(cancel);
        self
    }

    /// Chooses each unit's target under this policy.
    #[must_use]
    pub fn policy(mut self, policy: Policy) -> Self {
        self.settings.policy = policy;
        self
    }

    /// Gives these units their targets in place of the policy's (#ADR-0098). A target that
    /// names no unit of the mod changes nothing, and the settings hold it as given.
    #[must_use]
    pub fn unit_targets(mut self, given: Vec<UnitTarget>) -> Self {
        self.settings.unit_targets = given;
        self
    }

    /// Records, beside each decision, what these policies choose. They raise nothing.
    #[must_use]
    pub fn compare(mut self, policies: Vec<Policy>) -> Self {
        self.settings.compared = policies;
        self
    }

    /// Runs these fixups, on the files in `load` and on the bins in `fix`.
    #[must_use]
    pub fn fixups(mut self, fixups: FixupSet) -> Self {
        self.settings.fixups = fixups;
        self
    }

    /// Runs these checks on the package.
    #[must_use]
    pub fn checks(mut self, checks: CheckSet) -> Self {
        self.settings.checks = checks;
        self
    }

    /// Reads an archive under these caps.
    #[must_use]
    pub fn caps(mut self, caps: Caps) -> Self {
        self.settings.caps = caps;
        self
    }

    /// Carries stale copies as the mod's own files, or leaves them out (#ADR-0023).
    #[must_use]
    pub fn keep_stale(mut self, keep: bool) -> Self {
        self.settings.keep_stale = keep;
        self
    }

    /// Carries these files as the mod's own where they are stale copies (#ADR-0098). A file
    /// named that is no stale copy is judged as any other, and the settings hold it as given.
    #[must_use]
    pub fn keep_files(mut self, kept: Vec<KeptFile>) -> Self {
        self.settings.kept_files = kept;
        self
    }

    /// Judges restorations and reworked champions against these reworks.
    #[must_use]
    pub fn reworks(mut self, reworks: ReworkTable) -> Self {
        self.settings.reworks = reworks;
        self
    }

    /// Cuts a bank's events from the game's events bank only (#ADR-0030).
    #[must_use]
    pub fn game_events_only(mut self, only: bool) -> Self {
        self.settings.game_events_only = only;
        self
    }

    /// Ships the units at slots other than their champion's target too, each at its own slot,
    /// or leaves them (#ADR-0082).
    #[must_use]
    pub fn keep_other_slots(mut self, keep: bool) -> Self {
        self.settings.keep_other_slots = keep;
        self
    }

    /// Ships each DDS as it is, not put in a TEX container (#ADR-0055).
    #[must_use]
    pub fn keep_dds(mut self, keep: bool) -> Self {
        self.settings.keep_dds = keep;
        self
    }

    /// A run of one archive, with nothing read, reading the porter's cancel.
    pub fn run(&self) -> Run<'_> {
        Run::new(self.env, &self.settings, self.cancel.as_ref())
    }

    /// What the porter's runs are run under.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Takes an archive's bytes through every stage but `write`. A fatal diagnostic or a
    /// cancel ends the run early and is no error: the run comes back as far as it went.
    pub fn port(&self, bytes: &[u8], file_name: &str) -> Result<Run<'_>, RunError> {
        let mut run = self.run();
        run.all(bytes, file_name)?;
        Ok(run)
    }

    /// A run started from a snapshot, its bytes in `store` (#ADR-0094).
    pub fn resume(
        &self,
        from: impl Into<Start>,
        store: impl BlobStore + 'static,
    ) -> Result<Run<'_>, ResumeError> {
        self.run().in_store(store).resume(from)
    }
}

/// Repairs mods against one environment: the mod as it is, at its own slots and paths, with
/// its bins fixed for the target build (#ADR-0011). The stages of its runs are read, load,
/// fix, build, check and write.
#[derive(Debug, Clone)]
pub struct Repairer<'a> {
    env: &'a Env,
    settings: Settings,
    cancel: Option<Cancel>,
}

impl<'a> Repairer<'a> {
    /// A repairer with the standard fixups and checks, the default caps, the reworks of
    /// #GAME-CHANGE-0001, and stale copies left out.
    pub fn new(env: &'a Env) -> Self {
        Self {
            env,
            settings: Settings {
                mode: Mode::InPlace,
                ..Settings::default()
            },
            cancel: None,
        }
    }

    /// Hands this cancel to each of its runs ([`Run::cancel`]). A cancel is no setting: no
    /// snapshot names it.
    #[must_use]
    pub fn cancel(mut self, cancel: Cancel) -> Self {
        self.cancel = Some(cancel);
        self
    }

    /// Runs these fixups, on the files in `load` and on the bins in `fix`.
    #[must_use]
    pub fn fixups(mut self, fixups: FixupSet) -> Self {
        self.settings.fixups = fixups;
        self
    }

    /// Runs these checks on the package.
    #[must_use]
    pub fn checks(mut self, checks: CheckSet) -> Self {
        self.settings.checks = checks;
        self
    }

    /// Reads an archive under these caps.
    #[must_use]
    pub fn caps(mut self, caps: Caps) -> Self {
        self.settings.caps = caps;
        self
    }

    /// Carries stale copies as the mod's own files, or leaves them out (#ADR-0023).
    #[must_use]
    pub fn keep_stale(mut self, keep: bool) -> Self {
        self.settings.keep_stale = keep;
        self
    }

    /// Carries these files as the mod's own where they are stale copies (#ADR-0098). A file
    /// named that is no stale copy is judged as any other, and the settings hold it as given.
    #[must_use]
    pub fn keep_files(mut self, kept: Vec<KeptFile>) -> Self {
        self.settings.kept_files = kept;
        self
    }

    /// Ships each DDS as it is, not put in a TEX container (#ADR-0055).
    #[must_use]
    pub fn keep_dds(mut self, keep: bool) -> Self {
        self.settings.keep_dds = keep;
        self
    }

    /// Judges restorations against these reworks.
    #[must_use]
    pub fn reworks(mut self, reworks: ReworkTable) -> Self {
        self.settings.reworks = reworks;
        self
    }

    /// A run of one archive, with nothing read, reading the repairer's cancel.
    pub fn run(&self) -> Run<'_> {
        Run::new(self.env, &self.settings, self.cancel.as_ref())
    }

    /// What the repairer's runs are run under.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Takes an archive's bytes through every stage but `write`. A fatal diagnostic or a
    /// cancel ends the run early and is no error: the run comes back as far as it went.
    pub fn repair(&self, bytes: &[u8], file_name: &str) -> Result<Run<'_>, RunError> {
        let mut run = self.run();
        run.all(bytes, file_name)?;
        Ok(run)
    }

    /// A run started from a snapshot, its bytes in `store` (#ADR-0094).
    pub fn resume(
        &self,
        from: impl Into<Start>,
        store: impl BlobStore + 'static,
    ) -> Result<Run<'_>, ResumeError> {
        self.run().in_store(store).resume(from)
    }
}

/// The mod as far as a run took it.
#[derive(Default)]
enum Reached {
    /// No mod: the archive is not loaded.
    #[default]
    Nothing,
    /// `load` ended at a fatal diagnostic: the mod as far as it went.
    Partial(Mod),
    /// The mod as `load` left it.
    Loaded(Mod),
    /// The mod as the fixups left it.
    Fixed(FixedMod),
}

impl Reached {
    fn module(&self) -> Option<&Mod> {
        match self {
            Reached::Nothing => None,
            Reached::Partial(module) | Reached::Loaded(module) => Some(module),
            Reached::Fixed(fixed) => Some(fixed),
        }
    }

    /// The fixed mod, for a stage that reads it.
    fn fixed(&self, stage: Stage) -> Result<&FixedMod, RunError> {
        match self {
            Reached::Fixed(fixed) => Ok(fixed),
            _ => Err(RunError::NotReached {
                stage,
                needs: Stage::Fix,
            }),
        }
    }
}

/// A stage's result the run holds, for a later stage that reads it.
fn held<T>(result: Option<&T>, stage: Stage, needs: Stage) -> Result<&T, RunError> {
    result.ok_or(RunError::NotReached { stage, needs })
}

/// What a run's memory layer reads through to: the caller's store over the cache's ingest
/// store, each an empty memory store unless given.
type Under = Layered<Box<dyn BlobStore>, Box<dyn BlobStore>>;

/// One archive taken through the stages, with everything the stages gave.
///
/// A stage runs once: asked again, its method gives the result the run holds. After a fatal
/// diagnostic every stage answers [`RunError::Halted`]. A stage whose input the run does not
/// hold answers [`RunError::NotReached`].
pub struct Run<'a> {
    env: &'a Env,
    settings: &'a Settings,
    mode: Mode,
    known: Option<Evidence>,
    log: Diagnostics,
    book: NameBook,
    /// The run's memory layer over the caller's store, over the cache's ingest store
    /// (#ADR-0093): every byte a stage makes goes into the layer, and the caller flushes it.
    store: Layered<MemoryStore, Under>,
    /// The raw snapshots `read` takes in place of reading an archive, and leaves (#ADR-0095).
    cache: Option<RawCache>,
    /// Where the findings of `read` begin in the log.
    first: usize,
    raw: Option<RawSnapshot>,
    loaded: Option<LoadSnapshot>,
    fix_snapshot: Option<FixSnapshot>,
    facts_snapshot: Option<FactsSnapshot>,
    decide_snapshot: Option<DecideSnapshot>,
    plan_snapshot: Option<PlanSnapshot>,
    build_snapshot: Option<BuildSnapshot>,
    check_snapshot: Option<CheckSnapshot>,
    module: Reached,
    dating: Option<Dating>,
    redated: Option<Dating>,
    provenance: Option<FileProvenance>,
    changes: Option<Changes>,
    facts: Option<Facts>,
    plan: Option<Port>,
    package: Option<Package>,
    written: bool,
    again: Option<AgainRow>,
}

impl<'a> Run<'a> {
    fn new(env: &'a Env, settings: &'a Settings, cancel: Option<&Cancel>) -> Self {
        let mut log = Diagnostics::new();
        if let Some(cancel) = cancel {
            log.cancel_on(cancel.clone());
        }
        Self {
            env,
            settings,
            mode: settings.mode,
            known: None,
            log,
            book: NameBook::new(),
            store: Layered::new(
                MemoryStore::new(),
                Layered::new(Box::new(MemoryStore::new()), Box::new(MemoryStore::new())),
            ),
            cache: None,
            first: 0,
            raw: None,
            loaded: None,
            fix_snapshot: None,
            facts_snapshot: None,
            decide_snapshot: None,
            plan_snapshot: None,
            build_snapshot: None,
            check_snapshot: None,
            module: Reached::Nothing,
            dating: None,
            redated: None,
            provenance: None,
            changes: None,
            facts: None,
            plan: None,
            package: None,
            written: false,
            again: None,
        }
    }

    /// Starts from a log that holds what the inputs raised when the environment was built.
    /// The run's cancel goes with it to a log that has none.
    #[must_use]
    pub fn log_from(mut self, mut log: Diagnostics) -> Self {
        if let (None, Some(cancel)) = (log.cancel(), self.log.cancel()) {
            log.cancel_on(cancel.clone());
        }
        self.log = log;
        self
    }

    /// Reads this cancel between the steps of each stage (#ADR-0097). Set, it ends the stage
    /// under way after the step it is on: the stage gives no result and writes no snapshot,
    /// and no stage after it runs.
    #[must_use]
    pub fn cancel(mut self, cancel: Cancel) -> Self {
        self.log.cancel_on(cancel);
        self
    }

    /// Judges the mod with what an earlier run of it established: its dating in place of the
    /// one the archive shows, and its restorations beside the ones `load` finds.
    #[must_use]
    pub fn known(mut self, evidence: Evidence) -> Self {
        self.known = Some(evidence);
        self
    }

    /// Reads through to this store under the run's memory layer, in place of an empty one
    /// (#ADR-0093). Nothing is written into it: the caller flushes the layer
    /// ([`Run::into_layer`]).
    #[must_use]
    pub fn in_store(mut self, store: impl BlobStore + 'static) -> Self {
        let (layer, under) = self.store.into_parts();
        let (_, ingest) = under.into_parts();
        self.store = Layered::new(layer, Layered::new(Box::new(store), ingest));
        self
    }

    /// Reads archives through this cache (#ADR-0095): `read` takes the raw snapshot it holds of
    /// an archive in place of reading it, and puts what it reads of another into the cache's
    /// ingest store with the snapshot beside it. The run reads through to the ingest store
    /// under the caller's store, and writes nothing else into it.
    #[must_use]
    pub fn in_cache(mut self, cache: RawCache) -> Self {
        let (layer, under) = self.store.into_parts();
        let (store, _) = under.into_parts();
        let ingest: Box<dyn BlobStore> = Box::new(cache.store().clone());
        self.store = Layered::new(layer, Layered::new(store, ingest));
        self.cache = Some(cache);
        self
    }

    /// Ends a stage's method where a fatal diagnostic or a cancel ended the run.
    fn going(&self) -> Result<(), RunError> {
        if let Some(halted) = self.log.halted() {
            return Err(halted.into());
        }
        self.log
            .cancelled()
            .map_or(Ok(()), |cancelled| Err(cancelled.into()))
    }

    /// The stage `read`: the archive from its bytes, which are borrowed for the call, read
    /// into the run's store. The run holds the raw snapshot (#ADR-0094). `file_name` is for
    /// display and is never opened.
    ///
    /// With a cache ([`Run::in_cache`]) the raw snapshot it holds of the archive is the
    /// result, its findings raised again, and no entry is inflated. An archive it does not
    /// hold is read into the cache's ingest store, and its snapshot kept there.
    pub fn read(&mut self, bytes: &[u8], file_name: &str) -> Result<&RawSnapshot, RunError> {
        self.going()?;
        self.first = self.log.len();
        if let Some(raw) = self.cached(bytes, file_name) {
            return Ok(self.raw.insert(raw));
        }
        let mut sink = self.log.at(Stage::Read);
        let caps = &self.settings.caps;
        let raw = match &self.cache {
            None => archive::read(bytes, file_name, caps, &self.store, &mut sink)?,
            Some(cache) => {
                let raw = archive::read(bytes, file_name, caps, cache.store(), &mut sink)?;
                if let Err(error) = cache.put(&raw) {
                    // The run goes on: the next run of the archive reads it again.
                    tracing::warn!(%error, "a raw snapshot is not kept in the cache");
                }
                raw
            }
        };
        Ok(self.raw.insert(raw))
    }

    /// The raw snapshot the run's cache holds of an archive, its findings raised again, under
    /// the caller's name for the archive. None without a cache, for an archive it does not
    /// hold, and for a snapshot whose findings name a code this program does not declare.
    fn cached(&mut self, bytes: &[u8], file_name: &str) -> Option<RawSnapshot> {
        let cache = self.cache.as_ref()?;
        let mut raw = cache.get(ContentId::of(bytes), &self.settings.caps)?;
        let codes: Vec<Code> = codes().collect();
        self.log.restore(&raw.findings, &codes).ok()?;
        raw.meta.file_name = Spelled::new(file_name);
        Some(raw)
    }

    /// The stage `read`, of an archive file.
    pub fn read_file(&mut self, path: &Utf8Path) -> Result<&RawSnapshot, RunError> {
        self.going()?;
        self.first = self.log.len();
        let mut sink = self.log.at(Stage::Read);
        let bytes = archive::file_bytes(path, &self.settings.caps, &mut sink)?;
        self.read(&bytes, archive::file_name_of(path))
    }

    /// Starts the run from a snapshot in place of `read`, with the bytes it names in the run's
    /// store (#ADR-0094). From the raw snapshot the run holds it as `read` gives it, and
    /// `load` runs next. From the load snapshot the run holds the mod as `load` left it, its
    /// bins parsed again from the store, and `fix` runs next. From the snapshots after a later
    /// stage the run holds what that stage left: the objects of the bins `fix` wrote again
    /// read from their new bytes, and each stage's results read back from its snapshot.
    ///
    /// The raw snapshot is refused when its reading is not this program's or its caps are not
    /// the run's; a later snapshot when its format, its program, an input its stages read or a
    /// setting they read is not this run's, each snapshot from the load on judged by what its
    /// own stage declares. A refused run is dropped, and the caller starts again from an
    /// earlier snapshot.
    pub fn resume(mut self, from: impl Into<Start>) -> Result<Self, ResumeError> {
        let ended = self.log.halted().is_some() || self.log.cancelled().is_some();
        let started = self.raw.is_some() || self.loaded.is_some() || ended;
        if started || !matches!(self.module, Reached::Nothing) {
            return Err(ResumeError::Started);
        }
        self.first = self.log.len();
        let codes: Vec<Code> = codes().collect();
        match from.into() {
            Start::Raw(raw) => {
                if raw.version != archive::RAW_VERSION {
                    return Err(ResumeError::Reading {
                        found: raw.version,
                        reads: archive::RAW_VERSION,
                    });
                }
                if raw.caps != self.settings.caps {
                    return Err(ResumeError::Setting(Setting::Caps));
                }
                self.log.restore(&raw.findings, &codes)?;
                self.raw = Some(*raw);
            }
            Start::Load(load) => {
                let module = self.reopen_load(&load)?;
                self.log.restore(load.findings(), &codes)?;
                self.module = Reached::Loaded(module);
                self.loaded = Some(*load);
            }
            Start::Fix(snapshots) => self.reopen_run(*snapshots, Stage::Fix, &codes)?,
            Start::Facts(snapshots) => self.reopen_run(*snapshots, Stage::Facts, &codes)?,
            Start::Decide(snapshots) => self.reopen_run(*snapshots, Stage::Decide, &codes)?,
            Start::Plan(snapshots) => self.reopen_run(*snapshots, Stage::Plan, &codes)?,
            Start::Build(snapshots) => self.reopen_run(*snapshots, Stage::Build, &codes)?,
            Start::Check(snapshots) => self.reopen_run(*snapshots, Stage::Check, &codes)?,
        }
        Ok(self)
    }

    /// The run as the snapshots left it after `last`, a stage of the run group: each stage's
    /// state reopened from its snapshot in order, the findings of the last one restored.
    fn reopen_run(
        &mut self,
        snapshots: Snapshots,
        last: Stage,
        codes: &[Code],
    ) -> Result<(), ResumeError> {
        if !self.mode.stages().contains(&last) {
            return Err(ResumeError::NotInARepair(last));
        }
        let Snapshots {
            load,
            fix,
            facts,
            decide,
            plan,
            build,
            check,
        } = snapshots;
        let stages = self.mode.stages();
        // A stage the run's mode has not keeps no snapshot to resume after.
        let through = |stage: Stage| stages.contains(&stage) && stage <= last;
        let fix = snapshot::after(fix, Stage::Fix)?;
        let facts = snapshot::through(facts, Stage::Facts, through)?;
        let decide = snapshot::through(decide, Stage::Decide, through)?;
        let plan = snapshot::through(plan, Stage::Plan, through)?;
        let build = snapshot::through(build, Stage::Build, through)?;
        let check = snapshot::through(check, Stage::Check, through)?;
        let module = self.reopen_load(&load)?;
        let fixed = self.reopen_fix(module, &load, &fix)?;
        let mut findings = fix.findings();
        if let Some(facts) = &facts {
            facts.holds(&self.settings.facts_settings())?;
            let found = facts.found().clone();
            let reopened = Facts::reopen(&fixed, self.env, found);
            self.facts = Some(reopened.map_err(ResumeError::Facts)?);
            findings = facts.findings();
        }
        if let Some(decide) = &decide {
            decide.holds(&self.settings.decide_settings())?;
            findings = decide.findings();
        }
        if let Some(plan) = &plan {
            plan.holds(&self.settings.plan_settings())?;
            self.plan = Some(plan.port(&self.store)?);
            self.book = plan.names().clone();
            findings = plan.findings();
        }
        if let Some(build) = &build {
            build.holds(&self.settings.build_settings())?;
            self.package = Some(build.package(&self.store)?);
            findings = build.findings();
            if let Some(check) = &check {
                check.holds(&self.settings.check_settings())?;
                let patches = build.part().patches.len();
                let (applied, bins) = (check.applied().len(), check.bins().len());
                if applied != patches || bins != patches {
                    let what = format!("{applied} outcomes and {bins} bins of {patches} patches");
                    return Err(ResumeError::Malformed(what));
                }
                findings = check.findings();
            }
        }
        self.log.restore(findings, codes)?;
        self.module = Reached::Fixed(fixed);
        self.loaded = Some(load);
        self.fix_snapshot = Some(fix);
        self.facts_snapshot = facts;
        self.decide_snapshot = decide;
        self.plan_snapshot = plan;
        self.build_snapshot = build;
        self.check_snapshot = check;
        Ok(())
    }

    /// The mod as the load snapshot left it, and what the run holds once `load` ends: the
    /// names, the dating, the inventory and the file provenance.
    fn reopen_load(&mut self, load: &LoadSnapshot) -> Result<Mod, ResumeError> {
        let inputs = snapshot::LoadInputs::of(self.env);
        load.holds(&inputs, &self.settings.load_settings())?;
        let reopened = load.reopen(&self.store)?;
        self.book = reopened.names;
        self.dating = Some(reopened.dating);
        self.provenance = reopened.provenance;
        Ok(reopened.module)
    }

    /// The mod as the fix snapshot left it, from the mod as `load` left it, and what the run
    /// holds once `fix` ends: the names and the changes.
    fn reopen_fix(
        &mut self,
        module: Mod,
        load: &LoadSnapshot,
        fix: &FixSnapshot,
    ) -> Result<FixedMod, ResumeError> {
        fix.holds(
            &snapshot::FixInputs::of(self.env),
            &self.settings.fix_settings(),
        )?;
        let rewritten = fix.rewritten(load)?.into_iter().map(|(file, content)| {
            let bytes = self.store.get(content).ok_or(ResumeError::Store(content))?;
            Ok((file, bytes))
        });
        let rewritten = rewritten.collect::<Result<Vec<_>, ResumeError>>()?;
        let fixed = FixedMod::reopen(module, rewritten).map_err(ResumeError::Malformed)?;
        self.changes = Some(fix.changes()?);
        self.book = fix.names().clone();
        Ok(fixed)
    }

    /// The stage `load`: the mod from the raw snapshot and the store, then what is known of
    /// its files before any value is read. A porter's run gathers the dating, the inventory
    /// and the file provenance. A repairer's run gathers the dating alone: nothing of a
    /// repair reads the other two. The run keeps the load snapshot beside the raw one.
    pub fn load(&mut self) -> Result<&Mod, RunError> {
        self.going()?;
        if matches!(self.module, Reached::Nothing) && self.raw.is_some() {
            self.load_raw()?;
        }
        match &self.module {
            Reached::Loaded(module) => Ok(module),
            Reached::Fixed(fixed) => Ok(fixed),
            Reached::Nothing | Reached::Partial(_) => Err(RunError::NotReached {
                stage: Stage::Load,
                needs: Stage::Read,
            }),
        }
    }

    fn load_raw(&mut self) -> Result<(), RunError> {
        let env = self.env;
        let book = std::mem::take(&mut self.book);
        let options = self.settings.load_options();
        let mut loader = Loader::new(env).options(options).book(book);
        let mut sink = self.log.at(Stage::Load);
        let raw = held(self.raw.as_ref(), Stage::Load, Stage::Read)?;
        // A repair takes no inventory and no file provenance, the last two parts.
        let mut parts = Parts::new(match self.mode {
            Mode::Port => LOAD_PARTS,
            Mode::InPlace => &LOAD_PARTS[..LOAD_PARTS.len() - 2],
        });
        let opened = parts.begin(&mut sink).map_err(LoadError::from);
        let mut module = match opened.and_then(|()| loader.open(raw, &self.store)) {
            Ok(module) => module,
            Err(error) => {
                self.book = loader.into_names();
                return Err(RunError::of_load(error));
            }
        };
        let known = self.known.as_ref();
        let mut steps = || -> Result<(), LoadError> {
            parts.begin(&mut sink)?;
            loader.fix_files(&mut module, &mut sink)?;
            parts.begin(&mut sink)?;
            loader.take_game_bins(&mut module, &mut sink)?;
            if let Some(known) = known {
                let wads = module.wads.iter();
                let restored: Vec<_> = wads
                    .filter(|(_, wad)| known.restored.contains(wad.name.lower()))
                    .map(|(id, _)| id)
                    .collect();
                module.restorations.extend(restored);
            }
            parts.begin(&mut sink)?;
            loader.survey(&mut module, &mut sink)?;
            parts.begin(&mut sink)?;
            loader.resolve(&mut module, &mut sink)?;
            Ok(())
        };
        let done = steps();
        self.book = loader.into_names();
        match done {
            Ok(()) => {}
            Err(ended @ (LoadError::Halted(_) | LoadError::Cancelled(_))) => {
                self.module = Reached::Partial(module);
                return Err(RunError::of_load(ended));
            }
            Err(fault) => return Err(RunError::Load(fault)),
        }

        let (found, taken) = match self.files_known(&module, &mut parts) {
            Ok(known) => known,
            Err(error) => {
                if matches!(error, RunError::Cancelled(_)) {
                    self.module = Reached::Partial(module);
                }
                return Err(error);
            }
        };
        self.loaded = Some(self.snapshot_of(&module, &found, taken.as_ref()));
        match &self.known {
            Some(known) => {
                self.dating = Some(known.dating.clone());
                self.redated = Some(found);
            }
            None => self.dating = Some(found),
        }
        self.module = Reached::Loaded(module);
        Ok(())
    }

    /// The steps of `load` after the mod is resolved, what is known of its files: the dating,
    /// and in a port the inventory and the file provenance, which the run holds.
    fn files_known(
        &mut self,
        module: &Mod,
        parts: &mut Parts,
    ) -> Result<(Dating, Option<Inventory>), RunError> {
        let env = self.env;
        let mut sink = self.log.at(Stage::Load);
        parts.begin(&mut sink)?;
        let found = Dating::find(module, env.history());
        let found = found.map_err(|error| RunError::Load(LoadError::History(error)))?;
        if self.mode == Mode::InPlace {
            return Ok((found, None));
        }
        let speller = Speller::new(&self.book, env.names());
        parts.begin(&mut sink)?;
        let inventory = Inventory::take(module, &speller);
        inventory.report(&mut sink);
        parts.begin(&mut sink)?;
        let provenance = FileProvenance::find(module, &against(env, &speller));
        let provenance = provenance.map_err(|fault| RunError::Load(fault.into()))?;
        provenance.report(&mut sink);
        self.provenance = Some(provenance);
        Ok((found, Some(inventory)))
    }

    /// The load snapshot of the mod `load` left, at the dating `load` found, with what the
    /// archive carries in a port.
    fn snapshot_of(
        &self,
        module: &Mod,
        dating: &Dating,
        inventory: Option<&Inventory>,
    ) -> LoadSnapshot {
        let raw = self.raw.as_ref();
        let state = LoadState {
            module,
            dating,
            inventory,
            provenance: self.provenance.as_ref(),
            names: &self.book,
            extras: raw.map_or(&[], |raw| raw.extras.as_slice()),
            image: raw.and_then(|raw| raw.image),
        };
        let findings = self.log.logged_from(self.first);
        LoadSnapshot::of(
            &state,
            self.env,
            self.settings.load_settings(),
            findings,
            &self.store,
        )
    }

    /// The stage `fix`: the fixups on the bins applied to every loaded object (#ADR-0002).
    /// The run holds what they changed.
    pub fn fix(&mut self) -> Result<&FixedMod, RunError> {
        self.going()?;
        if matches!(self.module, Reached::Loaded(_)) {
            let dating = held(self.dating.as_ref(), Stage::Fix, Stage::Load)?;
            if let Reached::Loaded(module) = std::mem::take(&mut self.module) {
                let mut inputs = FixInputs {
                    env: self.env,
                    dating,
                    names: &mut self.book,
                };
                let mut sink = self.log.at(Stage::Fix);
                let set = &self.settings.fixups;
                match set.apply_or_keep(module, &ObjectSet::every(), &mut inputs, &mut sink) {
                    Ok((fixed, changes)) => {
                        let snapshot = self.fix_snapshot_of(&fixed, &changes);
                        self.module = Reached::Fixed(fixed);
                        self.changes = Some(changes);
                        self.fix_snapshot = Some(snapshot?);
                    }
                    // The report of a run ended here still holds what `load` gave.
                    Err(unfixed) => {
                        self.module = Reached::Loaded(unfixed.module);
                        return Err(RunError::of_fix(unfixed.fault));
                    }
                }
            }
        }
        match &self.module {
            Reached::Fixed(fixed) => Ok(fixed),
            _ => Err(RunError::NotReached {
                stage: Stage::Fix,
                needs: Stage::Load,
            }),
        }
    }

    /// The fix snapshot of the mod `fix` left, the bins it wrote again put into the run's
    /// store.
    fn fix_snapshot_of(
        &self,
        fixed: &FixedMod,
        changes: &Changes,
    ) -> Result<FixSnapshot, RunError> {
        let load = held(self.loaded.as_ref(), Stage::Fix, Stage::Load)?;
        let rewritten = fixed.rewritten(changes).into_iter();
        let rewritten = rewritten.map(|(file, bin)| (file, self.store.put(bin.bytes.into())));
        let declared = (
            snapshot::FixInputs::of(self.env),
            self.settings.fix_settings(),
        );
        let findings = self.log.logged_from(self.first);
        Ok(FixSnapshot::of(
            load,
            &rewritten.collect(),
            changes,
            &self.book,
            declared,
            findings,
        ))
    }

    /// Ends a stage's method on a repairer's run, which has no such stage.
    fn of_a_port(&self, stage: Stage) -> Result<(), RunError> {
        match self.mode {
            Mode::Port => Ok(()),
            Mode::InPlace => Err(RunError::NotInARepair(stage)),
        }
    }

    /// The stage `facts`: what is true of the fixed mod.
    pub fn gather(&mut self) -> Result<&Facts, RunError> {
        self.going()?;
        self.of_a_port(Stage::Facts)?;
        if self.facts_snapshot.is_none() {
            let fixed = self.module.fixed(Stage::Facts)?;
            let speller = Speller::new(&self.book, self.env.names());
            let inputs = FactInputs {
                env: self.env,
                dating: held(self.dating.as_ref(), Stage::Facts, Stage::Load)?,
                files: held(self.provenance.as_ref(), Stage::Facts, Stage::Load)?,
                names: &speller,
                reworks: &self.settings.reworks,
            };
            let mut sink = self.log.at(Stage::Facts);
            let facts = Facts::gather(fixed, &inputs, &mut sink).map_err(RunError::of_facts)?;
            facts.report(&mut sink);
            let findings = self.log.logged_from(self.first);
            let snapshot = FactsSnapshot::of(&facts, self.settings.facts_settings(), findings);
            self.facts_snapshot = Some(snapshot);
            self.facts = Some(facts);
        }
        held(self.facts.as_ref(), Stage::Facts, Stage::Fix)
    }

    /// The stage `decide`: the policy's choice of a target for each unit. Only the porter's
    /// policy raises diagnostics. What the compared policies choose is recorded beside it.
    pub fn decide(&mut self) -> Result<&Targets, RunError> {
        self.going()?;
        self.of_a_port(Stage::Decide)?;
        if self.decide_snapshot.is_none() {
            let facts = held(self.facts.as_ref(), Stage::Decide, Stage::Facts)?;
            // A step is a policy's decisions: the porter's, then each compared with it.
            let mut sink = self.log.at(Stage::Decide);
            let policy = self.settings.policy;
            let mut steps = Steps::new(1 + self.settings.compared.len());
            let named = |policy: &Policy| Step::Policy {
                policy: policy.label(),
            };
            steps.begin(&mut sink, || named(&policy))?;
            let mut targets = policy.decide_with(facts, &self.settings.unit_targets);
            targets.report(&mut sink);
            for compared in &self.settings.compared {
                steps.begin(&mut sink, || named(compared))?;
                targets = targets.compared(facts, std::slice::from_ref(compared));
            }
            let findings = self.log.logged_from(self.first);
            let snapshot = DecideSnapshot::of(targets, self.settings.decide_settings(), findings);
            self.decide_snapshot = Some(snapshot);
        }
        held(self.targets(), Stage::Decide, Stage::Facts)
    }

    /// The stage `plan`: the port of the fixed mod at its targets. The run learns the names
    /// the plan makes, so a message spells them.
    pub fn plan(&mut self) -> Result<&Port, RunError> {
        self.going()?;
        self.of_a_port(Stage::Plan)?;
        if self.plan_snapshot.is_none() {
            let options = self.settings.port_options();
            let speller = Speller::new(&self.book, self.env.names());
            let inputs = PortInputs {
                fixed: self.module.fixed(Stage::Plan)?,
                facts: held(self.facts.as_ref(), Stage::Plan, Stage::Facts)?,
                targets: held(
                    self.decide_snapshot.as_ref().map(DecideSnapshot::targets),
                    Stage::Plan,
                    Stage::Decide,
                )?,
                env: self.env,
                files: held(self.provenance.as_ref(), Stage::Plan, Stage::Load)?,
                names: &speller,
                options: &options,
            };
            let port = Port::plan(&inputs, &mut self.log.at(Stage::Plan));
            let port = port.map_err(RunError::of_plan)?;
            let (paths, entries) = port.made();
            for path in paths {
                self.book.learn_path(path);
            }
            for entry in entries {
                self.book.learn_entry(entry);
            }
            let fixed = self.module.fixed(Stage::Plan)?;
            let settings = self.settings.plan_settings();
            let declared = (settings, self.log.logged_from(self.first));
            let snapshot = PlanSnapshot::of(&port, fixed, &self.book, declared, &self.store);
            self.plan_snapshot = Some(snapshot);
            self.plan = Some(port);
        }
        held(self.plan.as_ref(), Stage::Plan, Stage::Decide)
    }

    /// The stage `build`: the package, of the plan in a porter's run and of the fixed mod in
    /// place in a repairer's, with the evidence of the run in it (#ADR-0041). A package that
    /// would hold nothing ends the run.
    pub fn build(&mut self) -> Result<&Package, RunError> {
        self.going()?;
        if self.build_snapshot.is_none() {
            let built = {
                let fixed = self.module.fixed(Stage::Build)?;
                let mut sink = self.log.at(Stage::Build);
                match self.mode {
                    Mode::Port => {
                        let port = held(self.plan.as_ref(), Stage::Build, Stage::Plan)?;
                        let speller = Speller::new(&self.book, self.env.names());
                        PortBuilder::new(fixed, port, self.env, &speller)
                            .keep_dds(self.settings.keep_dds)
                            .build(&mut sink)
                    }
                    Mode::InPlace => {
                        let changes = held(self.changes.as_ref(), Stage::Build, Stage::Fix)?;
                        let options = self.settings.load_options();
                        InPlaceBuilder::new(fixed, changes)
                            .names(&self.book, self.env.names())
                            .judged(self.env, &options)
                            .keep_dds(self.settings.keep_dds)
                            .build(&mut sink)
                    }
                }
            };
            let mut package = built.map_err(RunError::of_build)?;
            package.evidence = self.evidence()?;
            if let Some(image) = self.module.module().and_then(|m| m.image.as_ref()) {
                package.carry_image(image, &mut self.log.at(Stage::Build));
            }
            let part = PackageRow::of(&package);
            let declared = (
                self.settings.build_settings(),
                self.log.logged_from(self.first),
            );
            let snapshot = BuildSnapshot::of(&package, part, declared, &self.store);
            self.build_snapshot = Some(snapshot);
            self.package = Some(package);
        }
        held(self.package.as_ref(), Stage::Build, Stage::Fix)
    }

    /// The evidence the run's package embeds: what the run holds once the mod is fixed and,
    /// in a porter's run, planned.
    fn evidence(&self) -> Result<Option<EvidenceDoc>, RunError> {
        // A dating an earlier run established names its piece by an id of that run's mod,
        // which this run's mod does not spell.
        if self.known.is_some() {
            return Ok(None);
        }
        let fixed = self.module.fixed(Stage::Build)?;
        let dating = held(self.dating.as_ref(), Stage::Build, Stage::Load)?;
        let doc = EvidenceDoc::of(self.mode.made(), fixed, dating, self.env)
            .fixups(self.settings.fixups.ids());
        Ok(Some(match (self.targets(), &self.plan) {
            (Some(targets), Some(port)) => doc.ported(targets, port, fixed),
            _ => doc,
        }))
    }

    /// The stage `check`: the chosen checks on the package. A porter's run applies the
    /// package's patches to the game's bins first, and holds what each came to. What a check
    /// finds is a diagnostic and no error (#ADR-0003).
    pub fn check(&mut self) -> Result<(), RunError> {
        self.going()?;
        if self.check_snapshot.is_some() {
            return Ok(());
        }
        let fixed = self.module.fixed(Stage::Check)?;
        let package = held(self.package.as_ref(), Stage::Check, Stage::Build)?;
        let checks = &self.settings.checks;
        let mut sink = self.log.at(Stage::Check);
        let overlay = match self.mode {
            Mode::Port => {
                let port = held(self.plan.as_ref(), Stage::Check, Stage::Plan)?;
                let game = self.env.game();
                // The patches applied are a step before the checks that read them.
                let mut steps = Steps::new(1 + checks.len());
                steps.begin(&mut sink, || Step::part("patches applied"))?;
                let overlay = Overlay::apply(package, game).map_err(RunError::Check)?;
                let types = TargetTypes::of(fixed, self.env).map_err(RunError::Check)?;
                let cx = CheckCx::new(package, &types).of_port(game, &overlay, port);
                checks.run_counted(&cx, &mut steps, &mut sink)?;
                Some(overlay)
            }
            Mode::InPlace => {
                let types = TargetTypes::of(fixed, self.env).map_err(RunError::Check)?;
                let cx = CheckCx::new(package, &types);
                checks.run(&cx, &mut sink)?;
                None
            }
        };
        let patched = match &overlay {
            Some(overlay) => overlay.in_order(package),
            None => vec![None; package.all_patches().count()],
        };
        let applied: Vec<Option<AppliedRow>> = patched
            .iter()
            .map(|patched| patched.map(|patched| AppliedRow::of(&patched.outcome)))
            .collect();
        let declared = (
            self.settings.check_settings(),
            self.log.logged_from(self.first),
        );
        let snapshot = CheckSnapshot::of(applied, &patched, declared, &self.store);
        self.check_snapshot = Some(snapshot);
        Ok(())
    }

    /// Folds the stages up to `last` over the run's snapshots, in order: each stage the run
    /// holds no snapshot of runs from the one before it and ends by writing its own. A fatal
    /// diagnostic or a cancel ends the run early and is no error. `read` and `write` are the
    /// caller's to call.
    pub fn through(&mut self, last: Stage) -> Result<(), RunError> {
        for &stage in self.mode.stages() {
            if stage > last || self.ran(stage) {
                continue;
            }
            match self.step(stage) {
                Ok(()) => {}
                Err(RunError::Halted(_) | RunError::Cancelled(_)) => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// One stage of [`Run::through`].
    fn step(&mut self, stage: Stage) -> Result<(), RunError> {
        match stage {
            Stage::Load => self.load().map(drop),
            Stage::Fix => self.fix().map(drop),
            Stage::Facts => self.gather().map(drop),
            Stage::Decide => self.decide().map(drop),
            Stage::Plan => self.plan().map(drop),
            Stage::Build => self.build().map(drop),
            Stage::Check => self.check(),
            Stage::Read | Stage::Write => Ok(()),
        }
    }

    /// Reads an archive's bytes and runs every stage but `write`.
    fn all(&mut self, bytes: &[u8], file_name: &str) -> Result<(), RunError> {
        match self.read(bytes, file_name) {
            Ok(_) | Err(RunError::Halted(_)) => {}
            Err(error) => return Err(error),
        }
        self.through(Stage::Check)
    }

    /// Takes the package of a repair through `read`, `load`, `fix` and `build` a second time,
    /// in memory, and holds what the second run changed: nothing, for a repair that is whole.
    /// The second run is judged with the evidence of this one, and reads no cancel. A
    /// cancelled run is not repaired again.
    pub fn again(&mut self) -> Result<&AgainRow, RunError> {
        if self.mode == Mode::Port {
            return Err(RunError::NotARepair);
        }
        if let Some(cancelled) = self.log.cancelled() {
            return Err(cancelled.into());
        }
        if self.again.is_none() {
            // The second run's `read` reads the package this run built.
            let fixed = self.module.fixed(Stage::Read)?;
            let first = held(self.package.as_ref(), Stage::Read, Stage::Build)?;
            let dating = held(self.dating.as_ref(), Stage::Read, Stage::Load)?;
            let restored = fixed.restorations.iter();
            let evidence = Evidence {
                dating: dating.clone(),
                restored: restored
                    .map(|&wad| fixed.wads[wad].name.lower().to_string())
                    .collect(),
            };
            let bytes = archive_of(first).map_err(RunError::Again)?;
            let mut second = Run::new(self.env, self.settings, None).known(evidence);
            let loaded = second.read(&bytes, REPAIRED_NAME).map(drop);
            let loaded = loaded.and_then(|()| second.load().map(drop));
            let row = match loaded {
                Err(RunError::Halted(_)) => AgainRow::of(first, None, None),
                Err(fault) => return Err(fault),
                Ok(()) => {
                    second.fix()?;
                    match second.build() {
                        Ok(_) | Err(RunError::Halted(_)) => {}
                        Err(fault) => return Err(fault),
                    }
                    let row = AgainRow::of(first, second.package(), second.changes());
                    match second.redated() {
                        Some(own) => row.redated(own),
                        None => row,
                    }
                }
            };
            self.again = Some(row);
        }
        held(self.again.as_ref(), Stage::Read, Stage::Build)
    }

    /// The stage `write`: the package through each writer, with the report of the run as it
    /// stands before the write beside it. The caller's policy is asked first, and a refusal
    /// raises `write.blocked` and writes nothing (#ADR-0003). A cancelled run writes nothing.
    pub fn write(
        &mut self,
        writers: &[&dyn PackageWriter],
        policy: &DiagnosticPolicy,
    ) -> Result<(), RunError> {
        // A package a cancelled `check` holds was never checked.
        if let Some(cancelled) = self.log.cancelled() {
            return Err(cancelled.into());
        }
        if self.package.is_none() {
            return Err(RunError::NotReached {
                stage: Stage::Write,
                needs: Stage::Build,
            });
        }
        permit(policy, &mut self.log)?;
        let report = self.report().to_json() + "\n";
        let package = held(self.package.as_ref(), Stage::Write, Stage::Build)?;
        for writer in writers {
            let mut sink = self.log.at(Stage::Write);
            let written = package.write(*writer, &report, &mut sink);
            written.map_err(|source| RunError::Write {
                kind: writer.kind(),
                source,
            })?;
        }
        self.written = true;
        Ok(())
    }

    /// The report of the run as far as it went: a part for each result the run holds, and
    /// every diagnostic, written with the names the run has learnt.
    pub fn report(&self) -> Report {
        let speller = self.speller();
        let module = self.module();
        let mut report = Report::builder().env(self.env);
        if let Some(m) = module {
            report = report.module(m);
        }
        if let Some(m) = module.filter(|_| !matches!(self.module, Reached::Partial(_))) {
            if let Some(dating) = &self.dating {
                report = report.dating(dating, m);
            }
            if let Some(inventory) = self.inventory() {
                report = report.inventory(inventory);
            }
            if let Some(provenance) = &self.provenance {
                report = report.provenance(provenance);
            }
            if let Some(changes) = &self.changes {
                report = report.changes(changes, m, &speller);
            }
        }
        if let Some(facts) = &self.facts {
            report = report.facts(facts, self.env, &speller);
        }
        if let Some(targets) = self.targets() {
            report = report.targets(targets);
        }
        if let Some(port) = &self.plan {
            report = report.port(port);
        }
        if let Some(build) = &self.build_snapshot {
            let mut part = build.part().clone();
            if let Some(check) = &self.check_snapshot {
                part.apply(check.applied());
            }
            report = report.package_row(part, build.evidence());
        }
        if let Some(again) = &self.again {
            report = report.again(again.clone());
        }
        let cx = Render {
            names: &speller,
            module,
        };
        report.diagnostics(&self.log, &cx).build()
    }

    /// Every diagnostic raised, in order.
    pub fn log(&self) -> &Diagnostics {
        &self.log
    }

    /// The diagnostic that ended the run, when one did.
    pub fn halted(&self) -> Option<Halted> {
        self.log.halted()
    }

    /// The stage a fatal diagnostic ended the run in, when one did.
    pub fn halted_at(&self) -> Option<Stage> {
        let halted = self.log.halted()?;
        self.log.get(halted.0).map(|diagnostic| diagnostic.stage)
    }

    /// The stage the run's cancel ended, when it ended one (#ADR-0097).
    pub fn cancelled_at(&self) -> Option<Stage> {
        self.log.cancelled().map(|cancelled| cancelled.0)
    }

    /// The stages of the run, in order, `read` and `write` among them. A repairer's run has
    /// no `facts`, `decide` or `plan` (#ADR-0011).
    pub fn stages(&self) -> &'static [Stage] {
        self.mode.stages()
    }

    /// Whether a stage ran to its end: whether the run holds its snapshot, or for `write`
    /// whether the package was written. A stage a fatal diagnostic, a cancel or a fault ended
    /// did not, and neither did a write the caller's policy refused.
    pub fn ran(&self, stage: Stage) -> bool {
        match stage {
            Stage::Read => self.raw.is_some() || self.loaded.is_some(),
            Stage::Load => self.loaded.is_some(),
            Stage::Fix => self.fix_snapshot.is_some(),
            Stage::Facts => self.facts_snapshot.is_some(),
            Stage::Decide => self.decide_snapshot.is_some(),
            Stage::Plan => self.plan_snapshot.is_some(),
            Stage::Build => self.build_snapshot.is_some(),
            Stage::Check => self.check_snapshot.is_some(),
            Stage::Write => self.written,
        }
    }

    /// The names the run has learnt.
    pub fn names(&self) -> &NameBook {
        &self.book
    }

    /// Spells hashes from the names learnt and the tables.
    pub fn speller(&self) -> Speller<'_> {
        Speller::new(&self.book, self.env.names())
    }

    /// The environment the run is judged against.
    pub fn env(&self) -> &'a Env {
        self.env
    }

    /// The raw snapshot of the archive read (#ADR-0094). None for a run started from its load
    /// snapshot.
    pub fn archive(&self) -> Option<&RawSnapshot> {
        self.raw.as_ref()
    }

    /// A snapshot the run gives a caller: none in a run judged with an earlier run's
    /// evidence, which is no input a snapshot names, so a resume from one would not be judged
    /// as it was.
    fn given<'s, T>(&self, snapshot: &'s Option<T>) -> Option<&'s T> {
        snapshot.as_ref().filter(|_| self.known.is_none())
    }

    /// The load snapshot, once `load` ended or the run started from one (#ADR-0094).
    pub fn load_snapshot(&self) -> Option<&LoadSnapshot> {
        self.given(&self.loaded)
    }

    /// The fix snapshot, once `fix` ended or the run started from one (#ADR-0094).
    pub fn fix_snapshot(&self) -> Option<&FixSnapshot> {
        self.given(&self.fix_snapshot)
    }

    /// The run's snapshots from `load` on, as far as it went: what a caller keeps to resume
    /// after any of its stages. None before `load` ends, and in a run judged with an earlier
    /// run's evidence.
    pub fn snapshots(&self) -> Option<Snapshots> {
        let mut snapshots = Snapshots::new(self.load_snapshot()?.clone());
        snapshots.fix = self.fix_snapshot.clone();
        snapshots.facts = self.facts_snapshot.clone();
        snapshots.decide = self.decide_snapshot.clone();
        snapshots.plan = self.plan_snapshot.clone();
        snapshots.build = self.build_snapshot.clone();
        snapshots.check = self.check_snapshot.clone();
        Some(snapshots)
    }

    /// The check snapshot, once `check` ended or the run started from one (#ADR-0094).
    pub fn check_snapshot(&self) -> Option<&CheckSnapshot> {
        self.given(&self.check_snapshot)
    }

    /// The build snapshot, once `build` ended or the run started from one (#ADR-0094).
    pub fn build_snapshot(&self) -> Option<&BuildSnapshot> {
        self.given(&self.build_snapshot)
    }

    /// The plan snapshot, once `plan` ended or the run started from one (#ADR-0094).
    pub fn plan_snapshot(&self) -> Option<&PlanSnapshot> {
        self.given(&self.plan_snapshot)
    }

    /// The decide snapshot, once `decide` ended or the run started from one (#ADR-0094).
    pub fn decide_snapshot(&self) -> Option<&DecideSnapshot> {
        self.given(&self.decide_snapshot)
    }

    /// The facts snapshot, once `facts` ended or the run started from one (#ADR-0094).
    pub fn facts_snapshot(&self) -> Option<&FactsSnapshot> {
        self.given(&self.facts_snapshot)
    }

    /// The store the run's bytes are in: its memory layer over the caller's store.
    pub fn store(&self) -> &dyn BlobStore {
        &self.store
    }

    /// The blobs of the run's memory layer, every byte its stages made, for the caller to
    /// flush into a store of its own (#ADR-0093).
    pub fn into_layer(self) -> BTreeMap<ContentId, ByteView> {
        let (layer, _) = self.store.into_parts();
        layer.into_blobs()
    }

    /// The mod as far as the run took it: fixed, else as `load` left it, else as far as a
    /// `load` that a fatal diagnostic ended went.
    pub fn module(&self) -> Option<&Mod> {
        self.module.module()
    }

    /// The mod as the fixups left it.
    pub fn fixed(&self) -> Option<&FixedMod> {
        match &self.module {
            Reached::Fixed(fixed) => Some(fixed),
            _ => None,
        }
    }

    /// The dating the mod is judged at.
    pub fn dating(&self) -> Option<&Dating> {
        self.dating.as_ref()
    }

    /// The dating the archive shows by itself, in a run judged with an earlier run's.
    pub fn redated(&self) -> Option<&Dating> {
        self.redated.as_ref()
    }

    /// What the archive carries.
    pub fn inventory(&self) -> Option<&Inventory> {
        self.loaded.as_ref()?.part().inventory.as_ref()
    }

    /// Whose bytes the mod's files are.
    pub fn provenance(&self) -> Option<&FileProvenance> {
        self.provenance.as_ref()
    }

    /// What the fixups changed.
    pub fn changes(&self) -> Option<&Changes> {
        self.changes.as_ref()
    }

    /// What is true of the mod.
    pub fn facts(&self) -> Option<&Facts> {
        self.facts.as_ref()
    }

    /// Where each unit goes, as the decide snapshot holds it.
    pub fn targets(&self) -> Option<&Targets> {
        self.decide_snapshot.as_ref().map(DecideSnapshot::targets)
    }

    /// The plan of the port.
    pub fn port(&self) -> Option<&Port> {
        self.plan.as_ref()
    }

    /// The package.
    pub fn package(&self) -> Option<&Package> {
        self.package.as_ref()
    }

    /// What applying each patch of the package to the game's bins came to, in the package's
    /// order, once the run is checked: none for a patch a repair's run does not apply.
    pub fn applied(&self) -> Option<&[Option<AppliedRow>]> {
        self.check_snapshot.as_ref().map(CheckSnapshot::applied)
    }

    /// What repairing the package a second time changed, once asked.
    pub fn repaired_again(&self) -> Option<&AgainRow> {
        self.again.as_ref()
    }
}

/// A package's chunks as the bytes of a mod archive, each stored at `WAD/<wad>/<path>`. In
/// memory: nothing of the mod is written for the second run (#ADR-0024).
fn archive_of(package: &Package) -> Result<Vec<u8>, ZipError> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (_, key, chunk) in package.chunks() {
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(chunk.bytes.len() >= u32::MAX as usize);
        zip.start_file(format!("WAD/{}/{}", key.wad, key.path), options)?;
        zip.write_all(&chunk.bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}
