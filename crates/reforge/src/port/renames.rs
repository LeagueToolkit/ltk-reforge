//! The names a port changes, as maps: a unit's `Skin<S>` names to its target's, and each
//! carried object's name to the one it takes under the namespace.
//!
//! A unit whose target is not its slot has its skin entry and resolver renamed to the
//! target's, and its graph when the mod defines one. Other names that spell the source slot
//! are the unit's own objects, which take a name under the namespace.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use serde::{Serialize, Serializer};

use super::findings::Retargeted;
use super::plan::{Objects, PortInputs};
use super::shipping::Shipping;
use crate::diag::Sink;
use crate::model::entity::{BinId, CharId, Mod, ObjectId, UnitId};
use crate::model::hash::skin_paths;
use crate::model::{BinHash, Dump, DumpCx, Slot, name_hash, value};

/// A champion and its sub-characters at one target: the units that load together and share
/// one set of names.
pub type Group = (CharId, Slot);

fn pairs_hex<S: Serializer>(pairs: &[(BinHash, BinHash)], s: S) -> Result<S::Ok, S::Error> {
    let rows = pairs.iter();
    s.collect_seq(rows.map(|(from, to)| (format!("{:08x}", from.0), format!("{:08x}", to.0))))
}

/// A unit whose names moved from its slot to its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Retarget {
    /// The unit.
    pub unit: UnitId,
    /// The slot its bin sits in.
    pub from: Slot,
    /// Its target.
    pub to: Slot,
}

/// Every rename of a port.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Renames {
    /// The units retargeted.
    pub retargeted: Vec<Retarget>,
    /// Source to target names every object takes: a skin entry, a resolver, and a graph the
    /// mod defines.
    #[serde(serialize_with = "pairs_hex")]
    pub to_target: Vec<(BinHash, BinHash)>,
    /// Source to target names that swap one game object for another: the graph of the source
    /// slot for the target's. The objects of a unit whose rig does not fit its target do not
    /// take them.
    #[serde(serialize_with = "pairs_hex")]
    pub game_swaps: Vec<(BinHash, BinHash)>,
    /// How many objects kept their links to the source slot's game objects.
    pub links_kept: usize,
    /// The names carried objects take under the namespace, by group.
    #[serde(skip)]
    pub namespaced: BTreeMap<Group, Vec<(BinHash, BinHash)>>,
    /// The one new name of each carried object, whichever unit carries it first. A report
    /// holds them in object order.
    #[serde(serialize_with = "names_in_order")]
    pub names: HashMap<ObjectId, BinHash>,
}

fn names_in_order<S: Serializer>(
    names: &HashMap<ObjectId, BinHash>,
    s: S,
) -> Result<S::Ok, S::Error> {
    let ordered: BTreeMap<ObjectId, String> = names
        .iter()
        .map(|(object, name)| (*object, format!("{:08x}", name.0)))
        .collect();
    s.collect_map(ordered)
}

impl Renames {
    /// Moves the names of each shipping unit from its slot to its target, in `objects`, and
    /// keeps the names made in `made`.
    pub fn retarget(
        inputs: &PortInputs<'_>,
        shipping: &Shipping,
        objects: &mut Objects<'_>,
        made: &mut BTreeSet<String>,
        sink: &mut Sink<'_>,
    ) -> Self {
        let m: &Mod = inputs.fixed;
        let mut out = Self::default();
        // Names the mod's own objects define: renaming one moves the mod's object.
        let defined: HashSet<BinHash> = m.objects.values().map(|object| object.entry).collect();
        let mut unfit: BTreeSet<UnitId> = BTreeSet::new();
        for &unit in &shipping.ships {
            let node = &m.units[unit];
            let Some(t) = inputs.targets.target(unit) else {
                continue;
            };
            let s = node.source_slot;
            // Judged at the target and found not to fit, or judged elsewhere only.
            let candidates = inputs.facts.candidates.get(unit);
            if candidates.is_some_and(|candidates| {
                let at = candidates.slots.get(&t);
                at.is_none_or(|facts| facts.checks.iter().any(|check| check.fit.blocks()))
            }) {
                unfit.insert(unit);
            }
            if s == t {
                continue;
            }
            let c = &m.characters[node.character].spelled;
            let names = [
                (
                    skin_paths::skin_entry(c, s),
                    skin_paths::skin_entry(c, t),
                    true,
                ),
                (
                    skin_paths::resolver_entry(c, s),
                    skin_paths::resolver_entry(c, t),
                    true,
                ),
                (
                    skin_paths::graph_entry(c, s),
                    skin_paths::graph_entry(c, t),
                    false,
                ),
            ];
            for (from, to, identity) in names {
                let pair = (name_hash(&from), name_hash(&to));
                made.insert(to);
                if identity || defined.contains(&pair.0) {
                    out.to_target.push(pair);
                } else {
                    out.game_swaps.push(pair);
                }
            }
            // The entry the unit picked may spell another slot: a copy kept its source's name.
            if let Some(entry) = node.entry {
                let now = m.objects[entry].entry;
                let want = name_hash(&skin_paths::skin_entry(c, t));
                if now != want && !out.to_target.iter().any(|(from, _)| *from == now) {
                    out.to_target.push((now, want));
                }
            }
            out.retargeted.push(Retarget {
                unit,
                from: s,
                to: t,
            });
            sink.raise(Retargeted {
                unit,
                from: s,
                to: t,
            });
        }
        if out.to_target.is_empty() && out.game_swaps.is_empty() {
            return out;
        }
        // The objects only units whose rig does not fit their target load keep their links to
        // the game's objects.
        let mut loaders: BTreeMap<BinId, (bool, bool)> = BTreeMap::new();
        for &unit in &shipping.ships {
            for &bin in &m.units[unit].loads {
                let loaded = loaders.entry(bin).or_default();
                match unfit.contains(&unit) {
                    true => loaded.0 = true,
                    false => loaded.1 = true,
                }
            }
        }
        let keeps_links = |bin: BinId| loaders.get(&bin) == Some(&(true, false));
        let all: Vec<(BinHash, BinHash)> = out
            .to_target
            .iter()
            .chain(&out.game_swaps)
            .copied()
            .collect();
        let named: HashSet<BinHash> = all.iter().map(|(from, _)| *from).collect();
        // Every object of the mod, not the group's alone: a bin can hold objects under another
        // unit's source names beside its own entry, which takes one of them
        // (`docs/design/ARCHITECTURE.md`, "Plan").
        for (id, object) in m.objects.iter() {
            if !value::names_any(&object.value, &named) {
                continue;
            }
            let kept = keeps_links(object.bin);
            let with = if kept { &out.to_target } else { &all };
            let renamed = value::rename_hashes(&object.value, with);
            if kept && value::rename_hashes(&object.value, &all) != renamed {
                out.links_kept += 1;
            }
            if renamed != object.value {
                objects.set(id, renamed);
            }
        }
        out
    }

    /// The renames of a group as a table.
    pub fn of_group(&self, group: Group) -> HashMap<BinHash, BinHash> {
        let mut table = HashMap::new();
        for &(from, to) in self.namespaced.get(&group).into_iter().flatten() {
            table.entry(from).or_insert(to);
        }
        table
    }
}

impl Dump for Renames {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = String::new();
        for retarget in &self.retargeted {
            let _ = writeln!(
                out,
                "retarget {} skin{} -> skin{}",
                retarget.unit, retarget.from, retarget.to
            );
        }
        let namespaced: usize = self.namespaced.values().map(Vec::len).sum();
        let _ = writeln!(
            out,
            "renames to-target={} game-swaps={} links-kept={} namespaced={namespaced}",
            self.to_target.len(),
            self.game_swaps.len(),
            self.links_kept
        );
        for (from, to) in &self.to_target {
            let _ = writeln!(
                out,
                "   {} -> {}",
                crate::model::clean(&cx.names.entry_or_hex(*from)),
                crate::model::clean(&cx.names.entry_or_hex(*to))
            );
        }
        out
    }
}
