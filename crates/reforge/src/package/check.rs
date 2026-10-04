//! The stage `check`: a built package read as the client reads it. A check is an object that
//! describes itself ([`CheckInfo`]) and is chosen as a fixup is: a [`CheckSelection`] is the
//! choice as data, a [`CheckSet`] the checks of a run. What a check finds is a diagnostic and
//! does not stop the write (#ADR-0003).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fmt;
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use indexmap::IndexMap;
use ltk_meta::{Bin, BinObject, PropertyValueEnum};
use reforge_wwise::{Bank, BankKind, CLIENT_VERSION};
use serde::{Deserialize, Serialize};

use super::findings::{
    Against, ApplyFailed, Coverage, Done, FieldUndeclared, FileMissing, LinkUnresolved,
    ManifestUnreadable, NameUnresolved, Place, RecordSkipped, RoundTripFailed, RoundTripProblem,
    Shape, TypeMismatch, TypeRow, UndeclaredRow,
};
use super::overlay::{Loaded, Outcome, Overlay};
use super::{Layer, Origin, Package, PatchOf, WrittenBin};
use crate::diag::{Record, Sink};
use crate::fixups::{TargetTypes, TypeSource};
use crate::game::{FieldType, Game as _, GameBin, GameView};
use crate::model::entity::UnitId;
use crate::model::{BinHash, Fields, SkinEntry, WadHash, bin, panic_text, path_hash, value};
use crate::port::{FileVerdict, Group, Port};

/// The name a caller turns a check on and off by: `round-trip`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CheckId(Cow<'static, str>);

impl CheckId {
    /// The id of a built-in check.
    pub const fn built_in(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CheckId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&self.0)
    }
}

impl From<&str> for CheckId {
    fn from(id: &str) -> Self {
        Self(Cow::Owned(id.to_string()))
    }
}

/// What a caller lists and toggles a check by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckInfo {
    /// The name it is turned on and off by.
    pub id: CheckId,
    /// One line for a person: what it checks.
    pub title: Cow<'static, str>,
    /// The record it rests on, when it has one.
    pub record: Option<Record>,
    /// Whether the standard set holds it.
    pub default: bool,
}

impl CheckInfo {
    /// The description of a built-in check of the standard set.
    pub const fn built_in(id: &'static str, title: &'static str, record: Option<Record>) -> Self {
        Self {
            id: CheckId::built_in(id),
            title: Cow::Borrowed(title),
            record,
            default: true,
        }
    }
}

/// What a check reads: the package, the types the target build reads, and for a port its
/// plan and its patches applied.
pub struct CheckCx<'a> {
    /// The package. A check changes nothing in it.
    pub package: &'a Package,
    /// The target build's type of each field of each class.
    pub types: &'a TargetTypes<'a>,
    /// The game the package is for. Without it no file is looked for.
    pub game: Option<&'a GameView>,
    /// The package's patches applied to the game's bins. None for a package with no patch.
    pub overlay: Option<&'a Overlay>,
    /// The plan the package was built from. None for a repair.
    pub port: Option<&'a Port>,
}

impl<'a> CheckCx<'a> {
    /// What the checks of a repair's package read.
    pub fn new(package: &'a Package, types: &'a TargetTypes<'a>) -> Self {
        Self {
            package,
            types,
            game: None,
            overlay: None,
            port: None,
        }
    }

    /// What the checks of a port's package read.
    #[must_use]
    pub fn of_port(mut self, game: &'a GameView, overlay: &'a Overlay, port: &'a Port) -> Self {
        self.game = Some(game);
        self.overlay = Some(overlay);
        self.port = Some(port);
        self
    }
}

/// A test of a built package.
pub trait Check: Send + Sync {
    /// What a caller lists and toggles it by.
    fn info(&self) -> &CheckInfo;

    /// Reads the package and raises what it finds, and what it covered.
    fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>);
}

/// Every built-in check, in the order they run.
fn built_in() -> Vec<Arc<dyn Check>> {
    vec![Arc::new(Types), Arc::new(RoundTrip), Arc::new(Apply)]
}

/// A choice of checks, as data: saved and loaded beside a choice of fixups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSelection {
    /// Checks turned off: left out of the set whatever their default.
    #[serde(default)]
    pub off: BTreeSet<CheckId>,
    /// Checks turned on that the standard set leaves out.
    #[serde(default)]
    pub on: BTreeSet<CheckId>,
}

impl CheckSelection {
    /// The standard set: nothing turned off or on.
    pub fn standard() -> Self {
        Self::default()
    }

    /// Turns a check off.
    #[must_use]
    pub fn without(mut self, id: impl Into<CheckId>) -> Self {
        let id = id.into();
        self.on.remove(&id);
        self.off.insert(id);
        self
    }

    /// Turns a check on.
    #[must_use]
    pub fn with(mut self, id: impl Into<CheckId>) -> Self {
        let id = id.into();
        self.off.remove(&id);
        self.on.insert(id);
        self
    }

    /// The ids turned off or on that name no built-in check: a caller reports them, and the
    /// set leaves them out.
    pub fn unknown(&self) -> Vec<&CheckId> {
        let catalog = CheckSet::catalog();
        let known = |id: &&CheckId| catalog.iter().any(|info| info.id == **id);
        let named = self.off.iter().chain(&self.on);
        named.filter(|id| !known(id)).collect()
    }

    fn takes(&self, info: &CheckInfo) -> bool {
        !self.off.contains(&info.id) && (info.default || self.on.contains(&info.id))
    }
}

/// The checks of a run, in the order they run.
#[derive(Clone)]
pub struct CheckSet {
    checks: Vec<Arc<dyn Check>>,
}

impl Default for CheckSet {
    fn default() -> Self {
        Self::standard()
    }
}

impl CheckSet {
    /// A set with no check in it.
    pub fn empty() -> Self {
        Self { checks: Vec::new() }
    }

    /// Every built-in check whose default is on.
    pub fn standard() -> Self {
        Self::from(&CheckSelection::standard())
    }

    /// The description of every built-in check, in the order they run.
    pub fn catalog() -> Vec<CheckInfo> {
        let checks = built_in();
        checks.iter().map(|check| check.info().clone()).collect()
    }

    /// Adds a check after the ones the set holds. One with the id of a check the set holds
    /// takes its place.
    #[must_use]
    pub fn with(mut self, check: impl Check + 'static) -> Self {
        let id = &check.info().id;
        match self.checks.iter().position(|held| held.info().id == *id) {
            Some(place) => self.checks[place] = Arc::new(check),
            None => self.checks.push(Arc::new(check)),
        }
        self
    }

    /// The description of every check of the set, in the order they run.
    pub fn infos(&self) -> Vec<&CheckInfo> {
        self.checks.iter().map(|check| check.info()).collect()
    }

    /// The stage `check`: runs each check on the package, each through a sink that names the
    /// check and the record it rests on.
    pub fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>) {
        let _span = tracing::info_span!("check", checks = self.checks.len()).entered();
        for check in &self.checks {
            let info = check.info();
            check.run(cx, &mut sink.by(info.id.as_str(), info.record));
        }
    }
}

/// The set a selection gives: every built-in check it does not turn off, whose default is on
/// or which it turns on.
impl From<&CheckSelection> for CheckSet {
    fn from(selection: &CheckSelection) -> Self {
        let checks = built_in().into_iter();
        Self {
            checks: checks
                .filter(|check| selection.takes(check.info()))
                .collect(),
        }
    }
}

impl fmt::Debug for CheckSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ids = self.checks.iter().map(|check| check.info().id.as_str());
        f.debug_list().entries(ids).finish()
    }
}

/// The patches of a port, applied as the overlay applies them (#ADR-0026): every record
/// applies, every link the port adds leads to a bin, and every name and file the patched
/// entry reaches through the port's objects resolves in what the patched bin loads.
///
/// A unit's names resolve against its group's loads together: a champion and its
/// sub-characters at one target load together and share one set of names. Names and files
/// already unresolved in the mod, and loose ends of the game's own entry and resolver, are
/// counted apart.
#[derive(Debug, Clone, Copy, Default)]
pub struct Apply;

static APPLY: CheckInfo = CheckInfo::built_in(
    "apply",
    "Each patch applies to the game's bin, and what the patched entry reaches resolves",
    None,
);

/// The path the client reads when a file is missing: an old particle's `.troy` is read as
/// the `.troybin` beside it (#GAME-BEHAVIOR-0009).
fn client_fallback(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    let stem = lower.strip_suffix(".troy")?;
    Some(format!("{stem}.troybin"))
}

/// The names and files the plan found unresolved for the units of one group: a champion and
/// its sub-characters at one target load together, so a loose end of one is no new one of
/// another.
#[derive(Default)]
struct LooseEnds {
    names: BTreeSet<BinHash>,
    files: BTreeSet<WadHash>,
    /// The patched entries of the group's units, and the resolvers they link.
    entries: BTreeSet<BinHash>,
}

/// The walk of one port's patched entries.
struct Applying<'a> {
    cx: &'a CheckCx<'a>,
    port: &'a Port,
    /// The loads of each group, merged.
    groups: HashMap<Group, Loaded>,
    /// What the units of each group left unresolved in the mod already.
    loose: HashMap<Group, LooseEnds>,
    /// The banks a voice layer's audio block names that the client does not find with the
    /// layer on, by unit.
    layers: BTreeMap<UnitId, BTreeSet<WadHash>>,
    reached: usize,
    known: (usize, usize),
    games: (usize, usize),
}

impl Applying<'_> {
    /// Whether the client finds a file: the package ships it, or the game holds it.
    fn present(&self, path: WadHash) -> bool {
        let game = self.cx.game.is_some_and(|game| game.contains(path));
        game || self.cx.package.by_hash(path).is_some()
    }

    /// The banks the audio block of an entry names, as a voice layer's patch leaves it, that
    /// the client does not find with the layer on: neither the game's, nor shipped in the
    /// layer or in the base layer.
    fn layer_missing(&self, layer: &Layer, entry: &BinObject) -> BTreeSet<WadHash> {
        let shipped = |path: WadHash| {
            let chunks = self.cx.package.layer(layer);
            let mut chunks = chunks.chain(self.cx.package.layer(&Layer::Base));
            chunks.any(|(key, _)| key.hash() == path)
        };
        let units = SkinEntry::read(entry).audio().units;
        let paths = units.iter().flat_map(|unit| &unit.paths);
        let paths = paths.map(|path| path_hash(path));
        let game = |path: WadHash| self.cx.game.is_some_and(|game| game.contains(path));
        paths
            .filter(|&path| !game(path) && !shipped(path))
            .collect()
    }

    /// Walks what the patched entry of a unit reaches, and raises what does not resolve.
    fn unit(&mut self, unit: UnitId, entry: BinHash, game: &GameBin, sink: &mut Sink<'_>) {
        let Some(group) = self.port.unit(unit).map(|u| (u.champion, u.target)) else {
            return;
        };
        let Some(loaded) = self.groups.get(&group) else {
            return;
        };
        let known = self.loose.get(&group);
        let known_names = known.map(|known| &known.names);
        let known_file = |path: WadHash| known.is_some_and(|known| known.files.contains(&path));
        // What the game's own entry and resolver name, unpatched: the game's loose ends are
        // not the port's.
        let mut game_names: BTreeSet<BinHash> = BTreeSet::new();
        let mut game_files: BTreeSet<WadHash> = BTreeSet::new();
        if let Some(held) = game.object(entry) {
            let resolver = SkinEntry::read(&held.value).resolver();
            let resolver = resolver.and_then(|name| game.object(name));
            for object in std::iter::once(held).chain(resolver) {
                game_names.extend(value::links(&object.value));
                let files = value::file_refs(&object.value);
                game_files.extend(files.iter().map(value::FileRef::hash));
            }
        }
        // What the port answers for: the entries of the group's units, the resolvers they
        // link, the objects it ships.
        let ours = |name: BinHash| {
            let patched = known.is_some_and(|known| known.entries.contains(&name));
            patched || loaded.shipped.contains(&name)
        };
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([entry]);
        let mut unresolved = Vec::new();
        let mut missing = BTreeSet::new();
        while let Some(name) = queue.pop_front() {
            if !seen.insert(name) {
                continue;
            }
            let Some(held) = loaded.objects.get(&name) else {
                if known_names.is_some_and(|known| known.contains(&name)) {
                    self.known.0 += 1;
                } else if game_names.contains(&name) {
                    self.games.0 += 1;
                } else {
                    unresolved.push(name);
                }
                continue;
            };
            // The game's own objects link what the game's do, and are not followed.
            if !ours(name) {
                continue;
            }
            let object = held.object();
            queue.extend(value::links(object));
            for file in value::file_refs(object) {
                let path = file.hash();
                let fallback = file.spelled().and_then(client_fallback);
                let fallback = fallback.is_some_and(|path| self.present(path_hash(&path)));
                if self.present(path) || fallback {
                    continue;
                }
                if known_file(path) {
                    self.known.1 += 1;
                } else if game_files.contains(&path) {
                    self.games.1 += 1;
                } else {
                    missing.insert(path);
                }
            }
        }
        self.reached += seen.len();
        // The game's own loose ends are not the layer's.
        let of_layers = self.layers.remove(&unit).unwrap_or_default();
        missing.extend(of_layers.difference(&game_files));
        if !unresolved.is_empty() {
            sink.raise(NameUnresolved {
                unit,
                names: unresolved,
            });
        }
        if !missing.is_empty() {
            sink.raise(FileMissing {
                unit,
                files: missing.into_iter().collect(),
            });
        }
    }
}

impl Check for Apply {
    fn info(&self) -> &CheckInfo {
        &APPLY
    }

    fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>) {
        let (Some(overlay), Some(port)) = (cx.overlay, cx.port) else {
            return;
        };
        for (layer, reason) in &overlay.manifests {
            sink.raise(ManifestUnreadable {
                layer: layer.name(),
                reason: reason.clone(),
            });
        }
        let mut applying = Applying {
            cx,
            port,
            groups: HashMap::new(),
            loose: HashMap::new(),
            layers: BTreeMap::new(),
            reached: 0,
            known: (0, 0),
            games: (0, 0),
        };
        let mut patches = 0;
        let mut units = Vec::new();
        for applied in &overlay.patches {
            let Some(patch) = cx.package.patches(&applied.layer).get(applied.index) else {
                continue;
            };
            let failed = |reason: Option<String>| ApplyFailed {
                layer: applied.layer.name(),
                target: patch.target.clone(),
                reason,
            };
            let patched = match &applied.outcome {
                Outcome::NoGameBin => {
                    sink.raise(failed(None));
                    continue;
                }
                Outcome::Failed { reason } => {
                    sink.raise(failed(Some(reason.clone())));
                    continue;
                }
                Outcome::Applied(patched) => patched,
            };
            patches += 1;
            if !patched.skipped.is_empty() {
                sink.raise(RecordSkipped {
                    layer: applied.layer.name(),
                    target: patch.target.clone(),
                    records: patched.skipped.clone(),
                });
            }
            if let (true, PatchOf::Skin { unit, entry }) = (applied.layer != Layer::Base, patch.of)
            {
                let held = patched.objects.iter().find(|o| o.path_hash == entry);
                let missing = held.map(|held| applying.layer_missing(&applied.layer, held));
                let missing = missing.unwrap_or_default();
                if !missing.is_empty() {
                    let of_unit = applying.layers.entry(unit).or_default();
                    of_unit.extend(missing);
                }
            }
            let (Some(loaded), PatchOf::Skin { unit, entry }) = (&patched.loaded, patch.of) else {
                continue;
            };
            if !loaded.missing_links.is_empty() {
                sink.raise(LinkUnresolved {
                    target: patch.target.clone(),
                    links: loaded.missing_links.clone(),
                });
            }
            let Some(group) = port.unit(unit).map(|u| (u.champion, u.target)) else {
                continue;
            };
            let merged = applying.groups.entry(group).or_default();
            for (name, held) in &loaded.objects {
                merged.objects.entry(*name).or_insert_with(|| held.clone());
            }
            merged.shipped.extend(loaded.shipped.iter().copied());
            let loose = applying.loose.entry(group).or_default();
            loose.entries.insert(entry);
            let resolver = loaded.objects.get(&entry);
            let resolver = resolver.and_then(|held| SkinEntry::read(held.object()).resolver());
            loose.entries.extend(resolver);
            if let Some(reach) = port.reach.of(unit) {
                loose.names.extend(reach.dangling.iter().copied());
            }
            let verdicts = port.refs.of(unit).into_iter().flat_map(|refs| &refs.refs);
            let unfound = verdicts.filter(|(_, verdict)| {
                matches!(
                    verdict,
                    FileVerdict::Dangling | FileVerdict::Unmounted { .. }
                )
            });
            loose.files.extend(unfound.map(|(path, _)| *path));
            if let Some(game) = &applied.game {
                units.push((unit, entry, Arc::clone(game)));
            }
        }
        for (unit, entry, game) in units {
            applying.unit(unit, entry, &game, sink);
        }
        for (unit, files) in std::mem::take(&mut applying.layers) {
            let files = files.into_iter().collect();
            sink.raise(FileMissing { unit, files });
        }
        sink.raise(Done(Coverage::Apply {
            patches,
            reached: applying.reached,
            names_known: applying.known.0,
            files_known: applying.known.1,
            names_game: applying.games.0,
            files_game: applying.games.1,
        }));
    }
}

/// Every bin written again reads back, holds the entries it was built with, and holds each
/// changed object as it was built. Every bank a port made reads back, a sound bank at the
/// version the client loads.
#[derive(Debug, Clone, Copy, Default)]
pub struct RoundTrip;

static ROUND_TRIP: CheckInfo = CheckInfo::built_in(
    "round-trip",
    "A bin written again reads back as it was built, and a bank made reads back",
    None,
);

impl RoundTrip {
    /// How the bytes of a bin differ from what it was built to hold.
    fn problems(bytes: &[u8], written: &WrittenBin) -> Vec<RoundTripProblem> {
        // The bytes are this run's own, and the reader is another crate's.
        let read = catch_unwind(AssertUnwindSafe(|| {
            Bin::from_reader(&mut Cursor::new(bytes)).map_err(|error| error.to_string())
        }));
        let back = match read.unwrap_or_else(|payload| Err(panic_text(&*payload))) {
            Ok(back) => back,
            Err(reason) => return vec![RoundTripProblem::Unreadable { reason }],
        };
        let mut problems = Vec::new();
        let read: BTreeSet<BinHash> = back.objects.keys().copied().collect();
        if read != written.entries {
            problems.push(RoundTripProblem::Entries {
                built: written.entries.len(),
                read: read.len(),
            });
        }
        for object in &written.edited {
            if back.objects.get(&object.path_hash) != Some(object) {
                problems.push(RoundTripProblem::Object {
                    entry: object.path_hash,
                });
            }
        }
        problems
    }

    /// Why a bank the run made does not load: it does not read, or it is of another version
    /// than the client loads (#GAME-BEHAVIOR-0015).
    fn bank_problems(bytes: &[u8]) -> Vec<RoundTripProblem> {
        let reason = match Bank::read(bytes) {
            Err(error) => error.to_string(),
            Ok(bank) if bank.kind == BankKind::Bnk && bank.version != CLIENT_VERSION => {
                format!("a bank of version {}", bank.version)
            }
            Ok(_) => return Vec::new(),
        };
        vec![RoundTripProblem::Unreadable { reason }]
    }
}

impl Check for RoundTrip {
    fn info(&self) -> &CheckInfo {
        &ROUND_TRIP
    }

    fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>) {
        let (mut bins, mut objects, mut banks) = (0, 0, 0);
        for (_, key, chunk) in cx.package.chunks() {
            let problems = match (&chunk.origin, chunk.origin.written()) {
                (_, Some(written)) => {
                    bins += 1;
                    objects += written.edited.len();
                    Self::problems(&chunk.bytes, written)
                }
                (Origin::Bank(_), None) => {
                    banks += 1;
                    Self::bank_problems(&chunk.bytes)
                }
                _ => continue,
            };
            if !problems.is_empty() {
                sink.raise(RoundTripFailed {
                    wad: key.wad.clone(),
                    path: key.path.clone(),
                    problems,
                });
            }
        }
        sink.raise(Done(Coverage::RoundTrip {
            bins,
            objects,
            banks,
        }));
    }
}

/// Every value of every bin the package ships is of the type the target build reads there,
/// on a field its class declares. The types are the schema's at the install's build, and for
/// a field the schema does not describe what the install's bins hold. Without a schema
/// nothing is said of undeclared fields.
#[derive(Debug, Clone, Copy, Default)]
pub struct Types;

static TYPES: CheckInfo = CheckInfo::built_in(
    "types",
    "Every value written is typed as the target build reads it",
    Some(Record::game_behavior(2)),
);

impl Check for Types {
    fn info(&self) -> &CheckInfo {
        &TYPES
    }

    fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>) {
        let mut typing = Typing {
            types: cx.types,
            fields: 0,
            wrong: IndexMap::new(),
            undeclared: IndexMap::new(),
        };
        let mut bins = 0;
        for (_, key, chunk) in cx.package.bins() {
            // A bin that does not read is `load`'s finding, and the client reads none of it.
            let Ok(parsed) = bin::parse(&chunk.bytes) else {
                continue;
            };
            bins += 1;
            for object in &parsed.objects {
                let place = Place {
                    wad: key.wad.clone(),
                    path: key.path.clone(),
                    entry: object.value.path_hash,
                };
                typing.object(&object.value, &place);
            }
        }
        // The fields each patch changes in the bin it patches.
        let patches = cx.overlay.into_iter().flat_map(|overlay| &overlay.patches);
        for applied in patches {
            let (Outcome::Applied(patched), Some(wad)) = (&applied.outcome, &applied.wad) else {
                continue;
            };
            let Some(patch) = cx.package.patches(&applied.layer).get(applied.index) else {
                continue;
            };
            bins += 1;
            for (at, fields) in &patched.changed {
                let object = &patched.objects[*at];
                let place = Place {
                    wad: wad.clone(),
                    path: patch.target.clone(),
                    entry: object.path_hash,
                };
                for field in fields {
                    if let Some(value) = object.properties.get(field) {
                        typing.field((object.class_hash, *field), value, &place);
                    }
                }
            }
        }
        if !typing.wrong.is_empty() {
            let fields: Vec<TypeRow> = typing.wrong.into_values().collect();
            sink.raise(TypeMismatch {
                values: fields.iter().map(|row| row.values).sum(),
                fields,
            });
        }
        if !typing.undeclared.is_empty() {
            let fields: Vec<UndeclaredRow> = typing.undeclared.into_values().collect();
            sink.raise(FieldUndeclared {
                values: fields.iter().map(|row| row.values).sum(),
                fields,
            });
        }
        let against = match cx.types.schema().and(cx.types.build()) {
            Some(build) => Against::Schema { build },
            None => Against::Install,
        };
        sink.raise(Done(Coverage::Types {
            bins,
            fields: typing.fields,
            against,
        }));
    }
}

/// The walk of one package's values against the target's types.
struct Typing<'a> {
    types: &'a TargetTypes<'a>,
    /// How many fields were typed.
    fields: usize,
    /// The values of another type, by class, field and the two shapes, in the order found.
    wrong: IndexMap<(BinHash, BinHash, Shape, Shape), TypeRow>,
    /// The values on a field its class does not declare, by class and field.
    undeclared: IndexMap<(BinHash, BinHash), UndeclaredRow>,
}

impl Typing<'_> {
    /// Whether a value of the shape `found` reads as `want`.
    fn reads_as(&self, want: &FieldType, found: &FieldType, source: TypeSource) -> bool {
        let differs = |want: Option<_>, found: Option<_>| want.is_some() && want != found;
        if want.kind != found.kind || differs(want.key, found.key) || differs(want.item, found.item)
        {
            return false;
        }
        // A pointer or an embed of a derived class reads as its base. The install's bins
        // show one class a field held and not every class it may hold.
        match (want.class, found.class, source) {
            (Some(want), Some(found), TypeSource::Schema) if want != found => {
                let (Some(schema), Some(build)) = (self.types.schema(), self.types.build()) else {
                    return true;
                };
                schema.derives_from(found, want, build) || !schema.knows_class(found)
            }
            _ => true,
        }
    }

    fn object(&mut self, object: &BinObject, place: &Place) {
        self.structure(object.class_hash, &object.properties, place);
    }

    fn structure(&mut self, class: BinHash, fields: &Fields, place: &Place) {
        for (field, value) in fields {
            self.field((class, *field), value, place);
        }
    }

    fn field(
        &mut self,
        (class, field): (BinHash, BinHash),
        value: &PropertyValueEnum,
        place: &Place,
    ) {
        self.fields += 1;
        match self.types.type_of(class, field) {
            Some((want, source)) => {
                let found = FieldType::of(value);
                if !self.reads_as(&want, &found, source) {
                    let shapes = (Shape::of(&found), Shape::of(&want));
                    let row = self
                        .wrong
                        .entry((class, field, shapes.0, shapes.1))
                        .or_insert_with(|| TypeRow {
                            class,
                            field,
                            found: shapes.0,
                            wants: shapes.1,
                            values: 0,
                            first: place.clone(),
                        });
                    row.values += 1;
                    return;
                }
            }
            None => {
                let schema = self.types.schema().filter(|_| self.types.build().is_some());
                if schema.is_some_and(|schema| schema.knows_class(class)) {
                    let row =
                        self.undeclared
                            .entry((class, field))
                            .or_insert_with(|| UndeclaredRow {
                                class,
                                field,
                                values: 0,
                                first: place.clone(),
                            });
                    row.values += 1;
                    return;
                }
            }
        }
        self.value(value, place);
    }

    /// Goes on into the structs a value holds.
    fn value(&mut self, value: &PropertyValueEnum, place: &Place) {
        use PropertyValueEnum as V;
        match value {
            V::Embedded(embedded) => {
                self.structure(embedded.0.class_hash, &embedded.0.properties, place);
            }
            V::Struct(pointer) if pointer.class_hash.0 != 0 => {
                self.structure(pointer.class_hash, &pointer.properties, place);
            }
            V::Container(list) => list.items().iter().for_each(|v| self.value(v, place)),
            V::UnorderedContainer(list) => {
                list.0.items().iter().for_each(|v| self.value(v, place));
            }
            V::Optional(option) => {
                if let Some(inner) = option.value() {
                    self.value(inner, place);
                }
            }
            V::Map(map) => map.entries().iter().for_each(|(_, v)| self.value(v, place)),
            _ => {}
        }
    }
}
