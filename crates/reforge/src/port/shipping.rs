//! Which units ship, and why each other unit leaves the port (#ADR-0020, #ADR-0082).
//!
//! A copy leaves, and a unit with no target. Of the units left, two more kinds leave:
//!
//! - the game's own skin, shipped again by a spread beside the mod's skin: every object it
//!   reaches is the game's, of its target's skin or of no known skin, every file it names is
//!   the game's, and another unit of the character carries the mod's work and every own clip
//!   it names;
//! - a chroma that is the same work as the unit at its parent (#ADR-0018): every own file it
//!   names is, by its bytes, one the parent's unit names, and so is every own clip.
//!
//! Then a port changes its target slot only (#ADR-0082): the slot a champion's skin0 unit
//! goes to, when that unit is the author's. The units of the champion and of its
//! sub-characters at other slots leave unless the options keep them, and the own work they
//! hold that the units at the target do not hold is named.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::Serialize;

use super::audio::{ClipRef, OwnClips};
use super::decide::Reason;
use super::findings::{LeftRow, UnitWork, UnitsLeft, WorkKind, WorkLeft};
use super::plan::PortInputs;
use super::reach::{Fate, Reach};
use super::refs::{FileRefs, FileVerdict};
use crate::diag::{DiagId, Sink};
use crate::facts::{ObjectOrigin, UnitRole};
use crate::game::SkinClass;
use crate::model::entity::{CharId, CharKind, FileId, FileKind, Mod, ObjectId, UnitId};
use crate::model::{ContentId, Dump, DumpCx, Slot, clean};

/// Why a unit leaves the port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "why", rename_all = "kebab-case")]
pub enum Left {
    /// A copy of another unit, which is ported in its place.
    Copy {
        /// The unit it is a copy of.
        of: UnitId,
    },
    /// Another unit of the character took the slot.
    Superseded {
        /// The unit that keeps the slot.
        by: Option<UnitId>,
    },
    /// Units of the character of one rank target the slot, and each leaves it (#ADR-0076).
    Conflict,
    /// Made for the champion's model before its rework (#ADR-0019).
    PreRework,
    /// A sub-character whose model does not fit the base, left to the game.
    SubLeft,
    /// A sub-character whose every skin bin is the game's byte for byte: the mod adds nothing
    /// to it.
    EqualToGame,
    /// No target, for any other reason.
    NoTarget,
    /// The game's own skin, shipped again by a spread.
    GameSkin,
    /// A chroma that is the same work as the unit at its parent.
    ChromaFolded {
        /// The unit at the chroma's parent.
        into: UnitId,
    },
    /// At another slot than the one its champion's port changes (#ADR-0082).
    OffTarget {
        /// The unit that holds the target slot.
        target: UnitId,
    },
}

impl Left {
    /// The reason as a message says it.
    pub fn as_str(&self) -> &'static str {
        match self {
            Left::Copy { .. } => "a copy",
            Left::Superseded { .. } => "superseded",
            Left::Conflict => "tied with another unit on its target",
            Left::PreRework => "made before the rework",
            Left::SubLeft => "left to the game's base",
            Left::EqualToGame => "equal to the game's",
            Left::NoTarget => "no target",
            Left::GameSkin => "the game's own skin",
            Left::ChromaFolded { .. } => "a chroma folded into its parent's unit",
            Left::OffTarget { .. } => "at a slot the port does not change",
        }
    }
}

/// A unit that leaves the port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LeftUnit {
    /// Why.
    #[serde(flatten)]
    pub why: Left,
    /// The diagnostic that says so.
    pub diagnostic: DiagId,
}

/// The units that ship, and the ones that leave.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Shipping {
    /// The units that ship, in unit order.
    pub ships: Vec<UnitId>,
    /// The units that leave, each with why.
    pub left: BTreeMap<UnitId, LeftUnit>,
}

/// The units that leave though they have a target.
struct Folded {
    game: BTreeSet<UnitId>,
    chromas: BTreeMap<UnitId, UnitId>,
}

impl Shipping {
    /// Sorts the mod's units. `reach` and `refs` are of the units as the mod names them,
    /// before any rename.
    pub fn find(
        inputs: &PortInputs<'_>,
        (reach, refs): (&Reach, &FileRefs),
        clips: &OwnClips,
        sink: &mut Sink<'_>,
    ) -> Self {
        let m: &Mod = inputs.fixed;
        let folded = Self::folded(inputs, reach, refs, clips);
        let mut ships = Vec::new();
        let mut left: BTreeMap<UnitId, Left> = BTreeMap::new();
        for unit in m.units.ids() {
            let decision = inputs.targets.decisions.get(&unit);
            let why = match inputs.facts.copies.role(unit) {
                Some(UnitRole::Copy { of, .. }) => Some(Left::Copy { of: *of }),
                _ => match decision.map(|decision| (decision.chosen, decision)) {
                    Some((None, decision)) => Some(match decision.reason {
                        Reason::Superseded => Left::Superseded {
                            by: decision.taken_by,
                        },
                        Reason::Conflict => Left::Conflict,
                        Reason::PreRework => Left::PreRework,
                        Reason::SubLeftToBase => Left::SubLeft,
                        _ => Left::NoTarget,
                    }),
                    None if m.equal_to_game.contains(&m.units[unit].character) => {
                        Some(Left::EqualToGame)
                    }
                    None => Some(Left::NoTarget),
                    Some((Some(_), _)) if folded.game.contains(&unit) => Some(Left::GameSkin),
                    Some((Some(_), _)) => folded
                        .chromas
                        .get(&unit)
                        .map(|&into| Left::ChromaFolded { into }),
                },
            };
            match why {
                Some(why) => {
                    left.insert(unit, why);
                }
                None => ships.push(unit),
            }
        }
        let work = if inputs.options.keep_other_slots {
            Vec::new()
        } else {
            Self::off_target(inputs, (reach, refs), clips, &mut ships, &mut left)
        };
        // One finding per character says why its units leave.
        let mut by_character: BTreeMap<CharId, Vec<LeftRow>> = BTreeMap::new();
        for (&unit, &why) in &left {
            let rows = by_character.entry(m.units[unit].character).or_default();
            rows.push(LeftRow { unit, why });
        }
        let mut out = Self {
            ships,
            left: BTreeMap::new(),
        };
        for (character, units) in by_character {
            let rows = units.clone();
            let diagnostic = sink.raise(UnitsLeft { character, units });
            for row in rows {
                let why = row.why;
                out.left.insert(row.unit, LeftUnit { why, diagnostic });
            }
        }
        for found in work {
            sink.raise(found);
        }
        out
    }

    /// The slot a champion's port changes, and the unit that holds it: where its skin0 unit
    /// goes, when that unit is the author's (#ADR-0082). None when the skin0 unit is no work of
    /// the author's, or the champion has none.
    fn target_of(
        inputs: &PortInputs<'_>,
        champion: CharId,
        ships: &[UnitId],
        left: &BTreeMap<UnitId, Left>,
    ) -> Option<(Slot, UnitId)> {
        let m: &Mod = inputs.fixed;
        let mut units = m.characters[champion].units.iter().copied();
        let skin0 = units.find(|&unit| m.units[unit].source_slot == Slot::BASE)?;
        // The unit that took the skin0 unit's slot holds it, and a copy's work is its original's.
        let holder = match left.get(&skin0) {
            None => skin0,
            Some(Left::Superseded { by: Some(by) }) => *by,
            Some(Left::Copy { of }) => *of,
            Some(_) => return None,
        };
        let slot = inputs.targets.target(holder)?;
        ships.contains(&holder).then_some((slot, holder))
    }

    /// Leaves each unit at another slot than its champion's target (#ADR-0082), and gives,
    /// per character, what those units hold of their own that the units at the target do not.
    fn off_target(
        inputs: &PortInputs<'_>,
        (reach, refs): (&Reach, &FileRefs),
        clips: &OwnClips,
        ships: &mut Vec<UnitId>,
        left: &mut BTreeMap<UnitId, Left>,
    ) -> Vec<WorkLeft> {
        let m: &Mod = inputs.fixed;
        let champion_of = |unit: UnitId| {
            let character = m.units[unit].character;
            match m.characters[character].kind {
                CharKind::Champion => character,
                CharKind::Sub { champion } => champion,
            }
        };
        let mut found = Vec::new();
        for champion in m.characters.ids() {
            if m.characters[champion].kind != CharKind::Champion {
                continue;
            }
            let Some((slot, holder)) = Self::target_of(inputs, champion, ships, left) else {
                continue;
            };
            let family = ships
                .iter()
                .copied()
                .filter(|&u| champion_of(u) == champion);
            let (kept, gone): (Vec<UnitId>, Vec<UnitId>) = family
                .partition(|&unit| unit == holder || inputs.targets.target(unit) == Some(slot));
            if gone.is_empty() {
                continue;
            }
            ships.retain(|unit| !gone.contains(unit));
            let held = Held::of(inputs, (reach, refs), clips, &kept);
            let mut by_character: BTreeMap<CharId, Vec<UnitWork>> = BTreeMap::new();
            for &unit in &gone {
                left.insert(unit, Left::OffTarget { target: holder });
                let work = held.work_of(inputs, (reach, refs), clips, unit);
                if let Some(work) = work {
                    by_character
                        .entry(m.units[unit].character)
                        .or_default()
                        .push(work);
                }
            }
            for (character, units) in by_character {
                found.push(WorkLeft {
                    character,
                    target: holder,
                    units,
                });
            }
        }
        found
    }

    /// Whether a unit ships.
    pub fn ships(&self, unit: UnitId) -> bool {
        self.ships.contains(&unit)
    }

    fn folded(inputs: &PortInputs<'_>, reach: &Reach, refs: &FileRefs, clips: &OwnClips) -> Folded {
        let m: &Mod = inputs.fixed;
        let facts = inputs.facts;
        let mut out = Folded {
            game: BTreeSet::new(),
            chromas: BTreeMap::new(),
        };
        let target = |unit: UnitId| inputs.targets.target(unit);
        let live: Vec<UnitId> = m.units.ids().filter(|&u| target(u).is_some()).collect();
        if live.len() < 2 {
            return out;
        }
        let is_game = |unit: UnitId| -> bool {
            let (Some(t), Some(reached)) = (target(unit), reach.of(unit)) else {
                return false;
            };
            let catalog = facts.catalogs.of(m.units[unit].character);
            let canon = |slot| catalog.map_or(slot, |catalog| catalog.canon(slot));
            // The game's objects of the target's skin, of this build or an older one, are
            // not the mod's: a stale copy is carried only because it differs from today's.
            let own_object = reached.objects.iter().any(|&(object, fate)| {
                fate != Fate::Game
                    && match facts.objects.origin(object) {
                        Some(ObjectOrigin::Own) | None => true,
                        Some(origin) => origin.slot().is_some_and(|slot| canon(slot) != canon(t)),
                    }
            });
            let own_file = refs.of(unit).is_some_and(|refs| {
                refs.refs.values().any(|verdict| {
                    matches!(verdict, FileVerdict::Own { file } if !inputs.files.is_game(*file))
                })
            });
            !own_object && !own_file
        };
        let game: BTreeSet<UnitId> = live.iter().copied().filter(|&u| is_game(u)).collect();
        for &unit in &game {
            let character = m.units[unit].character;
            let kept = live
                .iter()
                .filter(|other| !game.contains(other) && m.units[**other].character == character);
            let kept: Vec<UnitId> = kept.copied().collect();
            if kept.is_empty() {
                continue;
            }
            let carried: BTreeSet<_> = kept.iter().flat_map(|&other| clips.of(other)).collect();
            if clips.of(unit).is_subset(&carried) {
                out.game.insert(unit);
            }
        }

        let own_files = |unit: UnitId| -> BTreeSet<ContentId> {
            let Some(refs) = refs.of(unit) else {
                return BTreeSet::new();
            };
            let own = refs.refs.values().filter_map(|verdict| match verdict {
                FileVerdict::Own { file } if !inputs.files.is_game(*file) => {
                    Some(m.files[*file].content)
                }
                _ => None,
            });
            own.collect()
        };
        for &unit in &live {
            if out.game.contains(&unit) {
                continue;
            }
            let character = m.units[unit].character;
            let class = target(unit).and_then(|t| {
                let catalog = facts.catalogs.of(character)?;
                catalog.skin(t).map(|skin| skin.class)
            });
            let Some(SkinClass::Chroma(parent)) = class else {
                continue;
            };
            let of = live.iter().copied().find(|&other| {
                other != unit
                    && !out.game.contains(&other)
                    && m.units[other].character == character
                    && target(other) == Some(parent)
            });
            if let Some(of) = of
                && own_files(unit).is_subset(&own_files(of))
                && clips.of(unit).is_subset(&clips.of(of))
            {
                out.chromas.insert(unit, of);
            }
        }
        out
    }
}

/// What the units at a champion's target hold, by bytes: the files they ship, the bodies of
/// the objects they reach, and their own clips.
struct Held {
    files: BTreeSet<ContentId>,
    bodies: BTreeSet<ContentId>,
    clips: BTreeSet<ClipRef>,
}

impl Held {
    fn of(
        inputs: &PortInputs<'_>,
        (reach, refs): (&Reach, &FileRefs),
        clips: &OwnClips,
        units: &[UnitId],
    ) -> Self {
        let m: &Mod = inputs.fixed;
        let files = units.iter().flat_map(|&unit| shipped(refs, unit));
        let bodies = units.iter().flat_map(|&unit| reached(reach, unit));
        Self {
            files: files.map(|file| m.files[file].content).collect(),
            bodies: bodies.map(|object| m.objects[object].body).collect(),
            clips: units.iter().flat_map(|&unit| clips.of(unit)).collect(),
        }
    }

    /// What a unit holds of its own that these units do not hold, when it holds any.
    fn work_of(
        &self,
        inputs: &PortInputs<'_>,
        (reach, refs): (&Reach, &FileRefs),
        clips: &OwnClips,
        unit: UnitId,
    ) -> Option<UnitWork> {
        let m: &Mod = inputs.fixed;
        let slot = inputs.targets.target(unit);
        let mut work = UnitWork {
            unit,
            slot: slot.unwrap_or(m.units[unit].source_slot),
            kind: WorkKind::Unsupported,
            models: 0,
            skeletons: 0,
            textures: 0,
            files: 0,
            objects: 0,
            clips: 0,
        };
        for file in shipped(refs, unit) {
            if inputs.files.is_game(file) || self.files.contains(&m.files[file].content) {
                continue;
            }
            match m.files[file].kind {
                FileKind::Skn => work.models += 1,
                FileKind::Skl => work.skeletons += 1,
                FileKind::Tex | FileKind::Dds => work.textures += 1,
                _ => work.files += 1,
            }
        }
        let own = |object: &ObjectId| {
            let origin = inputs.facts.objects.origin(*object);
            matches!(origin, Some(ObjectOrigin::Own) | None)
        };
        let objects = reached(reach, unit).filter(own);
        work.objects = objects
            .filter(|object| !self.bodies.contains(&m.objects[*object].body))
            .count();
        work.clips = clips.of(unit).difference(&self.clips).count();
        let slot_work = work.models + work.skeletons + work.textures;
        if slot_work > 0 {
            work.kind = WorkKind::SlotWork;
        }
        let other = work.files + work.objects + work.clips;
        (slot_work + other > 0).then_some(work)
    }
}

/// The files a unit names that the mod ships, each once.
fn shipped(refs: &FileRefs, unit: UnitId) -> BTreeSet<FileId> {
    let Some(refs) = refs.of(unit) else {
        return BTreeSet::new();
    };
    let own = refs.refs.values().filter_map(|verdict| match verdict {
        FileVerdict::Own { file } => Some(*file),
        _ => None,
    });
    own.collect()
}

/// The objects a unit reaches that a port carries or writes: all but the game's as it is.
fn reached(reach: &Reach, unit: UnitId) -> impl Iterator<Item = ObjectId> + '_ {
    let objects = reach.of(unit).into_iter().flat_map(|found| &found.objects);
    objects
        .filter(|(_, fate)| *fate != Fate::Game)
        .map(|&(object, _)| object)
}

impl Dump for Shipping {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = String::new();
        let name = |unit: UnitId| {
            cx.module
                .map_or(String::new(), |m| format!(" {}", clean(&m.unit_name(unit))))
        };
        for &unit in &self.ships {
            let _ = writeln!(out, "ships {unit}{}", name(unit));
        }
        for (&unit, left) in &self.left {
            let _ = writeln!(out, "left {unit}{}: {}", name(unit), left.why.as_str());
        }
        out
    }
}
