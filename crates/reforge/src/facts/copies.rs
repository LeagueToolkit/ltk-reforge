//! Which skin units are copies of one another (#MOD-ARTIFACT-0001): the same bin written to
//! many slots, with its entry and resolver renamed, sometimes from another build of the mod.
//! Related units form clusters, and each cluster has one representative: the unit the author
//! made, as far as the archive shows.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::FactsError;
use super::findings::{CopiesFound, DistinctUnits, NearCopy};
use crate::diag::Sink;
use crate::env::Env;
use crate::fixups::FixedMod;
use crate::game::SkinCatalog;
use crate::model::entity::{CharId, Mod, SkinUnit, UnitId};
use crate::model::hash::skin_paths;
use crate::model::{BinHash, SideTable, SkinEntry, Slot, name_hash, value};

/// The share of slot-normalised objects two bins have in common from which they are near
/// copies. Set over ltk-forge's first corpus (1,029 mods): builds of one mod at several slots
/// share 80% and more of their objects, distinct mods of one character far fewer.
pub const NEAR_JACCARD: f64 = 0.8;

/// A stand-in for a slot's own name while two skins' bins are compared: the skin entry, the
/// resolver and the graph of the slot normalised take this value and the two after it. No
/// entry name hashes to them in the game's bins.
const SLOT_PLACEHOLDER: u32 = 0x5107_0000;

/// How two units are the same work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CopyKind {
    /// The same bytes.
    ByteIdentical,
    /// Equal once each bin's own slot names are normalised.
    Renamed,
    /// Most objects equal and the same rig: another build of the mod.
    Near {
        /// The objects both bins hold.
        shared: u32,
        /// The objects one of them holds.
        differing: u32,
    },
    /// The same mesh, skeleton and texture: the same work under another identity.
    SameRig,
}

impl CopyKind {
    /// The kind's name in a dump.
    pub fn as_str(self) -> &'static str {
        match self {
            CopyKind::ByteIdentical => "identical",
            CopyKind::Renamed => "renamed",
            CopyKind::Near { .. } => "near",
            CopyKind::SameRig => "same rig",
        }
    }
}

/// A copy of a representative unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// The unit.
    pub unit: UnitId,
    /// Its slot.
    pub slot: Slot,
    /// How it is the representative's work.
    #[serde(flatten)]
    pub kind: CopyKind,
}

/// What a unit is in its cluster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitRole {
    /// The one unit its cluster is ported as.
    Representative {
        /// The other units of the cluster.
        members: Vec<Member>,
        /// Whether the cluster covers a skin other than the representative's own, chromas
        /// counted as their parent: a bin written over other skins' slots, meant for whatever
        /// skin the player picks.
        spread: bool,
    },
    /// A copy of its cluster's representative.
    Copy {
        /// The representative.
        of: UnitId,
        /// How it is the representative's work.
        kind: CopyKind,
    },
}

/// The representatives of one character, in the order of their clusters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OfCharacter {
    character: CharId,
    representatives: Vec<UnitId>,
}

/// The role of every skin unit the mod adds something to.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Copies {
    roles: SideTable<SkinUnit, UnitRole>,
    slots: SideTable<SkinUnit, Slot>,
    characters: Vec<OfCharacter>,
}

/// What two bins are compared by.
struct Print {
    objects: BTreeSet<[u8; 32]>,
    links: Vec<String>,
    rig: Vec<Option<String>>,
}

/// The names a bin at these slots may give its own skin, each with its stand-in.
fn slot_names(character: &str, slots: &[Slot]) -> Vec<(BinHash, BinHash)> {
    let mut out = Vec::new();
    for &slot in slots {
        let names = [
            skin_paths::skin_entry(character, slot),
            skin_paths::resolver_entry(character, slot),
            skin_paths::graph_entry(character, slot),
        ];
        for (at, name) in (0u32..).zip(names) {
            out.push((name_hash(&name), BinHash(SLOT_PLACEHOLDER + at)));
        }
    }
    out
}

fn normalise_link(link: &str, slots: &[Slot]) -> String {
    let mut link = link.to_ascii_lowercase();
    for slot in slots {
        link = link
            .replace(&format!("/skin{}.bin", slot.0), "/skin#.bin")
            .replace(&format!("/skin{}/", slot.0), "/skin#/");
    }
    link
}

impl Print {
    fn of(m: &Mod, unit: &SkinUnit) -> Option<Self> {
        let bin = &m.bins[unit.bin?];
        let character = &m.characters[unit.character].name;
        let mut slots = vec![unit.source_slot];
        slots.extend(unit.entry_name_slot.filter(|s| *s != unit.source_slot));
        let renames = slot_names(character, &slots);
        let digest = |&o| value::digest(&value::rename_hashes(&m.objects[o].value, &renames));
        let mut links: Vec<String> = bin
            .links
            .iter()
            .map(|link| link.path.raw())
            .map(|path| normalise_link(path, &slots))
            .collect();
        links.sort();
        let wad = m.files[unit.file].wad;
        // A shipped file compares by its bytes: copies often name one file under two paths.
        let file = |file: Option<value::FileRef>| {
            file.map(|file| match m.file_at(wad, file.hash()) {
                Some(shipped) => m.files[shipped].content.hex(),
                None => format!("{:016x}", file.hash().0),
            })
        };
        // The graph is left out: a paste in skin0 names its premium slot's graph, which only
        // the premium slot's own copy normalises.
        let rig = unit.entry.map_or(Vec::new(), |entry| {
            let entry = SkinEntry::read(&m.objects[entry].value);
            vec![
                file(entry.mesh()),
                file(entry.skeleton()),
                file(entry.texture()),
            ]
        });
        Some(Self {
            objects: bin.objects.iter().map(digest).collect(),
            links,
            rig,
        })
    }

    fn same_rig(&self, other: &Self) -> bool {
        !self.rig.is_empty() && self.rig.iter().any(Option::is_some) && self.rig == other.rig
    }
}

/// How two units of one character are related, when they are.
fn relate(
    m: &Mod,
    a: (&SkinUnit, Option<&Print>),
    b: (&SkinUnit, Option<&Print>),
) -> Option<CopyKind> {
    if m.files[a.0.file].content == m.files[b.0.file].content {
        return Some(CopyKind::ByteIdentical);
    }
    let (a, b) = (a.1?, b.1?);
    if a.objects == b.objects && a.links == b.links {
        return Some(CopyKind::Renamed);
    }
    let shared = a.objects.intersection(&b.objects).count();
    let union = a.objects.union(&b.objects).count();
    let same_rig = a.same_rig(b);
    if union > 0 && shared as f64 / union as f64 >= NEAR_JACCARD && same_rig {
        return Some(CopyKind::Near {
            shared: shared as u32,
            differing: (union - shared) as u32,
        });
    }
    same_rig.then_some(CopyKind::SameRig)
}

/// The root of `i` in a union-find forest, with the path to it shortened.
fn root(parent: &mut [usize], i: usize) -> usize {
    let mut r = i;
    while parent[r] != r {
        r = parent[r];
    }
    let mut j = i;
    while parent[j] != r {
        j = std::mem::replace(&mut parent[j], r);
    }
    r
}

impl Copies {
    /// The copies among a mod's skin units, character by character. A sub-character whose
    /// bins are the game's is left out: the mod adds nothing to it.
    pub fn find(fixed: &FixedMod, env: &Env) -> Result<Self, FactsError> {
        let m: &Mod = fixed;
        let mut out = Self::default();
        for (id, character) in m.characters.iter() {
            if m.equal_to_game.contains(&id) || character.units.is_empty() {
                continue;
            }
            let catalog = env.game().catalog(&character.name)?;
            let representatives = out.cluster(m, &character.units, &catalog);
            out.characters.push(OfCharacter {
                character: id,
                representatives,
            });
        }
        Ok(out)
    }

    /// Sets the role of each of a character's units, and gives the representatives.
    fn cluster(&mut self, m: &Mod, units: &[UnitId], catalog: &SkinCatalog) -> Vec<UnitId> {
        let prints: Vec<Option<Print>> = units.iter().map(|&u| Print::of(m, &m.units[u])).collect();
        let mut parent: Vec<usize> = (0..units.len()).collect();
        let mut kinds: BTreeMap<(usize, usize), CopyKind> = BTreeMap::new();
        for i in 0..units.len() {
            for j in (i + 1)..units.len() {
                let a = (&m.units[units[i]], prints[i].as_ref());
                let b = (&m.units[units[j]], prints[j].as_ref());
                let Some(kind) = relate(m, a, b) else {
                    continue;
                };
                kinds.insert((i, j), kind);
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                if ri != rj {
                    parent[ri.max(rj)] = ri.min(rj);
                }
            }
        }
        let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for i in 0..units.len() {
            clusters.entry(root(&mut parent, i)).or_default().push(i);
        }
        let slot = |i: usize| m.units[units[i]].source_slot;
        let mut representatives = Vec::new();
        for members in clusters.values() {
            // Slot 0, else a member that is no chroma and whose entry names its own slot,
            // else the lowest slot.
            let own = |&i: &usize| {
                !catalog.is_chroma(slot(i)) && m.units[units[i]].entry_name_slot == Some(slot(i))
            };
            let base = members.iter().copied().find(|&i| slot(i).is_base());
            let rep = base
                .or_else(|| members.iter().copied().find(own))
                .unwrap_or(members[0]);
            let mut list = Vec::new();
            for &i in members.iter().filter(|&&i| i != rep) {
                // Two members related only through a third are the renamed kind.
                let kind = kinds
                    .get(&(i.min(rep), i.max(rep)))
                    .copied()
                    .unwrap_or(CopyKind::Renamed);
                list.push(Member {
                    unit: units[i],
                    slot: slot(i),
                    kind,
                });
                self.roles.insert(
                    units[i],
                    UnitRole::Copy {
                        of: units[rep],
                        kind,
                    },
                );
                self.slots.insert(units[i], slot(i));
            }
            let canon = catalog.canon(slot(rep));
            let spread = members.iter().any(|&i| catalog.canon(slot(i)) != canon);
            self.roles.insert(
                units[rep],
                UnitRole::Representative {
                    members: list,
                    spread,
                },
            );
            self.slots.insert(units[rep], slot(rep));
            representatives.push(units[rep]);
        }
        representatives
    }

    /// A unit's role. None for a unit the mod adds nothing to.
    pub fn role(&self, unit: UnitId) -> Option<&UnitRole> {
        self.roles.get(unit)
    }

    /// Every unit with its role, in unit order.
    pub fn iter(&self) -> impl Iterator<Item = (UnitId, &UnitRole)> {
        self.roles.iter()
    }

    /// Whether a unit is the representative of its cluster.
    pub fn is_representative(&self, unit: UnitId) -> bool {
        matches!(self.role(unit), Some(UnitRole::Representative { .. }))
    }

    /// The representatives, in unit order.
    pub fn representatives(&self) -> impl Iterator<Item = UnitId> + '_ {
        self.iter()
            .filter(|(_, role)| matches!(role, UnitRole::Representative { .. }))
            .map(|(unit, _)| unit)
    }

    /// Whether a unit's cluster was written over other skins' slots.
    pub fn is_spread(&self, unit: UnitId) -> bool {
        matches!(
            self.role(unit),
            Some(UnitRole::Representative { spread: true, .. })
        )
    }

    /// The representative a unit is a copy of.
    pub fn copy_of(&self, unit: UnitId) -> Option<UnitId> {
        match self.role(unit) {
            Some(UnitRole::Copy { of, .. }) => Some(*of),
            _ => None,
        }
    }

    /// Raises what was found: each near copy, the copies of each representative, and a
    /// character with several distinct units.
    pub fn report(&self, sink: &mut Sink<'_>) {
        for of_character in &self.characters {
            for &rep in &of_character.representatives {
                let Some(UnitRole::Representative { members, .. }) = self.role(rep) else {
                    continue;
                };
                for member in members {
                    if let CopyKind::Near { shared, differing } = member.kind {
                        sink.raise(NearCopy {
                            unit: member.unit,
                            of: rep,
                            of_slot: self.slots.get(rep).copied().unwrap_or(Slot::BASE),
                            shared,
                            differing,
                        });
                    }
                }
                if !members.is_empty() {
                    sink.raise(CopiesFound {
                        unit: rep,
                        copies: members.iter().map(|member| member.unit).collect(),
                        slots: members.iter().map(|member| member.slot).collect(),
                    });
                }
            }
            if of_character.representatives.len() > 1 {
                sink.raise(DistinctUnits {
                    character: of_character.character,
                    units: of_character.representatives.len(),
                });
            }
        }
    }
}

/// Slots as a person reads a run of them: `1 2 3 4 7` as `1-4 7`.
pub fn compact(slots: &[Slot]) -> String {
    let mut numbers: Vec<u16> = slots.iter().map(|slot| slot.0).collect();
    numbers.sort_unstable();
    let mut out = Vec::new();
    let mut i = 0;
    while i < numbers.len() {
        let mut j = i;
        while j + 1 < numbers.len() && numbers[j + 1] == numbers[j] + 1 {
            j += 1;
        }
        out.push(match j > i {
            true => format!("{}-{}", numbers[i], numbers[j]),
            false => numbers[i].to_string(),
        });
        i = j + 1;
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_in_a_run_are_written_as_a_range() {
        let slots: Vec<Slot> = [7, 1, 2, 3, 4].into_iter().map(Slot).collect();
        assert_eq!(compact(&slots), "1-4 7");
        assert_eq!(compact(&[]), "");
    }
}
