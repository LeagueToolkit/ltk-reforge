//! The objects a fixup set is applied to. Every loaded object by default (#ADR-0002). A caller
//! that narrows the set passes one it computed: what the units' entries reach is the one a
//! port writes at most.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::entity::{ObjectId, UnitId};
use crate::model::hash::skin_paths;
use crate::model::{BinHash, MAX_SLOT, Mod, Slot, name_hash, value};

/// A set of the mod's objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectSet {
    /// Every object the client loads.
    Every,
    /// These objects.
    Only(BTreeSet<ObjectId>),
}

impl ObjectSet {
    /// Every object the client loads.
    pub fn every() -> Self {
        ObjectSet::Every
    }

    /// Whether the set holds an object.
    pub fn contains(&self, object: ObjectId) -> bool {
        match self {
            ObjectSet::Every => true,
            ObjectSet::Only(objects) => objects.contains(&object),
        }
    }

    /// The objects these units reach by name: from each unit's skin entry, and from each
    /// object under a name the entry, the resolver or the graph of its character has at any
    /// slot, through every link, within the unit's load. A sub-character's unit also reaches
    /// into the loads of its champion's units among `units`.
    ///
    /// It holds everything a port can carry for the units at any target slot.
    pub fn reached(m: &Mod, units: &[UnitId]) -> Self {
        let mut out = BTreeSet::new();
        for &unit in units {
            let Some(entry) = m.units[unit].entry else {
                continue;
            };
            let character = m.units[unit].character;
            let champion = m.champion_of(character);
            let mut pool = Pool::default();
            pool.add(m, unit);
            if champion != character {
                let of_champion = m.characters[champion].units.iter();
                for &other in of_champion.filter(|other| units.contains(other)) {
                    pool.add(m, other);
                }
            }
            let mut seen: BTreeSet<BinHash> = BTreeSet::new();
            let mut stack = vec![m.objects[entry].entry];
            stack.extend(identities(&m.characters[character].spelled));
            while let Some(name) = stack.pop() {
                if !seen.insert(name) {
                    continue;
                }
                if let Some(&object) = pool.by_entry.get(&name) {
                    out.insert(object);
                    stack.extend(value::links(&m.objects[object].value));
                }
            }
        }
        ObjectSet::Only(out)
    }
}

/// The objects the client loads for some units, the first under each entry hash.
#[derive(Default)]
struct Pool {
    by_entry: BTreeMap<BinHash, ObjectId>,
}

impl Pool {
    fn add(&mut self, m: &Mod, unit: UnitId) {
        for &bin in &m.units[unit].loads {
            for &object in &m.bins[bin].objects {
                self.by_entry
                    .entry(m.objects[object].entry)
                    .or_insert(object);
            }
        }
    }
}

/// The names a character's skin entry, resolver and graph have at every slot. A unit's target
/// is chosen after the fixups, so each of them may become the unit's.
fn identities(character: &str) -> impl Iterator<Item = BinHash> {
    (0..=MAX_SLOT).map(Slot).flat_map(move |slot| {
        [
            skin_paths::skin_entry(character, slot),
            skin_paths::resolver_entry(character, slot),
            skin_paths::graph_entry(character, slot),
        ]
        .map(|name| name_hash(&name))
    })
}
