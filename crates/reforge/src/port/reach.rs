//! What each unit's skin entry reaches, and how each name it reaches resolves once the port
//! is loaded at the unit's target.
//!
//! The walk follows object links from the entry through the unit's load. A name the load does
//! not hold resolves to the game's objects at the target: the bins of the game's skin there,
//! the bin of shader definitions, and the game bins the mod's load links (#GAME-BEHAVIOR-0006).
//! Anything else is dangling.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt::Write as _;

use serde::Serialize;

use super::base::{Base, BaseObjects};
use super::findings::{GameCarried, LinkDangling, Unreached, hashes_hex};
use super::plan::{Objects, PlanError, PortInputs};
use crate::diag::Sink;
use crate::load::provenance::{AtPath, BytesOrigin};
use crate::model::entity::{CharId, LinkTo, Mod, ObjectId, UnitId};
use crate::model::hash::skin_paths;
use crate::model::known::class;
use crate::model::{
    BinHash, ChunkPath, Dump, DumpCx, SkinEntry, Slot, WadHash, clean, name_hash, path_hash, value,
};

/// The bin of shader definitions, which the client loads whatever it plays.
pub const SHADERS_BIN: &str = "data/shaders/shaders.bin";

/// What becomes of an object the entry reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fate {
    /// The unit's skin entry: its carried fields patch the game's entry at the target.
    Entry,
    /// The resolver the entry links: its map is laid over the game's resolver.
    Resolver,
    /// The animation graph the entry links.
    Graph,
    /// Byte for byte the object the game loads at the target under this name: named, and not
    /// carried.
    Game,
    /// Carried into the port under a name of its own.
    Carried,
}

impl Fate {
    /// The fate's name in a dump.
    pub fn as_str(self) -> &'static str {
        match self {
            Fate::Entry => "entry",
            Fate::Resolver => "resolver",
            Fate::Graph => "graph",
            Fate::Game => "game",
            Fate::Carried => "carried",
        }
    }
}

/// What one unit's entry reaches at its target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnitReach {
    /// The target.
    pub target: Slot,
    /// The mod's objects the entry reaches, breadth first, each with its fate.
    pub objects: Vec<(ObjectId, Fate)>,
    /// Names reached that resolve to the game's objects at the target.
    #[serde(serialize_with = "hashes_hex")]
    pub game: BTreeSet<BinHash>,
    /// A sub-character's names that resolve to its champion's objects, which the champion's
    /// unit carries.
    #[serde(serialize_with = "hashes_hex")]
    pub champion: BTreeSet<BinHash>,
    /// Names reached that nothing loaded at the target holds.
    #[serde(serialize_with = "hashes_hex")]
    pub dangling: BTreeSet<BinHash>,
    /// The game bins the port keeps as links: the ones the mod's load links, and a bin of the
    /// source slot that holds a name the target's bins lack.
    pub game_links: Vec<ChunkPath>,
    /// Objects of the load that nothing reaches.
    pub leftovers: Vec<ObjectId>,
    /// How many of the game's objects are carried because they name a file the mod overrides.
    pub carried_for_overrides: usize,
}

impl UnitReach {
    /// The fate of an object the entry reaches.
    pub fn fate(&self, object: ObjectId) -> Option<Fate> {
        let found = self.objects.iter().find(|(held, _)| *held == object);
        found.map(|(_, fate)| *fate)
    }

    /// The objects of one fate, in the order reached.
    pub fn with(&self, fate: Fate) -> impl Iterator<Item = ObjectId> + '_ {
        let of_fate = self.objects.iter().filter(move |(_, held)| *held == fate);
        of_fate.map(|(object, _)| *object)
    }

    /// The objects the port answers for: every one but the game's own.
    pub fn ported(&self) -> impl Iterator<Item = (ObjectId, Fate)> + '_ {
        let ported = self.objects.iter().filter(|(_, fate)| *fate != Fate::Game);
        ported.copied()
    }
}

/// What each unit with a target and an entry reaches.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Reach {
    /// By unit.
    pub units: BTreeMap<UnitId, UnitReach>,
}

/// Every object the client loads for a unit, by the entry hash it has in `objects`: the first
/// under each, in load order.
pub(super) fn pool(m: &Mod, objects: &Objects<'_>, unit: UnitId) -> (Vec<ObjectId>, Pool) {
    let mut by_entry = HashMap::new();
    let mut order = Vec::new();
    let loaded = m.units[unit].loads.iter();
    for &object in loaded.flat_map(|&bin| &m.bins[bin].objects) {
        if let std::collections::hash_map::Entry::Vacant(free) =
            by_entry.entry(objects.entry(object))
        {
            free.insert(object);
            order.push(object);
        }
    }
    (order, Pool { by_entry })
}

/// The objects of a load by entry hash.
#[derive(Default)]
pub(super) struct Pool {
    by_entry: HashMap<BinHash, ObjectId>,
}

impl Pool {
    pub(super) fn get(&self, entry: BinHash) -> Option<ObjectId> {
        self.by_entry.get(&entry).copied()
    }

    fn extend(&mut self, other: Pool) {
        for (entry, object) in other.by_entry {
            self.by_entry.entry(entry).or_insert(object);
        }
    }
}

/// The walk of one mod's units.
struct Walk<'a, 'b> {
    inputs: &'a PortInputs<'a>,
    objects: &'a Objects<'b>,
    base: Base<'a>,
    /// The paths of the files the mod overrides at the game's own paths.
    overridden: HashSet<WadHash>,
}

impl Walk<'_, '_> {
    /// The bins of the game's skin of a character at a slot.
    fn chain(&self, character: CharId, slot: Slot) -> Option<Vec<WadHash>> {
        let catalog = self.inputs.facts.catalogs.of(character)?;
        catalog.skin(slot).map(|skin| skin.chain.clone())
    }

    fn chain_or_bin(&self, character: CharId, slot: Slot) -> Vec<WadHash> {
        let name = &self.inputs.fixed.characters[character].name;
        let bin = || vec![path_hash(&skin_paths::skin_bin(name, slot))];
        self.chain(character, slot).unwrap_or_else(bin)
    }

    /// The game's bins at the source slot that the target does not load, each with the names
    /// it holds, for a name the game loads at the source and not at the target.
    fn source_bins(
        &self,
        unit: UnitId,
        t: Slot,
        loaded: &[WadHash],
    ) -> Result<Vec<(ChunkPath, BaseObjects)>, PlanError> {
        let m: &Mod = self.inputs.fixed;
        let node = &m.units[unit];
        if node.source_slot == t {
            return Ok(Vec::new());
        }
        let champion = m.champion_of(node.character);
        let mut chains = self
            .chain(node.character, node.source_slot)
            .unwrap_or_default();
        if champion != node.character {
            chains.extend(self.chain(champion, node.source_slot).unwrap_or_default());
        }
        let mut out = Vec::new();
        for path in chains.into_iter().filter(|path| !loaded.contains(path)) {
            let spelled = crate::model::Spell::path(self.inputs.names, path);
            let Some(spelled) = spelled.and_then(|path| ChunkPath::parse(&path).ok()) else {
                continue;
            };
            out.push((spelled, self.base.loaded(&[path])?));
        }
        Ok(out)
    }

    fn unit(&self, unit: UnitId, t: Slot, done: &Reach) -> Result<Option<UnitReach>, PlanError> {
        let m: &Mod = self.inputs.fixed;
        let node = &m.units[unit];
        let Some(entry) = node.entry else {
            return Ok(None);
        };
        let targets = self.inputs.targets;
        let copies = &self.inputs.facts.copies;

        // What the game loads at the target: its skin's bins and the shaders. A sub-character
        // is loaded beside its champion, whose bins and whose unit at the slot count too.
        let mut paths = self.chain_or_bin(node.character, t);
        paths.push(path_hash(SHADERS_BIN));
        let champion = m.champion_of(node.character);
        let champion_units: Vec<UnitId> = if champion == node.character {
            Vec::new()
        } else {
            paths.extend(self.chain_or_bin(champion, t));
            let units = m.characters[champion].units.iter().copied();
            units
                .filter(|&cu| copies.is_representative(cu) && targets.target(cu) == Some(t))
                .collect()
        };
        let mut champion_pool = Pool::default();
        for &cu in &champion_units {
            champion_pool.extend(pool(m, self.objects, cu).1);
        }
        let mut game_links: Vec<ChunkPath> = Vec::new();
        let mut linked: Vec<WadHash> = Vec::new();
        for link in node.loads.iter().flat_map(|&bin| &m.bins[bin].links) {
            let hash = path_hash(link.path.raw());
            if link.to != LinkTo::Game || linked.contains(&hash) {
                continue;
            }
            linked.push(hash);
            paths.push(hash);
            if let Ok(path) = ChunkPath::parse(link.path.raw()) {
                game_links.push(path);
            }
        }
        let game = self.base.loaded(&paths)?;
        let source_bins = self.source_bins(unit, t, &paths)?;

        let (order, own) = pool(m, self.objects, unit);
        let entry_value = SkinEntry::read(self.objects.value(entry));
        let (linked, graph) = (entry_value.resolver(), entry_value.graph());
        // #ADR-0056: a link nothing loads at the target gives way to the resolver under the
        // slot's own name.
        let resolves = |name: BinHash| {
            own.get(name).is_some()
                || champion_pool.get(name).is_some()
                || game.holds(name)
                || source_bins.iter().any(|(_, held)| held.holds(name))
        };
        let own_name = name_hash(&skin_paths::resolver_entry(
            &m.characters[node.character].spelled,
            t,
        ));
        let is_resolver = |object: ObjectId| {
            self.objects.value(object).class_hash == name_hash(class::RESOURCE_RESOLVER)
        };
        let stale_link =
            linked.filter(|&name| !resolves(name) && own.get(own_name).is_some_and(is_resolver));
        let resolver = if stale_link.is_some() {
            Some(own_name)
        } else {
            linked
        };

        let mut reach = UnitReach {
            target: t,
            objects: Vec::new(),
            game: BTreeSet::new(),
            champion: BTreeSet::new(),
            dangling: BTreeSet::new(),
            game_links,
            leftovers: Vec::new(),
            carried_for_overrides: 0,
        };
        let mut seen: BTreeSet<BinHash> = stale_link.into_iter().collect();
        // Each name with the object that reached it, and whether through the game's objects
        // only: those are followed for overridden files, and are no one's loose end.
        let mut queue: VecDeque<(BinHash, Option<ObjectId>, bool)> =
            VecDeque::from([(self.objects.entry(entry), None, false)]);
        if stale_link.is_some() {
            queue.push_back((own_name, Some(entry), false));
        }
        let mut parents: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
        let mut adopted: HashMap<BinHash, ObjectId> = HashMap::new();
        while let Some((name, parent, under_game)) = queue.pop_front() {
            // A name of the champion's that the champion's own unit at the target does not
            // reach is carried by no one: the sub-character follows it as its own.
            let found = own.get(name).or_else(|| {
                let object = champion_pool.get(name)?;
                let covered = champion_units.iter().any(|cu| {
                    let reached = done.units.get(cu);
                    reached.is_some_and(|reached| reached.fate(object).is_some())
                });
                (!covered).then(|| {
                    adopted.insert(name, object);
                    object
                })
            });
            if let (Some(object), Some(parent)) = (found, parent) {
                parents.entry(object).or_default().push(parent);
            }
            if !seen.insert(name) || (under_game && found.is_none()) {
                continue;
            }
            let Some(object) = found else {
                if champion_pool.get(name).is_some() {
                    reach.champion.insert(name);
                } else if game.holds(name) {
                    reach.game.insert(name);
                } else if let Some((bin, _)) = source_bins.iter().find(|(_, held)| held.holds(name))
                {
                    if !reach.game_links.contains(bin) {
                        reach.game_links.push(bin.clone());
                    }
                    reach.game.insert(name);
                } else {
                    reach.dangling.insert(name);
                }
                continue;
            };
            let fate = if object == entry {
                Fate::Entry
            } else if Some(name) == resolver {
                Fate::Resolver
            } else if Some(name) == graph {
                Fate::Graph
            } else if game.object(name).map(|held| held.raw) == Some(m.objects[object].raw) {
                Fate::Game
            } else {
                Fate::Carried
            };
            reach.objects.push((object, fate));
            let links = value::links(self.objects.value(object)).into_iter();
            queue.extend(links.map(|link| (link, Some(object), fate == Fate::Game)));
        }

        // A game object naming an overridden file is carried, and so is every game object on
        // the way to it: the file ships under the namespace, and the links to it follow.
        let names_override = |object: ObjectId| {
            let files = value::file_refs(self.objects.value(object));
            files.iter().any(|f| self.overridden.contains(&f.hash()))
        };
        let mut carry: Vec<ObjectId> = reach
            .objects
            .iter()
            .filter(|(object, fate)| *fate == Fate::Game && names_override(*object))
            .map(|(object, _)| *object)
            .collect();
        while let Some(object) = carry.pop() {
            let Some(slot) = reach.objects.iter_mut().find(|(held, _)| *held == object) else {
                continue;
            };
            if slot.1 != Fate::Game {
                continue;
            }
            slot.1 = Fate::Carried;
            reach.carried_for_overrides += 1;
            carry.extend(parents.get(&object).into_iter().flatten().copied());
        }
        // What stayed the game's and was reached only through the game's objects is not the
        // mod's.
        let fates: HashMap<ObjectId, Fate> = reach.objects.iter().copied().collect();
        let mut kept: BTreeSet<ObjectId> = BTreeSet::new();
        let mut stack = vec![entry];
        stack.extend(stale_link.and_then(|_| own.get(own_name)));
        while let Some(object) = stack.pop() {
            if !kept.insert(object) || fates.get(&object) == Some(&Fate::Game) {
                continue;
            }
            for link in value::links(self.objects.value(object)) {
                if let Some(child) = own.get(link).or_else(|| adopted.get(&link).copied()) {
                    stack.push(child);
                }
            }
        }
        reach.objects.retain(|(object, _)| kept.contains(object));
        let reached: BTreeSet<ObjectId> = reach.objects.iter().map(|(o, _)| *o).collect();
        reach.leftovers = order
            .into_iter()
            .filter(|object| !reached.contains(object))
            .collect();
        Ok(Some(reach))
    }
}

impl Reach {
    /// What each representative unit with a target and an entry reaches, the mod's objects
    /// read as `objects` holds them.
    pub fn find(inputs: &PortInputs<'_>, objects: &Objects<'_>) -> Result<Self, PlanError> {
        let m: &Mod = inputs.fixed;
        let kept_stale = |file: &crate::model::entity::FileNode, origin: &BytesOrigin| {
            let at_own_path =
                matches!(origin, BytesOrigin::History { path, .. } if *path == file.hash);
            inputs.options.keep_stale && at_own_path
        };
        let overridden = inputs.files.iter().filter(|(file, fact)| {
            let origin = &fact.origin;
            fact.at_path == AtPath::OtherBytes
                && (*origin == BytesOrigin::Own || kept_stale(&m.files[*file], origin))
        });
        let walk = Walk {
            inputs,
            objects,
            base: Base::at_target(inputs.env),
            overridden: overridden.map(|(file, _)| m.files[file].hash).collect(),
        };
        let mut out = Self::default();
        for unit in inputs.facts.copies.representatives() {
            let Some(t) = inputs.targets.target(unit) else {
                continue;
            };
            if let Some(reach) = walk.unit(unit, t, &out)? {
                out.units.insert(unit, reach);
            }
        }
        Ok(out)
    }

    /// What a unit reaches.
    pub fn of(&self, unit: UnitId) -> Option<&UnitReach> {
        self.units.get(&unit)
    }

    /// Raises what the walk found, unit by unit.
    pub fn report(&self, sink: &mut Sink<'_>) {
        for (&unit, reach) in &self.units {
            if reach.carried_for_overrides > 0 {
                sink.raise(GameCarried {
                    unit,
                    objects: reach.carried_for_overrides,
                });
            }
            if !reach.dangling.is_empty() {
                sink.raise(LinkDangling {
                    unit,
                    slot: reach.target,
                    names: reach.dangling.iter().copied().collect(),
                });
            }
            if !reach.leftovers.is_empty() {
                sink.raise(Unreached {
                    unit,
                    objects: reach.leftovers.len(),
                });
            }
        }
    }
}

impl Dump for Reach {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = String::new();
        for (unit, reach) in &self.units {
            let name = cx.module.map_or(String::new(), |m| {
                format!(" {}", clean(&m.unit_name(*unit)))
            });
            let count = |fate: Fate| reach.with(fate).count();
            let _ = writeln!(
                out,
                "reach {unit}{name} -> skin{}: carried {}, the game's {}, game names {}, the champion's {}, dangling {}, leftovers {}",
                reach.target,
                count(Fate::Carried),
                count(Fate::Game),
                reach.game.len(),
                reach.champion.len(),
                reach.dangling.len(),
                reach.leftovers.len()
            );
            for link in &reach.game_links {
                let _ = writeln!(out, "   keeps link {link}");
            }
        }
        out
    }
}
