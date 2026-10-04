//! Whether a unit's work fits a slot's rig: the mesh, the submeshes and the skeleton checked
//! against the game's skin at the slot, and the unit's clips sorted there (#ADR-0013). No
//! choice is made here. The clips are judged by the policy, at its level of reuse.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::ser::SerializeStruct as _;
use serde::{Serialize, Serializer};

use super::clips::{ClipJudge, ClipsAt, UnitClips};
use super::origin::Verdict;
use super::rig::{Rigs, Source, UnitRig, Visibility};
use crate::game::SkinCatalog;
use crate::model::entity::CharId;
use crate::model::{Slot, Spelled, clean};

/// What a check is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Aspect {
    /// Whether the game has the slot.
    Slot,
    /// The mesh.
    Mesh,
    /// The submeshes of an own mesh.
    Submeshes,
    /// The skeleton.
    Skeleton,
    /// The rig as a whole, for a unit that has none.
    Rig,
    /// The clips, under a level of reuse.
    Clips,
}

impl Aspect {
    /// The aspect's name in a dump and a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Aspect::Slot => "slot",
            Aspect::Mesh => "mesh",
            Aspect::Submeshes => "submeshes",
            Aspect::Skeleton => "skeleton",
            Aspect::Rig => "rig",
            Aspect::Clips => "clips",
        }
    }
}

/// A submesh copied from a premium skin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Copied {
    /// The submesh's name.
    pub name: Spelled,
    /// The skin it was copied from.
    pub from: Slot,
}

/// Slots as `3/9`.
fn slots_text(slots: &[Slot]) -> String {
    let slots: Vec<String> = slots.iter().map(Slot::to_string).collect();
    slots.join("/")
}

fn copied_text(copied: &[Copied]) -> String {
    let items: Vec<String> = copied
        .iter()
        .map(|c| format!("{:?} (skin {})", c.name.raw(), c.from))
        .collect();
    items.join(", ")
}

/// Why clips do not fit a slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ClipBlock {
    /// Clips of another skin that the level refuses.
    Refused {
        /// How many.
        clips: u32,
        /// The skin they are from.
        from: Slot,
    },
    /// The level accepts reuse only for the author's own rig and clips.
    NeedsOwnRig {
        /// Whether the mesh and the skeleton are the mod's own.
        own_rig: bool,
        /// The clips that are the mod's own.
        own: u32,
        /// The clips the unit plays.
        total: u32,
    },
    /// Own clips with no track on a joint of the skeleton.
    OwnOff {
        /// How many.
        clips: u32,
    },
}

impl fmt::Display for ClipBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClipBlock::Refused { clips, from } => write!(f, "{clips} clips of skin {from}"),
            ClipBlock::NeedsOwnRig {
                own_rig,
                own,
                total,
            } => {
                let rig = match own_rig {
                    true => "own mesh and skeleton",
                    false => "not its own mesh and skeleton",
                };
                write!(
                    f,
                    "reuse needs its own rig and half its clips: {rig}, {own} of {total} clips its own"
                )
            }
            ClipBlock::OwnOff { clips } => {
                write!(f, "{clips} own clips move none of the skeleton's joints")
            }
        }
    }
}

/// Why a check came out as it did. Its text is ltk-forge's detail of the same check, so the two
/// reports compare (#ADR-0004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Why {
    /// The game has no skin at the slot.
    NoSuchSkin {
        /// The slot.
        slot: Slot,
    },
    /// The entry names no mesh, or one nothing holds.
    NoMesh,
    /// The mesh is the slot's own.
    SlotMesh,
    /// The mesh is another skin's.
    OtherSkinMesh {
        /// The skins whose mesh it is, chromas counted as their parent.
        of: Vec<Slot>,
    },
    /// The mesh is the mod's own.
    OwnMesh,
    /// The mesh is a game file of no skin of the character.
    ForeignMesh,
    /// Submeshes copied from a premium skin are visible.
    CopiedVisible {
        /// The submeshes.
        submeshes: Vec<Copied>,
    },
    /// Submeshes copied from a premium skin are hidden, and an animation shows them.
    CopiedShown {
        /// The submeshes.
        submeshes: Vec<Copied>,
    },
    /// Submeshes copied from a premium skin are hidden and never shown: removing them fits.
    CopiedHidden {
        /// The submeshes.
        submeshes: Vec<Copied>,
    },
    /// No submesh is another skin's.
    NoCopiedSubmesh,
    /// The entry names no skeleton, or one nothing holds.
    NoSkeleton,
    /// The skeleton is the slot's own.
    SlotSkeleton,
    /// The skeleton has the slot's joints, in order.
    SlotJoints,
    /// The skeleton is another skin's, with other joints.
    OtherSkinSkeleton {
        /// The skins whose skeleton it is.
        of: Vec<Slot>,
        /// The slot judged.
        slot: Slot,
    },
    /// The mesh binds joints the mod's own skeleton does not have.
    Unbound,
    /// The skeleton is the mod's own.
    OwnSkeleton,
    /// The skeleton is a game file of no skin of the character.
    ForeignSkeleton,
    /// The unit has no skin entry that reads.
    NoEntry,
    /// The graph plays no clip, or was not found.
    NoClips {
        /// Whether the graph was found.
        graph_found: bool,
    },
    /// Clips do not fit.
    ClipsBlocked {
        /// Each reason.
        blocks: Vec<ClipBlock>,
    },
    /// The clips fit.
    ClipsFit {
        /// The clips that fit, reused ones included.
        fits: u32,
        /// The clips the unit plays.
        total: u32,
        /// The clips of other skins the level accepts.
        reused: u32,
        /// The level's name.
        level: &'static str,
        /// The clips nothing judges.
        unknown: u32,
    },
}

impl fmt::Display for Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Why::NoSuchSkin { slot } => write!(f, "the game has no skin {slot}"),
            Why::NoMesh => f.write_str("no mesh"),
            Why::SlotMesh => f.write_str("the slot's own mesh"),
            Why::OtherSkinMesh { of } => write!(f, "skin {}'s mesh", slots_text(of)),
            Why::OwnMesh => f.write_str("the mod's own mesh"),
            Why::ForeignMesh => f.write_str("a game mesh of no skin of this character"),
            Why::CopiedVisible { submeshes } => {
                write!(
                    f,
                    "visible submeshes copied from a premium skin: {}",
                    copied_text(submeshes)
                )
            }
            Why::CopiedShown { submeshes } => {
                write!(
                    f,
                    "props copied from a premium skin and shown by animation: {}",
                    copied_text(submeshes)
                )
            }
            Why::CopiedHidden { submeshes } => {
                write!(
                    f,
                    "remove never-shown copied submeshes: {}",
                    copied_text(submeshes)
                )
            }
            Why::NoCopiedSubmesh => f.write_str("no submesh of another skin"),
            Why::NoSkeleton => f.write_str("no skeleton"),
            Why::SlotSkeleton => f.write_str("the slot's own skeleton"),
            Why::SlotJoints => f.write_str("the slot's joints"),
            Why::OtherSkinSkeleton { of, slot } => {
                write!(
                    f,
                    "skin {}'s skeleton, other joints than skin {slot}'s",
                    slots_text(of)
                )
            }
            Why::Unbound => f.write_str("the mesh binds joints the skeleton does not have"),
            Why::OwnSkeleton => f.write_str("the mod's own skeleton"),
            Why::ForeignSkeleton => f.write_str("a game skeleton of no skin of this character"),
            Why::NoEntry => f.write_str("the unit has no readable skin entry"),
            Why::NoClips { graph_found: true } => f.write_str("the graph plays no clip"),
            Why::NoClips { graph_found: false } => f.write_str("the graph was not found"),
            Why::ClipsBlocked { blocks } => {
                let blocks: Vec<String> = blocks.iter().map(ClipBlock::to_string).collect();
                f.write_str(&blocks.join(", "))
            }
            Why::ClipsFit {
                fits,
                total,
                reused,
                level,
                unknown,
            } => {
                write!(f, "{fits} of {total} fit")?;
                if *reused > 0 {
                    write!(f, ", {reused} reused from other skins ({level})")?;
                }
                if *unknown > 0 {
                    write!(f, ", {unknown} not judged")?;
                }
                Ok(())
            }
        }
    }
}

/// How a check came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The work fits the slot.
    Fits,
    /// It fits once something is removed.
    FitsWithFix,
    /// It does not fit.
    DoesNotFit,
    /// Nothing judges it. It does not block, and does not count as a fit.
    Unknown,
}

impl Outcome {
    /// The outcome's mark in a dump.
    pub fn mark(self) -> &'static str {
        match self {
            Outcome::Fits => "fits",
            Outcome::FitsWithFix => "fix",
            Outcome::DoesNotFit => "NO",
            Outcome::Unknown => "?",
        }
    }
}

/// A check's result: how it came out, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fit {
    /// How it came out.
    pub outcome: Outcome,
    /// Why.
    pub why: Why,
}

impl Fit {
    fn new(outcome: Outcome, why: Why) -> Self {
        Self { outcome, why }
    }

    /// The work fits.
    pub fn fits(why: Why) -> Self {
        Self::new(Outcome::Fits, why)
    }

    /// The work fits once something is removed.
    pub fn with_fix(why: Why) -> Self {
        Self::new(Outcome::FitsWithFix, why)
    }

    /// The work does not fit.
    pub fn does_not(why: Why) -> Self {
        Self::new(Outcome::DoesNotFit, why)
    }

    /// Nothing judges it.
    pub fn unknown(why: Why) -> Self {
        Self::new(Outcome::Unknown, why)
    }

    /// Whether the check rules the slot out.
    pub fn blocks(&self) -> bool {
        self.outcome == Outcome::DoesNotFit
    }
}

impl Serialize for Fit {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_struct("Fit", 3)?;
        out.serialize_field("fit", &self.outcome)?;
        out.serialize_field("detail", &self.why.to_string())?;
        out.serialize_field("why", &self.why)?;
        out.end()
    }
}

/// One check of a unit at a slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RigCheck {
    /// What it is about.
    pub aspect: Aspect,
    /// How it came out.
    #[serde(flatten)]
    pub fit: Fit,
}

impl RigCheck {
    /// The check on one line of a dump. A submesh's name is the mod's text, and is cleaned.
    pub fn line(&self) -> String {
        let detail = self.fit.why.to_string();
        format!(
            "{:9} {:4} {}",
            self.aspect.as_str(),
            self.fit.outcome.mark(),
            clean(&detail)
        )
    }
}

/// Everything known about placing a unit at one slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SlotFacts {
    /// The slot.
    pub slot: Slot,
    /// The mesh, submesh and skeleton checks.
    pub checks: Vec<RigCheck>,
    /// The unit's clips there.
    pub clips: ClipsAt,
    /// Whether the game has a skin bin of the character at the slot.
    pub game_has_bin: bool,
}

/// Judges one unit at any slot, from the facts gathered of it.
#[derive(Clone, Copy)]
pub struct UnitJudge<'a> {
    pub(super) catalog: &'a SkinCatalog,
    pub(super) rigs: &'a Rigs,
    pub(super) character: CharId,
    pub(super) rig: Option<&'a UnitRig>,
    pub(super) clips: Option<&'a UnitClips>,
}

impl UnitJudge<'_> {
    /// The facts at slot `k`, which need not be a candidate: a person may force any slot.
    pub fn at(&self, k: Slot) -> SlotFacts {
        let game_has_bin = self.catalog.has_bin(k);
        match (self.rig, self.clips) {
            (Some(rig), Some(clips)) => {
                let judge = ClipJudge {
                    catalog: self.catalog,
                    rigs: self.rigs,
                    character: self.character,
                    rig,
                    clips,
                };
                SlotFacts {
                    slot: k,
                    checks: self.rig_checks(rig, k),
                    clips: judge.at(k),
                    game_has_bin,
                }
            }
            _ => SlotFacts {
                slot: k,
                checks: vec![RigCheck {
                    aspect: Aspect::Rig,
                    fit: Fit::unknown(Why::NoEntry),
                }],
                clips: ClipsAt::default(),
                game_has_bin,
            },
        }
    }

    /// Whether the unit's mesh, submeshes and skeleton fit slot `k`'s rig.
    fn rig_checks(&self, rig: &UnitRig, k: Slot) -> Vec<RigCheck> {
        let catalog = self.catalog;
        let Some(target) = catalog.skin(k) else {
            return vec![RigCheck {
                aspect: Aspect::Slot,
                fit: Fit::does_not(Why::NoSuchSkin { slot: k }),
            }];
        };
        let mut checks = Vec::new();

        let mesh_of = |slot: &Slot| catalog.skin(*slot).and_then(|skin| skin.mesh);
        let same_mesh = |slot: &Slot| mesh_of(slot).is_some() && mesh_of(slot) == target.mesh;
        let slots = &rig.mesh.file.game_slots;
        let mesh = match rig.mesh.file.source {
            Source::Missing => Fit::unknown(Why::NoMesh),
            _ if slots.iter().any(same_mesh) => Fit::fits(Why::SlotMesh),
            _ if !slots.is_empty() => {
                let of: BTreeSet<Slot> = slots.iter().map(|slot| catalog.canon(*slot)).collect();
                Fit::does_not(Why::OtherSkinMesh {
                    of: of.into_iter().collect(),
                })
            }
            _ if rig.mesh.file.own => Fit::fits(Why::OwnMesh),
            _ => Fit::unknown(Why::ForeignMesh),
        };
        checks.push(RigCheck {
            aspect: Aspect::Mesh,
            fit: mesh,
        });

        // #MOD-ARTIFACT-0024: submeshes copied whole from another skin.
        if rig.mesh.file.own {
            let mut by_visibility: BTreeMap<u8, Vec<Copied>> = BTreeMap::new();
            for submesh in &rig.mesh.submeshes {
                let Some(from) = submesh.copied_from().filter(|from| !same_mesh(from)) else {
                    continue;
                };
                let rank = match submesh.visibility {
                    Visibility::Visible => 0,
                    Visibility::HiddenShown => 1,
                    Visibility::HiddenNever => 2,
                };
                by_visibility.entry(rank).or_default().push(Copied {
                    name: submesh.name.clone(),
                    from,
                });
            }
            let fit = match by_visibility.into_iter().next() {
                Some((0, submeshes)) => Fit::does_not(Why::CopiedVisible { submeshes }),
                Some((1, submeshes)) => Fit::does_not(Why::CopiedShown { submeshes }),
                Some((_, submeshes)) => Fit::with_fix(Why::CopiedHidden { submeshes }),
                None => Fit::fits(Why::NoCopiedSubmesh),
            };
            checks.push(RigCheck {
                aspect: Aspect::Submeshes,
                fit,
            });
        }

        let target_joints = self.rigs.game_joints(self.character, k);
        let skeleton_of = |slot: &Slot| catalog.skin(*slot).and_then(|skin| skin.skeleton);
        let same_skeleton =
            |slot: &Slot| skeleton_of(slot).is_some() && skeleton_of(slot) == target.skeleton;
        // An own skeleton binds the mesh when it has every joint a vertex binds to.
        let bound = match (rig.mesh.max_blend, rig.skeleton.influences) {
            (Some(highest), Some(influences)) => usize::from(highest) < influences,
            _ => true,
        };
        let slots = &rig.skeleton.file.game_slots;
        let joints = rig.skeleton.joints.as_deref();
        let skeleton = match rig.skeleton.file.source {
            Source::Missing => Fit::unknown(Why::NoSkeleton),
            _ if slots.iter().any(same_skeleton) => Fit::fits(Why::SlotSkeleton),
            _ if joints.is_some() && joints == target_joints => Fit::fits(Why::SlotJoints),
            _ if !slots.is_empty() => Fit::does_not(Why::OtherSkinSkeleton {
                of: slots.iter().copied().collect(),
                slot: k,
            }),
            _ if rig.skeleton.file.own && !bound => Fit::does_not(Why::Unbound),
            _ if rig.skeleton.file.own => Fit::fits(Why::OwnSkeleton),
            _ => Fit::unknown(Why::ForeignSkeleton),
        };
        checks.push(RigCheck {
            aspect: Aspect::Skeleton,
            fit: skeleton,
        });
        checks
    }
}

/// What is known of placing one representative unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnitCandidates {
    /// The unit's origin.
    pub origin: Verdict,
    /// The premium skin the unit derives from, when its origin is one.
    pub premium: Option<Slot>,
    /// Whether the unit's cluster was written over other skins' slots.
    pub spread: bool,
    /// Whether the premium origin is on the base's mesh and skeleton: a paste of it fits the
    /// base as it is.
    pub base_rig_origin: bool,
    /// The facts at each candidate slot.
    pub slots: BTreeMap<Slot, SlotFacts>,
}
