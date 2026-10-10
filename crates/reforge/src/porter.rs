//! The stages driven in order. A [`Porter`] or a [`Repairer`] holds what a caller chose, and a
//! [`Run`] takes one archive through the stages with it: one method per stage, each handing
//! its stage the stage's sink and options.
//!
//! The run owns what passes between the stages: the log, the names learnt, the mod, the
//! evidence of its files and every stage's result. A caller stops after any stage and reads
//! what the run holds. The report is written from the run as far as it went.
//!
//! `docs/design/ARCHITECTURE.md`, "Shape".

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Write as _};

use camino::Utf8Path;
use zip::CompressionMethod;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;

use crate::archive::{self, Caps, RawSnapshot};
use crate::diag::{Code, DiagnosticPolicy, Diagnostics, Halted, Render, Stage};
use crate::env::Env;
use crate::facts::{FactInputs, Facts, FactsError};
use crate::fixups::{Changes, FixError, FixInputs, FixedMod, FixupSet, ObjectSet, TargetTypes};
use crate::game::{GameError, NameBook, Speller};
use crate::load::{
    Dating, Fault, FileProvenance, Inventory, LoadError, LoadOptions, Loader, ReworkTable, against,
};
use crate::model::{ByteView, ContentId, Mod};
use crate::package::{
    BuildError, CheckCx, CheckSet, EvidenceDoc, InPlaceBuilder, Made, Overlay, Package,
    PackageWriter, PortBuilder, Refused, WriteError, WriterKind, permit,
};
use crate::port::{PlanError, Policy, Port, PortInputs, PortOptions, Targets};
use crate::report::{AgainRow, AppliedRow, PackageRow, Report};
use crate::snapshot::{
    self, BuildSettings, BuildSnapshot, CheckSettings, CheckSnapshot, DecideSettings,
    DecideSnapshot, FactsSettings, FactsSnapshot, FixSettings, FixSnapshot, LoadSettings,
    LoadSnapshot, LoadState, PlanSettings, PlanSnapshot, ResumeError, Setting, Snapshots,
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
    /// What the build's own error is to a run: its fatal diagnostic, else the fault.
    fn of_build(error: BuildError) -> Self {
        match error {
            BuildError::Halted(halted) => RunError::Halted(halted),
            BuildError::Fault(fault) => RunError::Build(fault),
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

/// What a caller chose for the stages of a run. The default of each set and table is its
/// standard one.
#[derive(Debug, Clone, Default)]
struct Settings {
    fixups: FixupSet,
    checks: CheckSet,
    caps: Caps,
    keep_stale: bool,
    reworks: ReworkTable,
    policy: Policy,
    compared: Vec<Policy>,
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
            .reworks(self.reworks.clone())
            .file_fixups(self.fixups.file_fixups())
    }

    fn port_options(&self) -> PortOptions {
        PortOptions {
            keep_stale: self.keep_stale,
            game_events_only: self.game_events_only,
            keep_other_slots: self.keep_other_slots,
        }
    }

    /// The settings `build` reads.
    fn build_settings(&self) -> BuildSettings {
        BuildSettings {
            keep_dds: self.keep_dds,
            keep_stale: self.keep_stale,
        }
    }
}

/// What a run builds its package from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The plan of a port.
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
}

impl<'a> Porter<'a> {
    /// A porter with the standard fixups and checks, the default policy and caps, the reworks
    /// of #GAME-CHANGE-0001, and stale copies left out.
    pub fn new(env: &'a Env) -> Self {
        Self {
            env,
            settings: Settings::default(),
        }
    }

    /// Chooses each unit's target under this policy.
    #[must_use]
    pub fn policy(mut self, policy: Policy) -> Self {
        self.settings.policy = policy;
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

    /// A run of one archive, with nothing read.
    pub fn run(&self) -> Run<'_> {
        Run::new(self.env, &self.settings, Mode::Port)
    }

    /// Takes an archive's bytes through every stage but `write`. A fatal diagnostic ends the
    /// run early and is no error: the run comes back as far as it went.
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
}

impl<'a> Repairer<'a> {
    /// A repairer with the standard fixups and checks, the default caps, the reworks of
    /// #GAME-CHANGE-0001, and stale copies left out.
    pub fn new(env: &'a Env) -> Self {
        Self {
            env,
            settings: Settings::default(),
        }
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

    /// A run of one archive, with nothing read.
    pub fn run(&self) -> Run<'_> {
        Run::new(self.env, &self.settings, Mode::InPlace)
    }

    /// Takes an archive's bytes through every stage but `write`. A fatal diagnostic ends the
    /// run early and is no error: the run comes back as far as it went.
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
    /// The run's memory layer over the caller's store (#ADR-0093): every byte a stage makes
    /// goes into the layer, and the caller flushes it.
    store: Layered<MemoryStore, Box<dyn BlobStore>>,
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
    inventory: Option<Inventory>,
    provenance: Option<FileProvenance>,
    changes: Option<Changes>,
    facts: Option<Facts>,
    targets: Option<Targets>,
    plan: Option<Port>,
    package: Option<Package>,
    /// The package's part of the report, as the build snapshot holds it.
    package_part: Option<PackageRow>,
    /// What applying each patch of the package came to, in its order, once `check` ended.
    applied: Option<Vec<Option<AppliedRow>>>,
    written: bool,
    again: Option<AgainRow>,
}

impl<'a> Run<'a> {
    fn new(env: &'a Env, settings: &'a Settings, mode: Mode) -> Self {
        Self {
            env,
            settings,
            mode,
            known: None,
            log: Diagnostics::new(),
            book: NameBook::new(),
            store: Layered::new(MemoryStore::new(), Box::new(MemoryStore::new())),
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
            inventory: None,
            provenance: None,
            changes: None,
            facts: None,
            targets: None,
            plan: None,
            package: None,
            package_part: None,
            applied: None,
            written: false,
            again: None,
        }
    }

    /// Starts from a log that holds what the inputs raised when the environment was built.
    #[must_use]
    pub fn log_from(mut self, log: Diagnostics) -> Self {
        self.log = log;
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
        let (layer, _) = self.store.into_parts();
        self.store = Layered::new(layer, Box::new(store));
        self
    }

    /// Ends a stage's method where a fatal diagnostic ended the run.
    fn going(&self) -> Result<(), RunError> {
        self.log
            .halted()
            .map_or(Ok(()), |halted| Err(halted.into()))
    }

    /// The stage `read`: the archive from its bytes, which are borrowed for the call, read
    /// into the run's store. The run holds the raw snapshot (#ADR-0094). `file_name` is for
    /// display and is never opened.
    pub fn read(&mut self, bytes: &[u8], file_name: &str) -> Result<&RawSnapshot, RunError> {
        self.going()?;
        self.first = self.log.len();
        let mut sink = self.log.at(Stage::Read);
        let caps = &self.settings.caps;
        let raw = archive::read(bytes, file_name, caps, &self.store, &mut sink)?;
        Ok(self.raw.insert(raw))
    }

    /// The stage `read`, of an archive file.
    pub fn read_file(&mut self, path: &Utf8Path) -> Result<&RawSnapshot, RunError> {
        self.going()?;
        self.first = self.log.len();
        let mut sink = self.log.at(Stage::Read);
        let caps = &self.settings.caps;
        let raw = archive::read_file(path, caps, &self.store, &mut sink)?;
        Ok(self.raw.insert(raw))
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
        let started = self.raw.is_some() || self.loaded.is_some() || self.log.halted().is_some();
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
            facts.holds(&self.facts_settings())?;
            let found = facts.found().clone();
            let reopened = Facts::reopen(&fixed, self.env, found);
            self.facts = Some(reopened.map_err(ResumeError::Facts)?);
            findings = facts.findings();
        }
        if let Some(decide) = &decide {
            decide.holds(&self.decide_settings())?;
            self.targets = Some(decide.targets().clone());
            findings = decide.findings();
        }
        if let Some(plan) = &plan {
            plan.holds(&PlanSettings::of(&self.settings.port_options()))?;
            self.plan = Some(plan.port(&self.store)?);
            self.book = plan.names().clone();
            findings = plan.findings();
        }
        if let Some(build) = &build {
            build.holds(&self.settings.build_settings())?;
            self.package = Some(build.package(&self.store)?);
            self.package_part = Some(build.part().clone());
            findings = build.findings();
            if let Some(check) = &check {
                check.holds(&CheckSettings::of(&self.settings.checks))?;
                let applied = check.applied();
                let patches = build.part().patches.len();
                if applied.len() != patches {
                    let what = format!("{} outcomes of {patches} patches", applied.len());
                    return Err(ResumeError::Malformed(what));
                }
                self.applied = Some(applied);
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

    /// The settings `decide` reads in this run.
    fn decide_settings(&self) -> DecideSettings {
        DecideSettings {
            policy: self.settings.policy,
            compared: self.settings.compared.clone(),
        }
    }

    /// The settings `facts` reads in this run.
    fn facts_settings(&self) -> FactsSettings {
        FactsSettings {
            reworks: self.settings.reworks.clone(),
        }
    }

    /// The mod as the load snapshot left it, and what the run holds once `load` ends: the
    /// names, the dating, the inventory and the file provenance.
    fn reopen_load(&mut self, load: &LoadSnapshot) -> Result<Mod, ResumeError> {
        let inputs = snapshot::LoadInputs::of(self.env);
        load.holds(&inputs, &self.load_settings())?;
        let reopened = load.reopen(&self.store)?;
        self.book = reopened.names;
        self.dating = Some(reopened.dating);
        self.inventory = reopened.inventory;
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
        fix.holds(&snapshot::FixInputs::of(self.env), &self.fix_settings())?;
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

    /// The settings `fix` reads in this run.
    fn fix_settings(&self) -> FixSettings {
        FixSettings::of(&self.settings.fixups)
    }

    /// The settings `load` reads in this run.
    fn load_settings(&self) -> LoadSettings {
        LoadSettings::of(self.mode.snapshot(), &self.settings.load_options())
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
        let mut module = match loader.open(raw, &self.store) {
            Ok(module) => module,
            Err(error) => {
                self.book = loader.into_names();
                return Err(RunError::Load(error));
            }
        };
        let known = self.known.as_ref();
        let mut steps = || -> Result<(), LoadError> {
            loader.fix_files(&mut module, &mut sink)?;
            loader.take_game_bins(&mut module, &mut sink)?;
            if let Some(known) = known {
                let wads = module.wads.iter();
                let restored: Vec<_> = wads
                    .filter(|(_, wad)| known.restored.contains(wad.name.lower()))
                    .map(|(id, _)| id)
                    .collect();
                module.restorations.extend(restored);
            }
            loader.survey(&mut module, &mut sink)?;
            loader.resolve(&mut module, &mut sink)?;
            Ok(())
        };
        let done = steps();
        self.book = loader.into_names();
        match done {
            Ok(()) => {}
            Err(LoadError::Halted(halted)) => {
                self.module = Reached::Partial(module);
                return Err(halted.into());
            }
            Err(fault) => return Err(RunError::Load(fault)),
        }

        let found = Dating::find(&module, env.history());
        let found = found.map_err(|error| RunError::Load(LoadError::History(error)))?;
        if self.mode == Mode::Port {
            let speller = Speller::new(&self.book, env.names());
            let inventory = Inventory::take(&module, &speller);
            inventory.report(&mut sink);
            let provenance = FileProvenance::find(&module, &against(env, &speller));
            let provenance = provenance.map_err(|fault| RunError::Load(fault.into()))?;
            provenance.report(&mut sink);
            self.inventory = Some(inventory);
            self.provenance = Some(provenance);
        }
        match &self.known {
            Some(known) => {
                self.dating = Some(known.dating.clone());
                self.redated = Some(found);
            }
            None => {
                self.loaded = Some(self.snapshot_of(&module, &found));
                self.dating = Some(found);
            }
        }
        self.module = Reached::Loaded(module);
        Ok(())
    }

    /// The load snapshot of the mod `load` left. A run judged with an earlier run's evidence
    /// keeps none: the evidence is no input a snapshot names.
    fn snapshot_of(&self, module: &Mod, dating: &Dating) -> LoadSnapshot {
        let raw = self.raw.as_ref();
        let state = LoadState {
            module,
            dating,
            inventory: self.inventory.as_ref(),
            provenance: self.provenance.as_ref(),
            names: &self.book,
            extras: raw.map_or(&[], |raw| raw.extras.as_slice()),
            image: raw.and_then(|raw| raw.image),
        };
        let findings = self.log.logged_from(self.first);
        LoadSnapshot::of(
            &state,
            self.env,
            self.load_settings(),
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
                        self.fix_snapshot = self.fix_snapshot_of(&fixed, &changes);
                        self.module = Reached::Fixed(fixed);
                        self.changes = Some(changes);
                    }
                    // The report of a run ended here still holds what `load` gave.
                    Err(unfixed) => {
                        self.module = Reached::Loaded(unfixed.module);
                        return Err(RunError::Fix(unfixed.fault));
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
    /// store. A run with no load snapshot keeps none: a repair's second run, judged with the
    /// first's evidence, keeps no snapshot after `load` as it keeps none of `load`.
    fn fix_snapshot_of(&self, fixed: &FixedMod, changes: &Changes) -> Option<FixSnapshot> {
        let load = self.loaded.as_ref()?;
        let rewritten = fixed.rewritten(changes).into_iter();
        let rewritten = rewritten.map(|(file, bin)| (file, self.store.put(bin.bytes.into())));
        let declared = (snapshot::FixInputs::of(self.env), self.fix_settings());
        let findings = self.log.logged_from(self.first);
        Some(FixSnapshot::of(
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
        if self.facts.is_none() {
            let fixed = self.module.fixed(Stage::Facts)?;
            let speller = Speller::new(&self.book, self.env.names());
            let inputs = FactInputs {
                env: self.env,
                dating: held(self.dating.as_ref(), Stage::Facts, Stage::Load)?,
                files: held(self.provenance.as_ref(), Stage::Facts, Stage::Load)?,
                names: &speller,
                reworks: &self.settings.reworks,
            };
            let facts = Facts::gather(fixed, &inputs).map_err(RunError::Facts)?;
            facts.report(&mut self.log.at(Stage::Facts));
            // A run keeps a snapshot after a stage only after the snapshots before it.
            if self.fix_snapshot.is_some() {
                let findings = self.log.logged_from(self.first);
                let snapshot = FactsSnapshot::of(&facts, self.facts_settings(), findings);
                self.facts_snapshot = Some(snapshot);
            }
            self.facts = Some(facts);
        }
        held(self.facts.as_ref(), Stage::Facts, Stage::Fix)
    }

    /// The stage `decide`: the policy's choice of a target for each unit. Only the porter's
    /// policy raises diagnostics. What the compared policies choose is recorded beside it.
    pub fn decide(&mut self) -> Result<&Targets, RunError> {
        self.going()?;
        self.of_a_port(Stage::Decide)?;
        if self.targets.is_none() {
            let facts = held(self.facts.as_ref(), Stage::Decide, Stage::Facts)?;
            let targets = self.settings.policy.decide(facts);
            targets.report(&mut self.log.at(Stage::Decide));
            let targets = targets.compared(facts, &self.settings.compared);
            if self.facts_snapshot.is_some() {
                let findings = self.log.logged_from(self.first);
                let snapshot = DecideSnapshot::of(&targets, self.decide_settings(), findings);
                self.decide_snapshot = Some(snapshot);
            }
            self.targets = Some(targets);
        }
        held(self.targets.as_ref(), Stage::Decide, Stage::Facts)
    }

    /// The stage `plan`: the port of the fixed mod at its targets. The run learns the names
    /// the plan makes, so a message spells them.
    pub fn plan(&mut self) -> Result<&Port, RunError> {
        self.going()?;
        self.of_a_port(Stage::Plan)?;
        if self.plan.is_none() {
            let options = self.settings.port_options();
            let speller = Speller::new(&self.book, self.env.names());
            let inputs = PortInputs {
                fixed: self.module.fixed(Stage::Plan)?,
                facts: held(self.facts.as_ref(), Stage::Plan, Stage::Facts)?,
                targets: held(self.targets.as_ref(), Stage::Plan, Stage::Decide)?,
                env: self.env,
                files: held(self.provenance.as_ref(), Stage::Plan, Stage::Load)?,
                names: &speller,
                options: &options,
            };
            let port = Port::plan(&inputs, &mut self.log.at(Stage::Plan));
            let port = port.map_err(RunError::Plan)?;
            let (paths, entries) = port.made();
            for path in paths {
                self.book.learn_path(path);
            }
            for entry in entries {
                self.book.learn_entry(entry);
            }
            if self.decide_snapshot.is_some() {
                let fixed = self.module.fixed(Stage::Plan)?;
                let settings = PlanSettings::of(&self.settings.port_options());
                let declared = (settings, self.log.logged_from(self.first));
                let snapshot = PlanSnapshot::of(&port, fixed, &self.book, declared, &self.store);
                self.plan_snapshot = Some(snapshot);
            }
            self.plan = Some(port);
        }
        held(self.plan.as_ref(), Stage::Plan, Stage::Decide)
    }

    /// The stage `build`: the package, of the plan in a porter's run and of the fixed mod in
    /// place in a repairer's, with the evidence of the run in it (#ADR-0041). A package that
    /// would hold nothing ends the run.
    pub fn build(&mut self) -> Result<&Package, RunError> {
        self.going()?;
        if self.package.is_none() {
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
            let before = match self.mode {
                Mode::Port => self.plan_snapshot.is_some(),
                Mode::InPlace => self.fix_snapshot.is_some(),
            };
            if before {
                let part = PackageRow::of(&package);
                let declared = (
                    self.settings.build_settings(),
                    self.log.logged_from(self.first),
                );
                let snapshot = BuildSnapshot::of(&package, &part, declared, &self.store);
                self.build_snapshot = Some(snapshot);
                self.package_part = Some(part);
            }
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
        Ok(Some(match (&self.targets, &self.plan) {
            (Some(targets), Some(port)) => doc.ported(targets, port, fixed),
            _ => doc,
        }))
    }

    /// The stage `check`: the chosen checks on the package. A porter's run applies the
    /// package's patches to the game's bins first, and holds what each came to. What a check
    /// finds is a diagnostic and no error (#ADR-0003).
    pub fn check(&mut self) -> Result<(), RunError> {
        self.going()?;
        if self.applied.is_some() {
            return Ok(());
        }
        let fixed = self.module.fixed(Stage::Check)?;
        let package = held(self.package.as_ref(), Stage::Check, Stage::Build)?;
        let checks = &self.settings.checks;
        let overlay = match self.mode {
            Mode::Port => {
                let port = held(self.plan.as_ref(), Stage::Check, Stage::Plan)?;
                let game = self.env.game();
                let overlay = Overlay::apply(package, game).map_err(RunError::Check)?;
                let types = TargetTypes::of(fixed, self.env).map_err(RunError::Check)?;
                let cx = CheckCx::new(package, &types).of_port(game, &overlay, port);
                checks.run(&cx, &mut self.log.at(Stage::Check));
                Some(overlay)
            }
            Mode::InPlace => {
                let types = TargetTypes::of(fixed, self.env).map_err(RunError::Check)?;
                let cx = CheckCx::new(package, &types);
                checks.run(&cx, &mut self.log.at(Stage::Check));
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
        if self.build_snapshot.is_some() {
            let declared = (CheckSettings::of(checks), self.log.logged_from(self.first));
            let snapshot = CheckSnapshot::of(&applied, &patched, declared, &self.store);
            self.check_snapshot = Some(snapshot);
        }
        self.applied = Some(applied);
        Ok(())
    }

    /// Runs every stage the run has not run, in order, up to `last`. A fatal diagnostic ends
    /// the run early and is no error. `read` and `write` are the caller's to call.
    pub fn through(&mut self, last: Stage) -> Result<(), RunError> {
        for &stage in self.mode.stages().iter().filter(|&&stage| stage <= last) {
            let ran = match stage {
                Stage::Load => self.load().map(drop),
                Stage::Fix => self.fix().map(drop),
                Stage::Facts => self.gather().map(drop),
                Stage::Decide => self.decide().map(drop),
                Stage::Plan => self.plan().map(drop),
                Stage::Build => self.build().map(drop),
                Stage::Check => self.check(),
                Stage::Read | Stage::Write => Ok(()),
            };
            match ran {
                Ok(()) => {}
                Err(RunError::Halted(_)) => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        Ok(())
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
    /// The second run is judged with the evidence of this one.
    pub fn again(&mut self) -> Result<&AgainRow, RunError> {
        if self.mode == Mode::Port {
            return Err(RunError::NotARepair);
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
            let mut second = Run::new(self.env, self.settings, Mode::InPlace).known(evidence);
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
    /// raises `write.blocked` and writes nothing (#ADR-0003).
    pub fn write(
        &mut self,
        writers: &[&dyn PackageWriter],
        policy: &DiagnosticPolicy,
    ) -> Result<(), RunError> {
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
            if let Some(inventory) = &self.inventory {
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
        if let Some(targets) = &self.targets {
            report = report.targets(targets);
        }
        if let Some(port) = &self.plan {
            report = report.port(port);
        }
        if let Some(package) = &self.package {
            let part = self.package_part.clone();
            let mut part = part.unwrap_or_else(|| PackageRow::of(package));
            if let Some(applied) = &self.applied {
                part.apply(applied);
            }
            report = report.package_row(part, package.evidence.as_ref());
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

    /// The stages of the run, in order, `read` and `write` among them. A repairer's run has
    /// no `facts`, `decide` or `plan` (#ADR-0011).
    pub fn stages(&self) -> &'static [Stage] {
        self.mode.stages()
    }

    /// Whether a stage ran to its end. A stage a fatal diagnostic or a fault ended did not,
    /// and neither did a write the caller's policy refused.
    pub fn ran(&self, stage: Stage) -> bool {
        match stage {
            Stage::Read => self.raw.is_some() || self.loaded.is_some(),
            Stage::Load => matches!(self.module, Reached::Loaded(_) | Reached::Fixed(_)),
            Stage::Fix => matches!(self.module, Reached::Fixed(_)),
            Stage::Facts => self.facts.is_some(),
            Stage::Decide => self.targets.is_some(),
            Stage::Plan => self.plan.is_some(),
            Stage::Build => self.package.is_some(),
            Stage::Check => self.applied.is_some(),
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

    /// The load snapshot, once `load` ended or the run started from one (#ADR-0094).
    pub fn load_snapshot(&self) -> Option<&LoadSnapshot> {
        self.loaded.as_ref()
    }

    /// The fix snapshot, once `fix` ended or the run started from one (#ADR-0094).
    pub fn fix_snapshot(&self) -> Option<&FixSnapshot> {
        self.fix_snapshot.as_ref()
    }

    /// The run's snapshots from `load` on, as far as it went: what a caller keeps to resume
    /// after any of its stages. None before `load` ends, and in a run that keeps no load
    /// snapshot.
    pub fn snapshots(&self) -> Option<Snapshots> {
        let mut snapshots = Snapshots::new(self.loaded.clone()?);
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
        self.check_snapshot.as_ref()
    }

    /// The build snapshot, once `build` ended or the run started from one (#ADR-0094).
    pub fn build_snapshot(&self) -> Option<&BuildSnapshot> {
        self.build_snapshot.as_ref()
    }

    /// The plan snapshot, once `plan` ended or the run started from one (#ADR-0094).
    pub fn plan_snapshot(&self) -> Option<&PlanSnapshot> {
        self.plan_snapshot.as_ref()
    }

    /// The decide snapshot, once `decide` ended or the run started from one (#ADR-0094).
    pub fn decide_snapshot(&self) -> Option<&DecideSnapshot> {
        self.decide_snapshot.as_ref()
    }

    /// The facts snapshot, once `facts` ended or the run started from one (#ADR-0094).
    pub fn facts_snapshot(&self) -> Option<&FactsSnapshot> {
        self.facts_snapshot.as_ref()
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
        self.inventory.as_ref()
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

    /// Where each unit goes.
    pub fn targets(&self) -> Option<&Targets> {
        self.targets.as_ref()
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
        self.applied.as_deref()
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
