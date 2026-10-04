//! The stages driven in order. A [`Porter`] or a [`Repairer`] holds what a caller chose, and a
//! [`Run`] takes one archive through the stages with it: one method per stage, each handing
//! its stage the stage's sink and options.
//!
//! The run owns what passes between the stages: the log, the names learnt, the mod, the
//! evidence of its files and every stage's result. A caller stops after any stage and reads
//! what the run holds. The report is written from the run as far as it went.
//!
//! `docs/design/ARCHITECTURE.md`, "Shape".

use std::collections::BTreeSet;
use std::io::{Cursor, Write as _};

use camino::Utf8Path;
use zip::CompressionMethod;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;

use crate::archive::{Archive, Caps};
use crate::diag::{DiagnosticPolicy, Diagnostics, Halted, Render, Stage};
use crate::env::Env;
use crate::facts::{FactInputs, Facts, FactsError};
use crate::fixups::{Changes, FixError, FixInputs, FixedMod, FixupSet, ObjectSet, TargetTypes};
use crate::game::{GameError, NameBook, Speller};
use crate::load::{
    Dating, Fault, FileProvenance, Inventory, LoadError, LoadOptions, Loader, ReworkTable, against,
};
use crate::model::Mod;
use crate::package::{
    BuildError, CheckCx, CheckSet, EvidenceDoc, InPlaceBuilder, Made, Overlay, Package,
    PackageWriter, PortBuilder, Refused, WriteError, WriterKind, permit,
};
use crate::port::{PlanError, Policy, Port, PortInputs, PortOptions, Targets};
use crate::report::{AgainRow, Report};

/// The name the second run of a repair reads the first run's package under.
const REPAIRED_NAME: &str = "repaired.fantome";

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
    read: bool,
    archive: Option<Archive>,
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
    overlay: Option<Overlay>,
    checked: bool,
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
            read: false,
            archive: None,
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
            overlay: None,
            checked: false,
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

    /// Ends a stage's method where a fatal diagnostic ended the run.
    fn going(&self) -> Result<(), RunError> {
        self.log
            .halted()
            .map_or(Ok(()), |halted| Err(halted.into()))
    }

    /// The stage `read`: the archive from its bytes, which are borrowed for the call.
    /// `file_name` is for display and is never opened.
    pub fn read(&mut self, bytes: &[u8], file_name: &str) -> Result<&Archive, RunError> {
        self.going()?;
        let mut sink = self.log.at(Stage::Read);
        let archive = Archive::read(bytes, file_name, &self.settings.caps, &mut sink)?;
        self.read = true;
        Ok(self.archive.insert(archive))
    }

    /// The stage `read`, of an archive file.
    pub fn read_file(&mut self, path: &Utf8Path) -> Result<&Archive, RunError> {
        self.going()?;
        let mut sink = self.log.at(Stage::Read);
        let archive = Archive::read_file(path, &self.settings.caps, &mut sink)?;
        self.read = true;
        Ok(self.archive.insert(archive))
    }

    /// The stage `load`: the mod from the archive read, then what is known of its files
    /// before any value is read. A porter's run gathers the dating, the inventory and the
    /// file provenance. A repairer's run gathers the dating alone: nothing of a repair reads
    /// the other two.
    pub fn load(&mut self) -> Result<&Mod, RunError> {
        self.going()?;
        if matches!(self.module, Reached::Nothing)
            && let Some(archive) = self.archive.take()
        {
            self.load_archive(archive)?;
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

    fn load_archive(&mut self, archive: Archive) -> Result<(), RunError> {
        let env = self.env;
        let book = std::mem::take(&mut self.book);
        let options = self.settings.load_options();
        let mut loader = Loader::new(env).options(options).book(book);
        let mut sink = self.log.at(Stage::Load);
        let mut module = loader.open(archive);
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
            None => self.dating = Some(found),
        }
        self.module = Reached::Loaded(module);
        Ok(())
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
            self.targets = Some(targets.compared(facts, &self.settings.compared));
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
                        PortBuilder::new(fixed, port, self.env, &speller).build(&mut sink)
                    }
                    Mode::InPlace => {
                        let changes = held(self.changes.as_ref(), Stage::Build, Stage::Fix)?;
                        let options = self.settings.load_options();
                        InPlaceBuilder::new(fixed, changes)
                            .names(&self.book, self.env.names())
                            .judged(self.env, &options)
                            .build(&mut sink)
                    }
                }
            };
            let mut package = built.map_err(RunError::of_build)?;
            package.evidence = self.evidence()?;
            if let Some(image) = self.module.module().and_then(|m| m.image.as_ref()) {
                package.carry_image(image, &mut self.log.at(Stage::Build));
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
    /// package's patches to the game's bins first, and holds the result. What a check finds
    /// is a diagnostic and no error (#ADR-0003).
    pub fn check(&mut self) -> Result<(), RunError> {
        self.going()?;
        if self.checked {
            return Ok(());
        }
        let fixed = self.module.fixed(Stage::Check)?;
        let package = held(self.package.as_ref(), Stage::Check, Stage::Build)?;
        let checks = &self.settings.checks;
        match self.mode {
            Mode::Port => {
                let port = held(self.plan.as_ref(), Stage::Check, Stage::Plan)?;
                let game = self.env.game();
                let overlay = Overlay::apply(package, game).map_err(RunError::Check)?;
                let types = TargetTypes::of(fixed, self.env).map_err(RunError::Check)?;
                let cx = CheckCx::new(package, &types).of_port(game, &overlay, port);
                checks.run(&cx, &mut self.log.at(Stage::Check));
                self.overlay = Some(overlay);
            }
            Mode::InPlace => {
                let types = TargetTypes::of(fixed, self.env).map_err(RunError::Check)?;
                let cx = CheckCx::new(package, &types);
                checks.run(&cx, &mut self.log.at(Stage::Check));
            }
        }
        self.checked = true;
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
            report = report.package_applied(package, self.overlay.as_ref());
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
            Stage::Read => self.read,
            Stage::Load => matches!(self.module, Reached::Loaded(_) | Reached::Fixed(_)),
            Stage::Fix => matches!(self.module, Reached::Fixed(_)),
            Stage::Facts => self.facts.is_some(),
            Stage::Decide => self.targets.is_some(),
            Stage::Plan => self.plan.is_some(),
            Stage::Build => self.package.is_some(),
            Stage::Check => self.checked,
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

    /// The archive read, until `load` takes it.
    pub fn archive(&self) -> Option<&Archive> {
        self.archive.as_ref()
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

    /// The package's patches applied to the game's bins, once a porter's run is checked.
    pub fn overlay(&self) -> Option<&Overlay> {
        self.overlay.as_ref()
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
