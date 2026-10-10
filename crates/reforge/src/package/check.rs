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
use ltk_meta::property::values;
use ltk_meta::{Bin, BinObject, PropertyValueEnum};
use reforge_wwise::{Bank, BankKind, CLIENT_VERSION};
use serde::{Deserialize, Serialize};

use super::findings::{
    Against, ApplyFailed, ClassRow, ClassUndeclared, Coverage, Done, FieldUndeclared, FileMissing,
    HeldBy, LinkUnresolved, ManifestUnreadable, NameUnresolved, OtherSkin, OtherSkinName,
    OverlayCapped, Place, RecordSkipped, RoundTripFailed, RoundTripProblem, Shape, SkinValue,
    TypeMismatch, TypeRow, UndeclaredRow,
};
use super::overlay::{Loaded, MAX_OVERLAY_BINS, Outcome, Overlay};
use super::{Layer, Origin, Package, PatchOf, WrittenBin};
use crate::diag::{Cancelled, Record, Sink, Step, Steps};
use crate::fixups::{TargetTypes, TypeSource};
use crate::game::{FieldType, GameBin, GameSource};
use crate::model::entity::UnitId;
use crate::model::{
    BinHash, Fields, SkinEntry, WadHash, bin, name_hash, panic_text, path_hash, value,
};
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
    pub game: Option<&'a dyn GameSource>,
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
    pub fn of_port(
        mut self,
        game: &'a dyn GameSource,
        overlay: &'a Overlay,
        port: &'a Port,
    ) -> Self {
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
    vec![
        Arc::new(Types),
        Arc::new(RoundTrip),
        Arc::new(Apply),
        Arc::new(BaseSkin),
    ]
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

    /// How many checks the set holds.
    pub fn len(&self) -> usize {
        self.checks.len()
    }

    /// Whether the set holds no check.
    pub fn is_empty(&self) -> bool {
        self.checks.is_empty()
    }

    /// The stage `check`: runs each check on the package, each through a sink that names the
    /// check and the record it rests on. Each check is a step, and the run's cancel ends the
    /// stage between two (#ADR-0097).
    pub fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>) -> Result<(), Cancelled> {
        self.run_counted(cx, &mut Steps::new(self.len()), sink)
    }

    /// [`CheckSet::run`] in a stage whose steps before the checks `steps` counts.
    pub fn run_counted(
        &self,
        cx: &CheckCx<'_>,
        steps: &mut Steps,
        sink: &mut Sink<'_>,
    ) -> Result<(), Cancelled> {
        let _span = tracing::info_span!("check", checks = self.checks.len()).entered();
        for check in &self.checks {
            let info = check.info();
            steps.begin(sink, || Step::Check {
                check: info.id.to_string(),
            })?;
            check.run(cx, &mut sink.by(info.id.as_str(), info.record));
        }
        Ok(())
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
            if loaded.capped {
                sink.raise(OverlayCapped {
                    target: patch.target.clone(),
                    cap: MAX_OVERLAY_BINS,
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
                    Self::problems(&cx.package.bytes(chunk), written)
                }
                (Origin::Bank(_), None) => {
                    banks += 1;
                    Self::bank_problems(&cx.package.bytes(chunk))
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

/// A champion's base skin, patched, names no other skin of the champion in the six values a
/// check of the base skin for a premium skin put in its place reads (#ADR-0062).
#[derive(Debug, Clone, Copy, Default)]
pub struct BaseSkin;

static BASE_SKIN: CheckInfo = CheckInfo::built_in(
    "base-skin",
    "A champion's patched base skin names no other skin of the champion",
    None,
);

impl BaseSkin {
    /// The champion whose base skin a patch's target is: `data/characters/<c>/skins/skin0.bin`.
    fn champion(target: &str) -> Option<String> {
        let lower = target.to_ascii_lowercase();
        let rest = lower.strip_prefix("data/characters/")?;
        let champion = rest.strip_suffix("/skins/skin0.bin")?;
        (!champion.is_empty() && !champion.contains('/')).then(|| champion.to_string())
    }

    /// Whether `text` holds `marker`, then the number of a skin other than 0, then `suffix`,
    /// whatever the case. `marker` and `suffix` are lowercase.
    fn names_other_skin(text: &str, marker: &str, suffix: &str) -> bool {
        let text = text.to_ascii_lowercase();
        let mut rest = text.as_str();
        while let Some(at) = rest.find(marker) {
            let tail = &rest[at + marker.len()..];
            let digits = tail.len() - tail.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            let slot = tail[..digits].parse::<u32>();
            if slot.is_ok_and(|slot| slot != 0) && tail[digits..].starts_with(suffix) {
                return true;
            }
            rest = tail;
        }
        false
    }

    /// The values of a champion's skin entry that name another skin of the champion. A file the
    /// entry holds by its hash spells no path, and is not read.
    pub fn other_skin(entry: SkinEntry<'_>, champion: &str) -> Vec<OtherSkinName> {
        let name = format!("{champion}skin");
        let unit = format!("{champion}_skin");
        let folder = format!("{champion}/skins/skin");
        let mut out = Vec::new();
        let mut read = |value: SkinValue, text: &str, marker: &str, suffix: &str| {
            if Self::names_other_skin(text, marker, suffix) {
                out.push(OtherSkinName {
                    value,
                    names: text.to_string(),
                });
            }
        };
        if let Some(skin) = entry.skin_name() {
            read(SkinValue::SkinName, skin, &name, "");
        }
        let audio = entry.audio();
        for tag in &audio.tags {
            read(SkinValue::AudioTag, tag, &name, "");
        }
        for held in &audio.units {
            read(SkinValue::BankUnit, &held.name, &unit, "_sfx");
        }
        // A file the entry holds by its hash spells no path, and is not read.
        let files = [
            (SkinValue::Skeleton, entry.skeleton()),
            (SkinValue::SimpleSkin, entry.mesh()),
            (SkinValue::Texture, entry.texture()),
        ];
        for (value, file) in files {
            if let Some(path) = file.as_ref().and_then(value::FileRef::spelled) {
                read(value, path, &folder, "");
            }
        }
        out
    }
}

impl Check for BaseSkin {
    fn info(&self) -> &CheckInfo {
        &BASE_SKIN
    }

    fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>) {
        let Some(overlay) = cx.overlay else {
            return;
        };
        let mut entries = 0;
        // A voice layer's patch leaves the same entry with another audio block.
        let mut found: BTreeMap<UnitId, BTreeSet<OtherSkinName>> = BTreeMap::new();
        for applied in &overlay.patches {
            let Outcome::Applied(patched) = &applied.outcome else {
                continue;
            };
            let Some(patch) = cx.package.patches(&applied.layer).get(applied.index) else {
                continue;
            };
            let PatchOf::Skin { unit, entry } = patch.of else {
                continue;
            };
            let Some(champion) = Self::champion(patch.target.as_str()) else {
                continue;
            };
            // The base skin read is that of the champion the WAD is named after: no
            // sub-character's, in its own bin or in the champion's.
            let wad = applied
                .wad
                .as_ref()
                .map(|wad| wad.as_str().to_ascii_lowercase());
            let stem = wad.as_deref().and_then(|wad| wad.split('.').next());
            if stem != Some(champion.as_str())
                || entry != name_hash(&format!("Characters/{champion}/Skins/Skin0"))
            {
                continue;
            }
            let Some(object) = patched.objects.iter().find(|o| o.path_hash == entry) else {
                continue;
            };
            entries += 1;
            let values = Self::other_skin(SkinEntry::read(object), &champion);
            if !values.is_empty() {
                found.entry(unit).or_default().extend(values);
            }
        }
        for (unit, values) in found {
            let values = values.into_iter().collect();
            sink.raise(OtherSkin { unit, values });
        }
        sink.raise(Done(Coverage::BaseSkin { entries }));
    }
}

/// Every value of every bin the package ships is of the type the target build reads there,
/// on a field its class declares, and every struct and object is of a class the build
/// declares. The types are the schema's at the install's build, and for a field the schema
/// does not describe what the install's bins hold. Without a schema nothing is said of
/// undeclared fields and classes.
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
            classes: IndexMap::new(),
        };
        let mut bins = 0;
        for (_, key, chunk) in cx.package.bins() {
            // A bin that does not read is `load`'s finding, and the client reads none of it.
            let Ok(parsed) = bin::parse(&cx.package.bytes(chunk)) else {
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
        if !typing.classes.is_empty() {
            let classes: Vec<ClassRow> = typing.classes.into_values().collect();
            // The finding rests on a record of its own, and not on the check's.
            sink.by(TYPES.id.as_str(), None).raise(ClassUndeclared {
                structs: classes.iter().map(|row| row.structs).sum(),
                classes,
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
    /// The structs and objects of a class the target build does not declare, by what holds
    /// them and the class, in the order found.
    classes: IndexMap<(Option<HeldBy>, BinHash), ClassRow>,
}

/// How a field takes a struct of a class.
#[derive(Clone, Copy)]
enum Taken {
    /// The field's class, one that derives from it, or one whose bases are not known.
    Yes,
    /// A declared class that does not derive from the field's.
    Other,
    /// A class the target build does not declare.
    Undeclared,
}

/// A field whose value reads as the type the target build reads there, and that type: what
/// the class of each struct the value holds is judged against.
#[derive(Clone, Copy)]
struct Holder {
    class: BinHash,
    field: BinHash,
    want: FieldType,
    found: FieldType,
    source: TypeSource,
}

/// Whether a value of the shape `found` reads as `want` by its kinds. The class of each
/// struct it holds is judged apart ([`Typing::takes`]): [`FieldType::of`] gives the class of
/// a collection's first struct only.
fn reads_as(want: &FieldType, found: &FieldType) -> bool {
    let differs = |want: Option<_>, found: Option<_>| want.is_some() && want != found;
    want.kind == found.kind && !differs(want.key, found.key) && !differs(want.item, found.item)
}

impl Typing<'_> {
    /// How a field that reads structs of the class `want` takes one of the class `found`.
    fn takes(&self, want: Option<BinHash>, found: BinHash, source: TypeSource) -> Taken {
        // The install's bins show one class a field held, and not every class it takes, so
        // against them any class passes, one the schema lacks too. Against the schema, a class
        // the target build does not declare is found apart.
        if want == Some(found) || source != TypeSource::Schema {
            return Taken::Yes;
        }
        let (Some(schema), Some(build)) = (self.types.schema(), self.types.build()) else {
            return Taken::Yes;
        };
        if !schema.has_class_at(found, build) {
            // A class the install's bins hold is the build's, and its bases are not known.
            return match self.types.declares_class(found) {
                Some(true) => Taken::Yes,
                _ => Taken::Undeclared,
            };
        }
        // A pointer or an embed of a derived class reads as its base.
        match want {
            Some(want) if !schema.derives_from(found, want, build) => Taken::Other,
            _ => Taken::Yes,
        }
    }

    /// Counts one struct or object of a class the target build does not declare.
    fn class_undeclared(&mut self, held_by: Option<HeldBy>, held: BinHash, place: &Place) {
        let row = self
            .classes
            .entry((held_by, held))
            .or_insert_with(|| ClassRow {
                held_by,
                held,
                structs: 0,
                first: place.clone(),
            });
        row.structs += 1;
    }

    /// Counts one value of a field written in the shape `found` where the build reads `wants`.
    fn mistyped(
        &mut self,
        (class, field): (BinHash, BinHash),
        (found, wants): (Shape, Shape),
        place: &Place,
    ) {
        let row = self
            .wrong
            .entry((class, field, found, wants))
            .or_insert_with(|| TypeRow {
                class,
                field,
                found,
                wants,
                values: 0,
                first: place.clone(),
            });
        row.values += 1;
    }

    fn object(&mut self, object: &BinObject, place: &Place) {
        if self.types.declares_class(object.class_hash) == Some(false) {
            self.class_undeclared(None, object.class_hash, place);
            return;
        }
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
        let mut holder = None;
        match self.types.type_of(class, field) {
            Some((want, source)) => {
                let found = FieldType::of(value);
                if !reads_as(&want, &found) {
                    let shapes = (Shape::of(&found), Shape::of(&want));
                    self.mistyped((class, field), shapes, place);
                    return;
                }
                holder = Some(Holder {
                    class,
                    field,
                    want,
                    found,
                    source,
                });
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
        self.value(value, holder, place);
    }

    /// Goes on into the structs a value holds, each judged against the class its field reads.
    fn value(&mut self, value: &PropertyValueEnum, holder: Option<Holder>, place: &Place) {
        use PropertyValueEnum as V;
        match value {
            V::Embedded(embedded) => self.held(&embedded.0, holder, place),
            V::Struct(pointer) if pointer.class_hash.0 != 0 => self.held(pointer, holder, place),
            V::Container(list) => {
                list.items()
                    .iter()
                    .for_each(|v| self.value(v, holder, place));
            }
            V::UnorderedContainer(list) => {
                list.0
                    .items()
                    .iter()
                    .for_each(|v| self.value(v, holder, place));
            }
            V::Optional(option) => {
                if let Some(inner) = option.value() {
                    self.value(inner, holder, place);
                }
            }
            V::Map(map) => {
                map.entries()
                    .iter()
                    .for_each(|(_, v)| self.value(v, holder, place));
            }
            _ => {}
        }
    }

    /// Types the fields of a struct its field takes. A struct of a class the field does not
    /// take is one value of another type, one of a class the build does not declare is counted
    /// apart, and the fields of neither are typed.
    fn held(&mut self, held: &values::Struct, holder: Option<Holder>, place: &Place) {
        let Some(holder) = holder else {
            self.structure(held.class_hash, &held.properties, place);
            return;
        };
        match self.takes(holder.want.class, held.class_hash, holder.source) {
            Taken::Yes => self.structure(held.class_hash, &held.properties, place),
            Taken::Other => {
                let found = FieldType {
                    class: Some(held.class_hash),
                    ..holder.found
                };
                let shapes = (Shape::of(&found), Shape::of(&holder.want));
                self.mistyped((holder.class, holder.field), shapes, place);
            }
            Taken::Undeclared => {
                let by = HeldBy {
                    class: holder.class,
                    field: holder.field,
                };
                self.class_undeclared(Some(by), held.class_hash, place);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_another_skins_by_a_number_other_than_zero_after_the_marker() {
        let names = BaseSkin::names_other_skin;
        assert!(names("AatroxSkin07", "aatroxskin", ""));
        assert!(names("AatroxSkin123", "aatroxskin", ""));
        assert!(names("Aatrox_Skin07_SFX", "aatrox_skin", "_sfx"));
        assert!(!names("Aatrox_Skin07_VO", "aatrox_skin", "_sfx"));
        assert!(!names("AatroxSkin0", "aatroxskin", ""));
        assert!(!names("AatroxSkin99999999999999999999", "aatroxskin", ""));
        let base = "ASSETS/Characters/Aatrox/Skins/Base/Aatrox.skn";
        assert!(!names(base, "aatrox/skins/skin", ""));
        // The first number is the base's, the second is not.
        let both = "ASSETS/Characters/Aatrox/Skins/Skin0/Aatrox/Skins/Skin12.skn";
        assert!(names(both, "aatrox/skins/skin", ""));
        // Another champion's skin, and a sub-character's.
        let other = "ASSETS/Characters/Ahri/Skins/Skin07/Ahri_Skin07.skn";
        assert!(!names(other, "aatrox/skins/skin", ""));
        let sub = "ASSETS/Characters/AatroxPet/Skins/Skin07/AatroxPet.skn";
        assert!(!names(sub, "aatrox/skins/skin", ""));
    }

    #[test]
    fn the_base_skin_read_is_the_champions_own_bin() {
        let champion = BaseSkin::champion;
        let base = "DATA/Characters/Aatrox/Skins/Skin0.bin";
        assert_eq!(champion(base).as_deref(), Some("aatrox"));
        assert_eq!(champion("data/characters/aatrox/skins/skin7.bin"), None);
        assert_eq!(champion("data/characters/aatrox/aatrox.bin"), None);
    }
}
