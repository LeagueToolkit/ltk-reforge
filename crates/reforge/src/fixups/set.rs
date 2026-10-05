//! The set of fixups a run applies, the choice of them as data, and the stage itself: the set
//! applied to a loaded mod.

use std::collections::BTreeSet;
use std::io;
use std::ops::Deref;
use std::sync::Arc;

use camino::Utf8Path;
use fs_err as fs;
use serde::{Deserialize, Serialize};

use super::changes::Changes;
use super::context::FixCx;
use super::rule::Rule;
use super::scope::ObjectSet;
use super::types::TargetTypes;
use super::{BinFixup, built_in, findings};
use crate::diag::Sink;
use crate::env::Env;
use crate::game::{GameError, NameBook, NameHashError};
use crate::history::HistoryError;
use crate::load::{Dating, FileFixup, LoadOptions};
use crate::model::entity::ObjectId;
use crate::model::{FixupId, FixupInfo, Mod, walk};

/// Why a selection does not load or save.
#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    /// The file does not read or write.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// The text is not the JSON of a selection.
    #[error("not a selection of fixups")]
    Json(#[from] serde_json::Error),
}

/// A choice of fixups, as data: saved, loaded and edited with no code.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixupSelection {
    /// Fixups turned off: left out of the set whatever their default.
    #[serde(default)]
    pub off: BTreeSet<FixupId>,
    /// Fixups turned on that the standard set leaves out.
    #[serde(default)]
    pub on: BTreeSet<FixupId>,
    /// Rules added. A rule with the id of a built-in fixup takes its place.
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl FixupSelection {
    /// The standard set: nothing turned off or on, no rule added.
    pub fn standard() -> Self {
        Self::default()
    }

    /// Turns a fixup off.
    #[must_use]
    pub fn without(mut self, id: impl Into<FixupId>) -> Self {
        let id = id.into();
        self.on.remove(&id);
        self.off.insert(id);
        self
    }

    /// Turns a fixup on.
    #[must_use]
    pub fn with(mut self, id: impl Into<FixupId>) -> Self {
        let id = id.into();
        self.off.remove(&id);
        self.on.insert(id);
        self
    }

    /// Adds a rule.
    #[must_use]
    pub fn rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    /// The selection a JSON text holds.
    pub fn from_json(json: &str) -> Result<Self, SelectionError> {
        Ok(serde_json::from_str(json)?)
    }

    /// The selection as JSON.
    ///
    /// # Panics
    ///
    /// When serde fails to write a selection into memory, which no value of one makes it do.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("a selection of fixups does not write: {error}"))
    }

    /// Reads the selection a file holds.
    pub fn open(path: &Utf8Path) -> Result<Self, SelectionError> {
        Self::from_json(&fs::read_to_string(path)?)
    }

    /// Writes the selection to a file.
    pub fn save(&self, path: &Utf8Path) -> Result<(), SelectionError> {
        Ok(fs::write(path, self.to_json() + "\n")?)
    }

    /// The ids turned off or on that name no built-in fixup and no rule of the selection: a
    /// caller reports them, and the set leaves them out.
    pub fn unknown(&self) -> Vec<&FixupId> {
        let catalog = FixupSet::catalog();
        let known = |id: &&FixupId| {
            catalog.iter().any(|info| info.id == **id)
                || self.rules.iter().any(|rule| rule.info().id == **id)
        };
        let named = self.off.iter().chain(&self.on);
        named.filter(|id| !known(id)).collect()
    }

    /// Whether the selection holds a fixup with this description.
    fn takes(&self, info: &FixupInfo) -> bool {
        !self.off.contains(&info.id) && (info.default || self.on.contains(&info.id))
    }
}

/// An input of the run that did not answer while the fixups were applied: a fault, and no
/// diagnostic.
#[derive(Debug, thiserror::Error)]
pub enum FixError {
    /// A chunk the game holds did not read.
    #[error("the game did not read while the mod was fixed")]
    Game(#[from] GameError),
    /// The name tables did not answer.
    #[error("the name tables did not answer while the mod was fixed")]
    Names(#[from] NameHashError),
    /// The history's index did not answer.
    #[error("the history did not answer while the mod was fixed")]
    History(#[from] HistoryError),
}

/// A mod the stage `fix` ended on a fault. Its files, characters and units are as `load` left
/// them. Its objects hold what the fixups changed before the fault, so it is no [`FixedMod`].
pub struct Unfixed {
    /// The mod.
    pub module: Mod,
    /// What ended the stage.
    pub fault: FixError,
}

impl std::fmt::Debug for Unfixed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut shown = f.debug_struct("Unfixed");
        shown.field("fault", &self.fault).finish_non_exhaustive()
    }
}

/// What a set is applied against, beside the mod: the target, the build the mod was made at,
/// and the run's own names, which learn what the fixups write.
pub struct FixInputs<'a> {
    /// The environment: the game, the schema, the name tables.
    pub env: &'a Env,
    /// The oldest build the mod can have been made at.
    pub dating: &'a Dating,
    /// The names the run learns.
    pub names: &'a mut NameBook,
}

/// A mod whose values are what the target build reads. Only [`FixupSet::apply`] makes one, and
/// it is read-only: what is compared with the game afterwards is compared at the target's
/// types.
pub struct FixedMod(Mod);

impl FixedMod {
    /// The mod, to take through the fixups again or to change. What comes back is no fixed
    /// mod until a set is applied to it.
    pub fn into_mod(self) -> Mod {
        self.0
    }
}

impl Deref for FixedMod {
    type Target = Mod;

    fn deref(&self) -> &Mod {
        &self.0
    }
}

/// The fixups of a run, in the order they run: by phase, within the layout phase by build,
/// the oldest first, then by id (#ADR-0007). Among the fixups on files, one that runs at a
/// later step of `load` comes after the others.
#[derive(Clone)]
pub struct FixupSet {
    bins: Vec<Arc<dyn BinFixup>>,
    files: Vec<Arc<dyn FileFixup>>,
}

impl Default for FixupSet {
    fn default() -> Self {
        Self::standard()
    }
}

impl FixupSet {
    /// A set with no fixup in it.
    pub fn empty() -> Self {
        Self {
            bins: Vec::new(),
            files: Vec::new(),
        }
    }

    /// Every built-in fixup whose default is on.
    pub fn standard() -> Self {
        Self::from(&FixupSelection::standard())
    }

    /// The description of every built-in fixup, on the files and on the bins alike, in the
    /// order they run.
    pub fn catalog() -> Vec<FixupInfo> {
        let files = Self::standard_file_fixups();
        let mut bins = built_in();
        bins.sort_by(|a, b| a.info().order().cmp(&b.info().order()));
        let files = files.iter().map(|fixup| fixup.info().clone());
        let bins = bins.iter().map(|fixup| fixup.info().clone());
        files.chain(bins).collect()
    }

    /// The built-in file fixups in the order they run: by the step of `load` each runs at,
    /// then as every fixup.
    fn standard_file_fixups() -> Vec<Arc<dyn FileFixup>> {
        let mut files = LoadOptions::standard_fixups();
        files.sort_by(|a, b| (a.step(), a.info().order()).cmp(&(b.step(), b.info().order())));
        files
    }

    /// Adds a fixup on the bins, at its place in the order. One with the id of a fixup the
    /// set holds takes its place.
    #[must_use]
    pub fn with(mut self, fixup: impl BinFixup + 'static) -> Self {
        self.add(Arc::new(fixup));
        self
    }

    fn add(&mut self, fixup: Arc<dyn BinFixup>) {
        self.bins.retain(|held| held.info().id != fixup.info().id);
        self.bins.push(fixup);
        self.bins
            .sort_by(|a, b| a.info().order().cmp(&b.info().order()));
    }

    /// The fixups on the bins, in the order they run.
    pub fn bin_fixups(&self) -> &[Arc<dyn BinFixup>] {
        &self.bins
    }

    /// The fixups on the file set, in the order they run: what `load` is given.
    pub fn file_fixups(&self) -> Vec<Arc<dyn FileFixup>> {
        self.files.clone()
    }

    /// The description of every fixup of the set, in the order they run.
    pub fn infos(&self) -> Vec<&FixupInfo> {
        let files = self.files.iter().map(|fixup| fixup.info());
        files
            .chain(self.bins.iter().map(|fixup| fixup.info()))
            .collect()
    }

    /// The id of every fixup of the set, in the order they run.
    pub fn ids(&self) -> Vec<&str> {
        let infos = self.infos();
        infos.into_iter().map(|info| info.id.as_str()).collect()
    }

    /// The stage `fix`: applies the fixups on the bins, in order, to the objects of a set.
    /// Each fixup walks every object once and is called for each struct of its classes. What
    /// each did is raised once per fixup and outcome, through a sink that names the fixup and
    /// the record it implements.
    ///
    /// A fixup whose target shape the build does not have is skipped. A fault a fixup reports
    /// ends the run: there is no fixed mod then.
    pub fn apply(
        &self,
        m: Mod,
        objects: &ObjectSet,
        inputs: &mut FixInputs<'_>,
        sink: &mut Sink<'_>,
    ) -> Result<(FixedMod, Changes), FixError> {
        let applied = self.apply_or_keep(m, objects, inputs, sink);
        applied.map_err(|unfixed| unfixed.fault)
    }

    /// [`FixupSet::apply`], giving the mod back with the fault where one ends the stage, for a
    /// caller that reports the run as far as it went.
    pub fn apply_or_keep(
        &self,
        mut m: Mod,
        objects: &ObjectSet,
        inputs: &mut FixInputs<'_>,
        sink: &mut Sink<'_>,
    ) -> Result<(FixedMod, Changes), Box<Unfixed>> {
        match self.fix_in(&mut m, objects, inputs, sink) {
            Ok(changes) => Ok((FixedMod(m), changes)),
            Err(fault) => Err(Box::new(Unfixed { module: m, fault })),
        }
    }

    fn fix_in(
        &self,
        m: &mut Mod,
        objects: &ObjectSet,
        inputs: &mut FixInputs<'_>,
        sink: &mut Sink<'_>,
    ) -> Result<Changes, FixError> {
        let _span = tracing::info_span!("fix", fixups = self.bins.len()).entered();
        let types = TargetTypes::of(m, inputs.env)?;
        let ids: Vec<ObjectId> = m.objects.ids().filter(|&o| objects.contains(o)).collect();
        let mut changes = Changes::default();
        let mut fault = None;
        for fixup in self.bins.iter().filter(|fixup| fixup.applies(&types)) {
            let classes = fixup.classes();
            for &object in &ids {
                let mut cx = FixCx {
                    types: &types,
                    source: inputs.dating.build(),
                    characters: &m.characters,
                    units: &m.units,
                    tables: inputs.env.names(),
                    book: &mut *inputs.names,
                    fixup: &fixup.info().id,
                    object,
                    changes: &mut changes,
                    fault: &mut fault,
                };
                walk::structs_mut(&mut m.objects[object].value, |s| {
                    if classes.holds(s.class()) && !cx.faulted() {
                        fixup.fix(&mut cx, s);
                    }
                });
                if let Some(fault) = fault.take() {
                    return Err(fault);
                }
            }
        }
        for fixup in &self.bins {
            let info = fixup.info();
            let made = changes.of(info.id.as_str());
            if !made.is_empty() {
                findings::raise(made, &mut sink.by(info.id.as_str(), info.one_record()));
            }
        }
        tracing::debug!(objects = ids.len(), changes = changes.len(), "fixed");
        Ok(changes)
    }
}

/// The set a selection gives: every built-in fixup it does not turn off, whose default is on
/// or which it turns on, and the rules it adds.
impl From<&FixupSelection> for FixupSet {
    fn from(selection: &FixupSelection) -> Self {
        let mut set = Self::empty();
        for fixup in built_in() {
            if selection.takes(fixup.info()) {
                set.add(fixup);
            }
        }
        for rule in &selection.rules {
            if selection.takes(rule.info()) {
                set.add(Arc::new(rule.clone()));
            }
        }
        let files = Self::standard_file_fixups().into_iter();
        set.files = files
            .filter(|fixup| selection.takes(fixup.info()))
            .collect();
        set
    }
}

impl std::fmt::Debug for FixupSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.ids()).finish()
    }
}
