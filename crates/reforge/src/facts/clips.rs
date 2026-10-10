//! Which of a unit's clips each slot plays, and for the others whose clips they are and
//! whether they would play on the unit's rig (#MOD-ARTIFACT-0025). Facts only: how much reuse
//! is accepted is the policy's (#ADR-0014).
//!
//! A clip binds each track to a joint by the hash of the joint's name, and a track for a joint
//! the skeleton lacks is skipped (#GAME-BEHAVIOR-0005). So a borrowed clip plays where it has
//! a track on some joint of the unit's skeleton, and its other tracks are extra.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::copies::Copies;
use super::origin::Origins;
use super::rig::{Rigs, Source, UnitRig};
use super::{Catalogs, FactsError};
use crate::env::Env;
use crate::fixups::FixedMod;
use crate::game::SkinCatalog;
use crate::model::entity::{CharId, CharKind, Mod, SkinUnit, UnitId};
use crate::model::rig;
use crate::model::{SideTable, Slot, WadHash};

/// Clips of one skin that the slot judged does not play.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reused {
    /// The skin they are from, a chroma counted as its parent.
    pub from: Slot,
    /// How many clips.
    pub clips: u32,
    /// Whether that skin's skeleton has the judged slot's joints, in order.
    pub same_joints: bool,
    /// Of those clips, the ones whose every track is on a joint of the unit's skeleton.
    pub joints_fit: u32,
    /// The ones with some tracks on its joints and others on joints it lacks.
    pub joints_extra: u32,
    /// The ones with no track on its joints: they move nothing of the unit's model.
    pub joints_off: u32,
    /// The ones whose joints, or the skeleton's, did not read.
    pub joints_unknown: u32,
    /// The joints the skeleton lacks, each with how many clips animate it. A joint is spelled
    /// from the skin's skeleton where that names it, else written as its hash.
    pub extra_joints: BTreeMap<String, u32>,
}

/// A unit's clips judged against one slot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipsAt {
    /// The clips the unit's graph plays.
    pub total: u32,
    /// Clips the slot's graph plays, and own clips with a track on a joint of the unit's
    /// skeleton.
    pub fits: u32,
    /// Own clips with no track on a joint of the unit's skeleton.
    pub own_off: u32,
    /// Clips nothing judges: own clips that did not read, clips no skin of the character
    /// plays.
    pub unknown: u32,
    /// Clips only other skins play, by skin.
    pub reused: Vec<Reused>,
    /// Whether the unit's graph was found.
    pub graph_found: bool,
    /// Clips that are the mod's own bytes.
    pub own: u32,
    /// Whether the unit's mesh and skeleton are both the mod's own.
    pub own_rig: bool,
}

impl ClipsAt {
    /// Whether the rig and the animation set are the author's: own mesh and skeleton, and at
    /// least half the clips own (#ADR-0014). Half counts: a paste onto an own rig plays its own
    /// copies of half the clips and the premium skin's other half by their game paths.
    pub fn own_work(&self) -> bool {
        self.own_rig && self.own * 2 >= self.total
    }

    /// How many clips only other skins play.
    pub fn borrowed(&self) -> u32 {
        self.reused.iter().map(|reused| reused.clips).sum()
    }

    /// Whether the unit borrows a few clips: fewer than half of what it plays (#ADR-0015).
    pub fn borrows_few(&self) -> bool {
        let borrowed = self.borrowed();
        borrowed > 0 && borrowed * 2 < self.total
    }

    /// The skin most of the unit's clips are from, with how many: more than half of what the
    /// unit plays, and of two such skins the lower slot (#ADR-0015).
    pub fn mostly_from(&self) -> Option<(Slot, u32)> {
        let most = self
            .reused
            .iter()
            .max_by_key(|reused| (reused.clips, std::cmp::Reverse(reused.from)));
        most.map(|reused| (reused.from, reused.clips))
            .filter(|(_, clips)| 2 * clips > self.total)
    }
}

/// The clip facts of one unit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitClips {
    /// The joints of every clip of the unit that some skin plays and that is not the mod's
    /// own, by the path the graph names. None for a clip that did not read.
    #[serde(with = "crate::model::pairs")]
    joints: BTreeMap<WadHash, Option<Vec<u32>>>,
    /// The clips judged at each candidate slot.
    pub at: BTreeMap<Slot, ClipsAt>,
}

/// What a unit's clips are judged with at a slot.
pub(super) struct ClipJudge<'a> {
    pub catalog: &'a SkinCatalog,
    pub rigs: &'a Rigs,
    pub character: CharId,
    pub rig: &'a UnitRig,
    pub clips: &'a UnitClips,
}

impl ClipJudge<'_> {
    /// The unit's clips against slot `k`.
    pub fn at(&self, k: Slot) -> ClipsAt {
        let Self {
            catalog,
            rigs,
            character,
            rig,
            clips,
        } = *self;
        let mut at = ClipsAt {
            total: rig.clips.len() as u32,
            graph_found: rig.graph.is_some(),
            own: rig.clips.iter().filter(|clip| clip.file.own).count() as u32,
            own_rig: rig.mesh.file.own && rig.skeleton.file.own,
            ..ClipsAt::default()
        };
        let skeleton = rig.joint_hashes();
        let k_canon = catalog.canon(k);
        let k_joints = rigs.game_joints(character, k_canon);
        let mut by_skin: BTreeMap<Slot, Reused> = BTreeMap::new();
        for clip in &rig.clips {
            let slots = &clip.file.game_slots;
            if slots.iter().any(|slot| catalog.canon(*slot) == k_canon) {
                at.fits += 1;
            } else if clip.file.own {
                match (&clip.joints, &skeleton) {
                    (Some(tracks), Some(joints)) if tracks.iter().any(|j| joints.contains(j)) => {
                        at.fits += 1
                    }
                    (Some(_), Some(_)) => at.own_off += 1,
                    _ => at.unknown += 1,
                }
            } else if let Some(&first) = slots.first() {
                let from = catalog.canon(first);
                let from_skeleton = rigs.game_skeleton(character, from);
                let reused = by_skin.entry(from).or_insert_with(|| Reused {
                    from,
                    clips: 0,
                    same_joints: from_skeleton.is_some()
                        && from_skeleton.map(|s| s.joints.as_slice()) == k_joints,
                    joints_fit: 0,
                    joints_extra: 0,
                    joints_off: 0,
                    joints_unknown: 0,
                    extra_joints: BTreeMap::new(),
                });
                reused.clips += 1;
                let tracks = clips.joints.get(&clip.reference()).and_then(Option::as_ref);
                match (tracks, &skeleton) {
                    (Some(tracks), Some(joints)) if tracks.iter().all(|j| joints.contains(j)) => {
                        reused.joints_fit += 1
                    }
                    (Some(tracks), Some(joints)) if tracks.iter().any(|j| joints.contains(j)) => {
                        reused.joints_extra += 1;
                        for &track in tracks.iter().filter(|j| !joints.contains(j)) {
                            let named = from_skeleton.and_then(|s| s.joint_named(track));
                            let name = named.map_or_else(|| format!("{track:08x}"), str::to_string);
                            *reused.extra_joints.entry(name).or_default() += 1;
                        }
                    }
                    (Some(_), Some(_)) => reused.joints_off += 1,
                    _ => reused.joints_unknown += 1,
                }
            } else {
                at.unknown += 1;
            }
        }
        at.reused = by_skin.into_values().collect();
        at
    }
}

/// The clip facts of the representative units.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Clips {
    units: SideTable<SkinUnit, UnitClips>,
}

/// The candidate slots of a unit: the base, its own slot and its premium origin; for a
/// sub-character also the slots and origins of its champion's units, which it may follow. Only
/// slots the game has a skin of.
pub fn candidate_slots(
    m: &Mod,
    catalog: &SkinCatalog,
    origins: &Origins,
    copies: &Copies,
    id: UnitId,
) -> BTreeSet<Slot> {
    let unit = &m.units[id];
    let mut slots: BTreeSet<Slot> = [Slot::BASE, unit.source_slot].into_iter().collect();
    slots.extend(origins.premium(id));
    if let CharKind::Sub { champion } = m.characters[unit.character].kind {
        let units = m.characters[champion].units.iter();
        for &of_champion in units.filter(|u| copies.is_representative(**u)) {
            slots.insert(m.units[of_champion].source_slot);
            slots.extend(origins.premium(of_champion));
        }
    }
    slots.retain(|slot| catalog.skins.contains_key(slot));
    slots
}

impl Clips {
    /// The clip facts of a mod's representative units, at each candidate slot.
    pub fn find(
        fixed: &FixedMod,
        env: &Env,
        catalogs: &Catalogs,
        copies: &Copies,
        origins: &Origins,
        rigs: &Rigs,
    ) -> Result<Self, FactsError> {
        let m: &Mod = fixed;
        let mut out = Self::default();
        for id in copies.representatives() {
            let character = m.units[id].character;
            let (Some(catalog), Some(unit_rig)) = (catalogs.of(character), rigs.of(id)) else {
                continue;
            };
            let mut clips = UnitClips::default();
            // A clip reused at one slot may be the slot's own at another, so the joints of
            // every clip some skin plays are read once.
            let played = unit_rig
                .clips
                .iter()
                .filter(|clip| !clip.file.own && !clip.file.game_slots.is_empty());
            for clip in played {
                let joints = match clip.file.source {
                    Source::Mod(file) => rig::clip_joints(&m.files[file].bytes),
                    Source::Game => {
                        let path = clip.file.moved.unwrap_or(clip.reference());
                        env.game().clip_joints(path)?.map(|joints| joints.to_vec())
                    }
                    Source::Missing => None,
                };
                clips.joints.insert(clip.reference(), joints);
            }
            let judge = ClipJudge {
                catalog,
                rigs,
                character,
                rig: unit_rig,
                clips: &clips,
            };
            let at = candidate_slots(m, catalog, origins, copies, id)
                .into_iter()
                .map(|k| (k, judge.at(k)))
                .collect();
            clips.at = at;
            out.units.insert(id, clips);
        }
        Ok(out)
    }

    /// A unit's clip facts. None for a unit with no rig.
    pub fn of(&self, unit: UnitId) -> Option<&UnitClips> {
        self.units.get(unit)
    }

    /// Every unit with clip facts, in unit order.
    pub fn iter(&self) -> impl Iterator<Item = (UnitId, &UnitClips)> {
        self.units.iter()
    }
}
