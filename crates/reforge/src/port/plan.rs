//! The stage `plan`: a fixed mod and its targets as a [`Port`]. Each step is a type with one
//! job, and a step reads only the steps before it:
//!
//! 1. `Shipping`: the units that ship, and why each other unit leaves;
//! 2. `Renames`: the names of each shipping unit moved from its slot to its target;
//!    `AudioPlan`: the bank units and banks each unit adds after the game's;
//! 3. `Reach` and `FileRefs`: what each entry reaches at its target, and where each file is,
//!    the literal sound names of what it reaches rewritten;
//! 4. `Layout`: the name of each carried object and the place of each file;
//! 5. `Port`: the objects that ship, cloned, with the renames and the places applied.
//!
//! The mod's objects stay as the fixups left them. The port is linked against the game as a
//! linker would: the skin entry, the resolver and a graph equal to the game's keep the names
//! the game finds them by, and every other carried object takes a name of its own under the
//! namespace, which every link to it follows.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use ltk_meta::{BinObject, PropertyValueEnum};
use serde::{Deserialize, Serialize};

use super::audio::{AudioPlan, OwnClips};
use super::base::Base;
use super::decide::Targets;
use super::findings::Carried;
use super::hud::HudPlan;
use super::layout::Layout;
use super::reach::{Fate, Reach};
use super::refs::{FileRefs, FileVerdict};
use super::renames::{Group, Renames};
use super::shipping::Shipping;
use crate::diag::{Cancelled, Sink};
use crate::env::Env;
use crate::facts::Facts;
use crate::fixups::FixedMod;
use crate::game::{GameError, Speller};
use crate::history::HistoryError;
use crate::load::{Fault, FileProvenance};
use crate::model::entity::{CharId, FileId, Mod, ObjectId, UnitId};
use crate::model::known::{class, field};
use crate::model::{
    BinHash, ChunkPath, Dump, DumpCx, Slot, Spell, WadHash, clean, name_hash, value,
};

/// What `plan` is configured with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortOptions {
    /// Stale copies were loaded as the mod's own files, and a port ships them.
    pub keep_stale: bool,
    /// A bank's events are cut from the game's events bank only: a bank the author added,
    /// which no event of the game's plays, is reported and not carried (#ADR-0030).
    pub game_events_only: bool,
    /// The units at slots other than their champion's target ship too, each at its own slot,
    /// for a mod that works on several skins (#ADR-0082).
    pub keep_other_slots: bool,
}

/// An input of the run that did not answer while the port was planned: a fault of the machine
/// or the tool, and no diagnostic.
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    /// A chunk the game holds did not read.
    #[error("the game did not read")]
    Game(#[from] GameError),
    /// The history's index did not answer a question.
    #[error("the history did not answer")]
    History(#[from] HistoryError),
    /// The run's cancel ended the stage between two of its steps (#ADR-0097).
    #[error(transparent)]
    Cancelled(#[from] Cancelled),
}

impl From<Fault> for PlanError {
    fn from(fault: Fault) -> Self {
        match fault {
            Fault::History(error) => PlanError::History(error),
            Fault::Game(error) => PlanError::Game(error),
        }
    }
}

/// What `plan` reads.
#[derive(Clone, Copy)]
pub struct PortInputs<'a> {
    /// The mod as the fixups left it.
    pub fixed: &'a FixedMod,
    /// What is true of it.
    pub facts: &'a Facts,
    /// Where each unit goes.
    pub targets: &'a Targets,
    /// The game and the history.
    pub env: &'a Env,
    /// Whose bytes the mod's files are.
    pub files: &'a FileProvenance,
    /// The names of the run.
    pub names: &'a Speller<'a>,
    /// The options.
    pub options: &'a PortOptions,
}

/// The mod's objects as a step of the plan reads them: the clone a step changed, else the
/// mod's own.
pub struct Objects<'a> {
    m: &'a Mod,
    changed: BTreeMap<ObjectId, BinObject>,
}

impl<'a> Objects<'a> {
    /// The objects of a mod, none changed.
    pub fn of(m: &'a Mod) -> Self {
        Self {
            m,
            changed: BTreeMap::new(),
        }
    }

    /// An object's value.
    pub fn value(&self, object: ObjectId) -> &BinObject {
        let changed = self.changed.get(&object);
        changed.unwrap_or(&self.m.objects[object].value)
    }

    /// An object's entry hash.
    pub fn entry(&self, object: ObjectId) -> BinHash {
        self.value(object).path_hash
    }

    /// Takes a changed clone of an object.
    pub fn set(&mut self, object: ObjectId, value: BinObject) {
        self.changed.insert(object, value);
    }

    /// A clone of an object to change.
    fn edit(&mut self, object: ObjectId) -> &mut BinObject {
        let m = self.m;
        let own = || m.objects[object].value.clone();
        self.changed.entry(object).or_insert_with(own)
    }
}

/// One unit of a port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortUnit {
    /// The unit.
    pub unit: UnitId,
    /// Its character.
    pub character: CharId,
    /// The champion its character loads with.
    pub champion: CharId,
    /// Its target.
    pub target: Slot,
    /// Its skin entry.
    pub entry: ObjectId,
    /// The resolver its entry links.
    pub resolver: Option<ObjectId>,
    /// Its graph, when it ships: it differs from the game's graph at the target.
    pub graph: Option<ObjectId>,
    /// The objects it carries, in the order reached.
    pub carried: Vec<ObjectId>,
    /// The game objects of other skins it copies, each as it ships: under a name of the
    /// target's, its links renamed (#ADR-0083).
    #[serde(skip)]
    pub copies: Vec<BinObject>,
    /// The game bins it keeps as links.
    pub game_links: Vec<ChunkPath>,
    /// Its directory.
    pub dir: String,
}

/// The stage `plan`'s result: the units that ship, renamed and placed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Port {
    /// The options it was planned under.
    pub options: PortOptions,
    /// The units that ship and the ones that leave.
    pub shipping: Shipping,
    /// What each unit's entry reaches at its target.
    pub reach: Reach,
    /// Where each file a unit names is found.
    pub refs: FileRefs,
    /// The names and paths made.
    pub layout: Layout,
    /// The renames.
    pub renames: Renames,
    /// The units, in unit order.
    pub units: Vec<PortUnit>,
    /// The objects the port answers for, each as it ships: renamed, its files where the port
    /// puts them.
    #[serde(skip)]
    pub objects: BTreeMap<ObjectId, BinObject>,
    /// The files that ship in the base layer, each with its place.
    pub files: BTreeMap<FileId, ChunkPath>,
    /// How many references were pointed at a file of the game's.
    pub repointed: usize,
    /// The audio.
    pub audio: AudioPlan,
    /// The spell icons and HUD textures.
    pub hud: HudPlan,
}

/// The placing of every unit's objects and files.
struct Linking<'a, 'b> {
    inputs: &'a PortInputs<'a>,
    objects: Objects<'b>,
    layout: Layout,
    renames: Renames,
    files: BTreeMap<FileId, ChunkPath>,
    repointed: usize,
    units: Vec<PortUnit>,
    /// The objects each unit answers for, with the group whose renames they follow.
    by_group: Vec<(Vec<ObjectId>, Group)>,
    /// The names copied so far, by group: a group ships one copy of a name.
    copied: HashSet<(Group, BinHash)>,
}

impl Linking<'_, '_> {
    /// The spelled keys of a unit's resolver, by the carried object each names: the names
    /// the game plays effects by.
    fn resolver_keys(
        &self,
        resolver: Option<ObjectId>,
        carried: &[ObjectId],
    ) -> HashMap<ObjectId, String> {
        let mut keys = HashMap::new();
        let map = resolver.and_then(|r| value::get(self.objects.value(r), &[field::RESOURCE_MAP]));
        let Some(PropertyValueEnum::Map(map)) = map else {
            return keys;
        };
        for (key, target) in map.entries() {
            let (PropertyValueEnum::Hash(key), Some(target)) = (key, value::as_link(target)) else {
                continue;
            };
            let spelled = self.inputs.names.hash(key.value);
            let Some(spelled) = spelled.or_else(|| self.inputs.names.entry(key.value)) else {
                continue;
            };
            let named = carried.iter().find(|&&o| self.objects.entry(o) == target);
            if let Some(&object) = named {
                keys.entry(object).or_insert(spelled);
            }
        }
        keys
    }

    fn unit(&mut self, unit: UnitId, reach: &Reach, refs: &FileRefs) {
        let inputs = self.inputs;
        let m: &Mod = inputs.fixed;
        let node = &m.units[unit];
        let (Some(reached), Some(entry)) = (reach.of(unit), node.entry) else {
            return;
        };
        let t = reached.target;
        let character = &m.characters[node.character];
        let champion = m.champion_of(node.character);
        let group: Group = (champion, t);
        let dir = self.layout.unit_dir(&character.name, t);

        // The graph ships unless it is the game's graph at the target, value for value.
        let base = Base::at_target(inputs.env);
        let catalog = inputs.facts.catalogs.of(node.character);
        let game_graph = catalog.and_then(|catalog| {
            let graph = catalog.skin(t)?.graph?;
            base.of_skins(catalog, graph)
        });
        let graph = reached.with(Fate::Graph).next().filter(|&graph| {
            let ours = self.objects.value(graph);
            game_graph.is_none_or(|theirs| {
                theirs.class_hash != ours.class_hash || theirs.properties != ours.properties
            })
        });
        let carried: Vec<ObjectId> = reached.with(Fate::Carried).collect();
        let resolver = reached.with(Fate::Resolver).next();
        let keys = self.resolver_keys(resolver, &carried);

        // A name of its own for everything carried, the graph included.
        let mut leaves: HashMap<ObjectId, String> = HashMap::new();
        for &object in graph.iter().chain(&carried) {
            let held = self.objects.value(object);
            let (old, class) = (held.path_hash, held.class_hash);
            let leaf = Layout::leaf(inputs.names, held);
            let new = match self.renames.names.get(&object) {
                Some(&new) => new,
                None => {
                    let new = match Some(object) == graph {
                        true => self.layout.name_graph(&character.spelled, t),
                        false => self.layout.name_object(&character.spelled, t, class, &leaf),
                    };
                    self.renames.names.insert(object, new);
                    new
                }
            };
            leaves.insert(object, leaf);
            self.renames
                .namespaced
                .entry(group)
                .or_default()
                .push((old, new));
        }
        // A copy of another skin's object takes a name of the target's, as a carried object
        // does, and every link to it follows (#ADR-0083). The group copies a name once: the
        // first unit ships the object, and the links of the others follow its name.
        let mut copies: Vec<(BinObject, String)> = Vec::new();
        for copied in &reached.copies {
            let old = copied.name.name;
            if !self.copied.insert((group, old)) {
                continue;
            }
            let mut object = copied.object.clone();
            let leaf = Layout::leaf(inputs.names, &object);
            let new = self
                .layout
                .name_object(&character.spelled, t, object.class_hash, &leaf);
            self.renames
                .namespaced
                .entry(group)
                .or_default()
                .push((old, new));
            object.path_hash = new;
            copies.push((object, leaf));
        }
        let ported: Vec<(ObjectId, Fate)> = reached.ported().collect();
        self.by_group
            .push((ported.iter().map(|(o, _)| *o).collect(), group));

        // Each file a ported value or a copy names, with the folder of its first user. The
        // entry comes first, so the skin's own textures stay the skin's.
        let mut named: HashSet<WadHash> = HashSet::new();
        let mut owner: HashMap<WadHash, String> = HashMap::new();
        let mut name_files = |held: &BinObject, folder: Option<String>| {
            for file in value::file_refs(held) {
                if named.insert(file.hash())
                    && let Some(folder) = &folder
                {
                    owner.insert(file.hash(), folder.clone());
                }
            }
        };
        for &(object, fate) in &ported {
            let held = self.objects.value(object);
            let name = keys.get(&object).or_else(|| leaves.get(&object));
            let folder = name.filter(|_| fate == Fate::Carried);
            let folder = folder.and_then(|name| self.layout.owner_folder(held, name));
            name_files(held, folder);
        }
        for (copy, leaf) in &copies {
            name_files(copy, self.layout.owner_folder(copy, leaf));
        }
        // Own bytes ship under the namespace. Game bytes are named where the game has them.
        let mut to: HashMap<WadHash, String> = HashMap::new();
        let verdicts = refs.of(unit).into_iter().flat_map(|refs| &refs.refs);
        for (&path, verdict) in verdicts.filter(|(path, _)| named.contains(path)) {
            let mut ship = |linking: &mut Self, file: FileId| {
                let place = linking.files.entry(file).or_insert_with(|| {
                    let owner = owner.get(&path).map(String::as_str);
                    linking
                        .layout
                        .place_file(&m.files[file], inputs.names, &dir, owner)
                });
                to.insert(path, place.as_str().to_string());
            };
            match verdict {
                FileVerdict::Own { file } => ship(self, *file),
                FileVerdict::Game { path: at } if *at != path => {
                    let moved = refs.moved.get(at).map(|path| path.as_str().to_string());
                    match moved.or_else(|| inputs.names.path(*at)) {
                        Some(spelled) => {
                            to.insert(path, spelled);
                            self.repointed += 1;
                        }
                        // A path string needs the game's spelling. Without it the mod's
                        // bytes ship.
                        None => {
                            let held = m.files.iter().find(|(_, file)| file.hash == path);
                            if let Some((file, _)) = held {
                                ship(self, file);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let moved = |held: &BinObject| {
            let moves = value::file_refs(held);
            moves.iter().any(|file| to.contains_key(&file.hash()))
        };
        for &(object, _) in &ported {
            if moved(self.objects.value(object)) {
                value::rewrite_files(self.objects.edit(object), &|hash| to.get(&hash).cloned());
            }
        }
        // #ADR-0088: an own file a copy names ships as a carried object's does.
        let mut copies: Vec<BinObject> = copies.into_iter().map(|(copy, _)| copy).collect();
        for copy in copies.iter_mut().filter(|copy| moved(copy)) {
            value::rewrite_files(copy, &|hash| to.get(&hash).cloned());
        }
        self.units.push(PortUnit {
            unit,
            character: node.character,
            champion,
            target: t,
            entry,
            resolver,
            graph,
            carried,
            copies,
            game_links: reached.game_links.clone(),
            dir,
        });
    }

    /// Each unit's objects follow its group's renames, and only those: a unit at another
    /// target that names the same entry means the game's object there, and keeps it.
    fn rename(&mut self) {
        let m: &Mod = self.inputs.fixed;
        // A resolver's key stays what it is, and so does a hash value naming one: the client
        // looks it up in the resolver.
        let resolver = name_hash(class::RESOURCE_RESOLVER);
        let resolvers = m.objects.values().filter(|object| object.class == resolver);
        let keys: HashSet<BinHash> = resolvers
            .flat_map(|object| value::map_keys(&object.value, field::RESOURCE_MAP))
            .collect();
        let mut done: HashSet<(ObjectId, Group)> = HashSet::new();
        for (objects, group) in &self.by_group {
            let renames = self.renames.of_group(*group);
            if renames.is_empty() {
                continue;
            }
            let named: HashSet<BinHash> = renames.keys().copied().collect();
            for &object in objects {
                if !done.insert((object, *group)) {
                    continue;
                }
                let held = self.objects.value(object);
                if !value::names_any(held, &named) {
                    continue;
                }
                let renamed = value::rename_hashes_by(held, &renames, &|hash| keys.contains(&hash));
                self.objects.set(object, renamed);
            }
        }
        for unit in &mut self.units {
            let renames = self.renames.of_group((unit.champion, unit.target));
            let named: HashSet<BinHash> = renames.keys().copied().collect();
            for copy in &mut unit.copies {
                if value::names_any(copy, &named) {
                    *copy = value::rename_hashes_by(copy, &renames, &|hash| keys.contains(&hash));
                }
            }
        }
    }
}

impl Port {
    /// The stage `plan`. It raises what each step found. The run's cancel ends the stage
    /// between two steps (#ADR-0097).
    pub fn plan(inputs: &PortInputs<'_>, sink: &mut Sink<'_>) -> Result<Self, PlanError> {
        let _span = tracing::info_span!("plan").entered();
        let m: &Mod = inputs.fixed;
        let mut objects = Objects::of(m);
        let mut layout = Layout::of(m);

        // The units are sorted under the names the mod gives them.
        sink.next_step()?;
        let reach = Reach::find(inputs, &objects)?;
        sink.next_step()?;
        let refs = FileRefs::find(inputs, &objects, &reach)?;
        sink.next_step()?;
        let clips = OwnClips::found(&inputs.facts.sounds);
        let shipping = Shipping::find(inputs, (&reach, &refs), &clips, sink);

        sink.next_step()?;
        let renames = Renames::retarget(
            inputs,
            &shipping,
            &mut objects,
            &mut layout.made.entries,
            sink,
        );
        sink.next_step()?;
        let audio = AudioPlan::plan(inputs, &shipping, &mut layout, sink)?;

        // At the target, under the target's names. A literal sound name is rewritten in the
        // objects the entry reaches, and what it reaches is found again once one changed.
        sink.next_step()?;
        let mut reach = Reach::find(inputs, &objects)?;
        if audio.rewrite_literals(inputs, &shipping, &reach, &mut objects, sink) {
            reach = Reach::find(inputs, &objects)?;
        }
        reach.report(&shipping.ships, sink);
        sink.next_step()?;
        let refs = FileRefs::find(inputs, &objects, &reach)?;
        refs.report(&shipping.ships, sink);

        sink.next_step()?;
        let mut linking = Linking {
            inputs,
            objects,
            layout,
            renames,
            files: BTreeMap::new(),
            repointed: 0,
            units: Vec::new(),
            by_group: Vec::new(),
            copied: HashSet::new(),
        };
        for &unit in &shipping.ships {
            linking.unit(unit, &reach, &refs);
        }
        linking.rename();
        let Linking {
            objects,
            mut layout,
            renames,
            files,
            repointed,
            units,
            ..
        } = linking;

        let answered = units.iter().flat_map(|unit| {
            let reached = reach.of(unit.unit).into_iter();
            reached.flat_map(|reached| reached.ported().map(|(object, _)| object))
        });
        let answered: BTreeSet<ObjectId> = answered.collect();
        let answered = answered.into_iter();
        let ported = answered.map(|object| (object, objects.value(object).clone()));
        let ported: BTreeMap<ObjectId, BinObject> = ported.collect();
        let carried: usize = units
            .iter()
            .map(|unit| unit.carried.len() + usize::from(unit.graph.is_some()))
            .sum();
        sink.raise(Carried {
            objects: carried,
            files: files.len(),
            repointed,
        });

        let shipped: BTreeSet<FileId> = files.keys().copied().collect();
        sink.next_step()?;
        let hud = HudPlan::find(inputs, &mut layout, &shipped, sink)?;
        for (_, place) in hud.champions.iter().flat_map(|hud| &hud.files) {
            layout.made_path(place);
        }
        Ok(Self {
            options: *inputs.options,
            shipping,
            reach,
            refs,
            layout,
            renames,
            units,
            objects: ported,
            files,
            repointed,
            audio,
            hud,
        })
    }

    /// The names the port made, for a run's book to learn: its paths and its entry names.
    pub fn made(&self) -> (impl Iterator<Item = &str>, impl Iterator<Item = &str>) {
        let made = &self.layout.made;
        (
            made.paths.iter().map(String::as_str),
            made.entries.iter().map(String::as_str),
        )
    }

    /// An object as the port ships it.
    pub fn object(&self, object: ObjectId) -> Option<&BinObject> {
        self.objects.get(&object)
    }

    /// The unit of the port a skin unit is.
    pub fn unit(&self, unit: UnitId) -> Option<&PortUnit> {
        self.units.iter().find(|held| held.unit == unit)
    }
}

impl Dump for Port {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = self.shipping.dump(cx);
        out.push_str(&self.renames.dump(cx));
        out.push_str(&self.reach.dump(cx));
        out.push_str(&self.refs.dump(cx));
        out.push_str(&self.layout.dump(cx));
        for unit in &self.units {
            let name = cx.module.map_or(String::new(), |m| {
                format!(" {}", clean(&m.unit_name(unit.unit)))
            });
            let graph = if unit.graph.is_some() { " graph" } else { "" };
            let _ = writeln!(
                out,
                "port {}{name} -> skin{} {} carried={}{graph}",
                unit.unit,
                unit.target,
                unit.dir,
                unit.carried.len()
            );
            for link in &unit.game_links {
                let _ = writeln!(out, "   links {link}");
            }
        }
        for (file, place) in &self.files {
            let _ = writeln!(out, "file {file} -> {place}");
        }
        out.push_str(&self.audio.dump(cx));
        out.push_str(&self.hud.dump(cx));
        out
    }
}
