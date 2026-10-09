//! The choice of each unit's target slot: a policy reads the facts and chooses (#ADR-0010).
//! It is neither part of reading the mod nor part of writing the output.
//!
//! `docs/design/ARCHITECTURE.md`, "Decide".

mod audio;
mod base;
mod decide;
mod findings;
mod hud;
mod layout;
mod plan;
mod reach;
mod refs;
mod renames;
mod shipping;

use std::fmt;

use serde::{Serialize, Serializer};

pub use self::audio::{AudioPlan, BankUnit, ClipRef, MadeBank, OwnClips, UnitAudio};
pub use self::base::{Base, BaseObject, BaseObjects};
pub use self::decide::{
    Alternative, Candidate, Conflict, Considered, Decision, MadeBefore, Reason, Targets, clip_fit,
};
pub use self::findings::{
    AudioCarried, AudioNotCarried, AudioUnitsLeftOut, CODES, Carried, CarriedBank, CarriedOnce,
    Carry, CopiedName, ForcedUnfit, GameCarried, HudCarried, Infeasible, LeftRow, LinkDangling,
    LiteralsRenamed, NotCarried, ObjectsCopied, PreRework, RefDangling, RefUnmounted, Retargeted,
    SubLeft, Superseded, TargetChosen, TargetConflict, UnitWork, UnitsLeft, Unjudged, Unreached,
    WorkKind, WorkLeft,
};
pub use self::hud::{FieldEdit, HudEdits, HudPlan};
pub use self::layout::{Layout, MadeNames, folder_name, tidy_stem};
pub use self::plan::{Objects, PlanError, Port, PortInputs, PortOptions, PortUnit};
pub use self::reach::{Copied, Fate, Reach, SHADERS_BIN, UnitReach};
pub use self::refs::{
    ALWAYS_MOUNTED, CARRIED_FIELDS, FileRefs, FileVerdict, ICON_FIELDS, UnitRefs,
};
pub use self::renames::{Group, Renames, Retarget};
pub use self::shipping::{Left, LeftUnit, Shipping};
use crate::model::Slot;

/// A word that names no policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    /// No way to choose a target has the name.
    #[error("{0:?} is no target: auto, base, origin, slot or a skin number")]
    Target(String),
    /// No level of animation reuse has the name.
    #[error("{0:?} is no level of animation reuse: block, same-joints, own-rig or allow")]
    AnimReuse(String),
}

/// How the target slot is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum TargetPolicy {
    /// By the facts: #ADR-0012, #ADR-0013, #ADR-0015, #ADR-0016.
    #[default]
    Auto,
    /// Always skin0.
    Base,
    /// The premium skin the unit derives from, else skin0.
    Origin,
    /// The slot the unit sits in.
    Slot,
    /// This slot.
    Fixed(Slot),
}

impl TargetPolicy {
    /// The policy a word names: `auto`, `base`, `origin`, `slot` or a skin number.
    pub fn parse(text: &str) -> Result<Self, PolicyError> {
        Ok(match text {
            "auto" => TargetPolicy::Auto,
            "base" => TargetPolicy::Base,
            "origin" => TargetPolicy::Origin,
            "slot" => TargetPolicy::Slot,
            number => {
                let slot = number
                    .parse()
                    .map_err(|_| PolicyError::Target(text.to_string()))?;
                TargetPolicy::Fixed(Slot(slot))
            }
        })
    }
}

impl fmt::Display for TargetPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetPolicy::Auto => f.write_str("auto"),
            TargetPolicy::Base => f.write_str("base"),
            TargetPolicy::Origin => f.write_str("origin"),
            TargetPolicy::Slot => f.write_str("slot"),
            TargetPolicy::Fixed(slot) => write!(f, "{slot}"),
        }
    }
}

impl Serialize for TargetPolicy {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// Whether a slot fits when the unit's graph plays game clips that only other skins play
/// (#ADR-0014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnimReuse {
    /// Such a clip blocks the slot.
    Block,
    /// It fits when the skin it is from has the slot's skeleton joints.
    SameJoints,
    /// As `Allow` for an author's own rig and animation set, and for a few borrowed clips.
    /// Otherwise as `Block`.
    #[default]
    OwnRig,
    /// It fits when it moves some joint of the unit's skeleton.
    Allow,
}

impl AnimReuse {
    /// Every level, the strictest first.
    pub const ALL: [AnimReuse; 4] = [
        AnimReuse::Block,
        AnimReuse::SameJoints,
        AnimReuse::OwnRig,
        AnimReuse::Allow,
    ];

    /// The level a word names.
    pub fn parse(text: &str) -> Result<Self, PolicyError> {
        let named = Self::ALL.into_iter().find(|level| level.as_str() == text);
        named.ok_or_else(|| PolicyError::AnimReuse(text.to_string()))
    }

    /// The level's name.
    pub fn as_str(self) -> &'static str {
        match self {
            AnimReuse::Block => "block",
            AnimReuse::SameJoints => "same-joints",
            AnimReuse::OwnRig => "own-rig",
            AnimReuse::Allow => "allow",
        }
    }
}

/// One policy: how the target is chosen, and how much animation reuse is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize)]
pub struct Policy {
    /// How the target is chosen.
    pub target: TargetPolicy,
    /// The level of animation reuse.
    pub anim_reuse: AnimReuse,
}

impl Policy {
    /// The policy as `auto/own-rig`.
    pub fn label(&self) -> String {
        format!("{}/{}", self.target, self.anim_reuse.as_str())
    }

    /// The same target policy at each other level of animation reuse: what a report shows
    /// beside the policy's own choice (#ADR-0014).
    pub fn other_levels(&self) -> Vec<Policy> {
        let others = AnimReuse::ALL
            .into_iter()
            .filter(|level| *level != self.anim_reuse);
        others
            .map(|anim_reuse| Policy {
                anim_reuse,
                ..*self
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_policy_is_named_by_its_words() {
        assert_eq!(TargetPolicy::parse("auto"), Ok(TargetPolicy::Auto));
        assert_eq!(TargetPolicy::parse("7"), Ok(TargetPolicy::Fixed(Slot(7))));
        assert!(TargetPolicy::parse("premium").is_err());
        assert_eq!(AnimReuse::parse("same-joints"), Ok(AnimReuse::SameJoints));
        assert!(AnimReuse::parse("some").is_err());
        assert_eq!(Policy::default().label(), "auto/own-rig");
        assert_eq!(Policy::default().other_levels().len(), 3);
    }
}
