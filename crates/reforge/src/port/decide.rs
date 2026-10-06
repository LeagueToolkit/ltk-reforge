//! The stage `decide`: which slot each representative unit targets, under one policy.
//!
//! - A unit in a premium slot that is no spread stays there (#ADR-0012).
//! - A skin0 unit, or a spread, goes to skin0 when its work fits the base's rig, else to the
//!   premium skin it derives from, else to the skin whose clips it mostly plays (#ADR-0013,
//!   #ADR-0015).
//! - When nothing fits, skin0 is kept and a person decides (#ADR-0016).
//! - A sub-character follows its champion (#ADR-0013, #ADR-0017).
//! - A unit made for a model before its rework gets no target (#ADR-0019).
//! - Of two units of one character on one slot, the one written for it keeps it, then a skin0
//!   unit, then one that is no spread; two of one rank leave it to the game (#ADR-0076).
//!
//! The decision reads the facts and nothing else. Its diagnostics are made from the decisions
//! afterwards, so a policy that is only compared raises none.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

use super::findings::{
    ForcedUnfit, Infeasible, PreRework, SubLeft, Superseded, TargetChosen, TargetConflict, Unjudged,
};
use super::{AnimReuse, Policy, TargetPolicy};
use crate::diag::Sink;
use crate::facts::{
    Aspect, ClipBlock, ClipsAt, Facts, Fit, RigCheck, RosterCharacter, RosterGroup, RosterUnit,
    SlotFacts, Verdict, Why,
};
use crate::model::entity::{CharId, UnitId};
use crate::model::{Dump, DumpCx, Slot, clean};

/// Why a unit goes where it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// A skin0 unit on the base.
    Base,
    /// A paste of a premium skin whose work fits the base: it stays on skin0.
    PasteCleaned,
    /// A unit in a premium slot stays in it.
    Slot,
    /// A skin0 unit whose work fits only its premium origin.
    Fallback,
    /// A skin0 unit that fits only the skin whose game clips it mostly plays.
    ClipsOrigin,
    /// A skin0 unit on a premium skin's skeleton file, which fits that skin.
    SkeletonOrigin,
    /// The policy named the slot.
    Forced,
    /// Nothing fits: skin0 is kept for a person to decide.
    Infeasible,
    /// A sub-character goes where its champion goes.
    SubFollows,
    /// A sub-character whose model does not fit the base is left to the game.
    SubLeftToBase,
    /// No rig facts: the unit stays at its slot, unjudged.
    Unjudged,
    /// Another unit of the character took the slot.
    Superseded,
    /// Units of the character of one rank target the slot: each leaves it to the game.
    Conflict,
    /// Made for the champion's model before its rework: left to the game.
    PreRework,
}

impl Reason {
    /// The reason's name in a dump and a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Base => "base",
            Reason::PasteCleaned => "paste-cleaned",
            Reason::Slot => "slot",
            Reason::Fallback => "fallback",
            Reason::ClipsOrigin => "clips-origin",
            Reason::SkeletonOrigin => "skeleton-origin",
            Reason::Forced => "forced",
            Reason::Infeasible => "infeasible",
            Reason::SubFollows => "sub-follows",
            Reason::SubLeftToBase => "sub-left-to-base",
            Reason::Unjudged => "unjudged",
            Reason::Superseded => "superseded",
            Reason::Conflict => "conflict",
            Reason::PreRework => "pre-rework",
        }
    }
}

/// Why a slot was weighed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Considered {
    /// A skin0 unit is tried on skin0 first.
    BaseFirst,
    /// The premium skin the unit derives from.
    PremiumOrigin,
    /// The skin whose clips the unit mostly plays.
    ClipsSkin,
    /// The skin whose skeleton file the unit takes.
    SkeletonSkin,
    /// The slot the unit's champion goes to.
    ChampionTarget,
    /// The slot the policy names.
    Policy,
}

impl Considered {
    /// The phrase a dump and a report give.
    pub fn as_str(self) -> &'static str {
        match self {
            Considered::BaseFirst => "skin0 first",
            Considered::PremiumOrigin => "the premium origin",
            Considered::ClipsSkin => "the skin whose clips it plays",
            Considered::SkeletonSkin => "the skin whose skeleton it takes",
            Considered::ChampionTarget => "the champion's target",
            Considered::Policy => "the policy",
        }
    }
}

impl Serialize for Considered {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// A slot the policy weighed, with every check judged under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    /// The slot.
    pub slot: Slot,
    /// Why it was weighed.
    pub why: Considered,
    /// The rig checks, then the clips under the policy's level.
    pub checks: Vec<RigCheck>,
    /// Whether no check rules the slot out.
    pub feasible: bool,
}

impl Candidate {
    fn judge(facts: &SlotFacts, why: Considered, anim: AnimReuse) -> Self {
        let mut checks = facts.checks.clone();
        checks.push(RigCheck {
            aspect: Aspect::Clips,
            fit: clip_fit(&facts.clips, anim),
        });
        Self {
            slot: facts.slot,
            why,
            feasible: !checks.iter().any(|check| check.fit.blocks()),
            checks,
        }
    }
}

/// A unit's clips at a slot, judged under a level of animation reuse (#ADR-0014).
pub fn clip_fit(at: &ClipsAt, anim: AnimReuse) -> Fit {
    if at.total == 0 {
        return Fit::unknown(Why::NoClips {
            graph_found: at.graph_found,
        });
    }
    // Reuse by joints: always under `allow`; under `own-rig` for an author's own rig and
    // clips, or for a few clips borrowed beside what the slot plays.
    let by_joints = match anim {
        AnimReuse::Allow => true,
        AnimReuse::OwnRig => at.own_work() || at.borrows_few(),
        AnimReuse::Block | AnimReuse::SameJoints => false,
    };
    let mut unknown = at.unknown;
    let mut reused = 0;
    let mut blocks = Vec::new();
    for from in &at.reused {
        let accepted = match anim {
            AnimReuse::SameJoints if from.same_joints => from.clips,
            // A clip whose joints did not read is not judged, and does not block.
            _ if by_joints => {
                unknown += from.joints_unknown;
                from.joints_fit + from.joints_extra
            }
            _ => 0,
        };
        reused += accepted;
        let refused = if by_joints {
            from.joints_off
        } else {
            from.clips - accepted
        };
        if refused > 0 {
            blocks.push(ClipBlock::Refused {
                clips: refused,
                from: from.from,
            });
        }
    }
    if anim == AnimReuse::OwnRig && !by_joints && !blocks.is_empty() {
        blocks.push(ClipBlock::NeedsOwnRig {
            own_rig: at.own_rig,
            own: at.own,
            borrowed: at.borrowed(),
            total: at.total,
        });
    }
    if at.own_off > 0 {
        blocks.push(ClipBlock::OwnOff { clips: at.own_off });
    }
    if !blocks.is_empty() {
        return Fit::does_not(Why::ClipsBlocked { blocks });
    }
    Fit::fits(Why::ClipsFit {
        fits: at.fits + reused,
        total: at.total,
        reused,
        level: anim.as_str(),
        unknown,
    })
}

/// What a rework says of a champion's unit that is left to the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MadeBefore {
    /// The champion.
    pub champion: CharId,
    /// The patch its new model shipped in.
    pub rework: String,
    /// The mod's files at the champion's paths the game dropped before the rework.
    pub older: u32,
    /// One of them.
    pub example: Option<crate::model::entity::FileId>,
}

/// The decision for one representative unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decision {
    /// The unit.
    pub unit: UnitId,
    /// Its character.
    pub character: CharId,
    /// The slot its bin sits in.
    pub source: Slot,
    /// The policy it was made under.
    pub policy: Policy,
    /// The target. None: the unit is left to the game.
    pub chosen: Option<Slot>,
    /// Why.
    pub reason: Reason,
    /// The unit's origin.
    pub origin: Verdict,
    /// The slots weighed, in the order weighed.
    pub candidates: Vec<Candidate>,
    /// Whether the premium origin is on the base's mesh and skeleton.
    pub base_rig_origin: bool,
    /// The chromas of the target.
    pub chroma_followers: Vec<Slot>,
    /// The target and the reason before another unit took the slot.
    pub placed: (Option<Slot>, Reason),
    /// The unit that took the slot, when one did.
    pub taken_by: Option<UnitId>,
    /// The premium skin the unit derives from.
    pub premium: Option<Slot>,
    /// How many of the unit's clips are the game clips of the skin it goes to by its clips.
    pub played: Option<u32>,
    /// Whether the work does not fit a target that was forced, by a person or by the champion.
    pub unfit: bool,
    /// The target of a sub-character at which the game has no skin bin of it.
    pub new_bin: Option<Slot>,
    /// What the rework says, for a champion's unit made before it.
    pub made_before: Option<MadeBefore>,
}

/// What another policy chooses for a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Alternative {
    /// The policy.
    pub policy: Policy,
    /// Its target.
    pub chosen: Option<Slot>,
    /// Its reason.
    pub reason: Reason,
}

/// Two units of one character on one slot, of the same rank.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Conflict {
    /// The character.
    pub character: CharId,
    /// The slot.
    pub slot: Slot,
    /// The units.
    pub units: Vec<UnitId>,
}

/// The decisions of one policy, and what the policies compared with it choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Targets {
    /// The policy.
    pub policy: Policy,
    /// One decision per representative unit.
    pub decisions: BTreeMap<UnitId, Decision>,
    /// The slots two units of the same rank target.
    pub conflicts: Vec<Conflict>,
    /// What each compared policy chooses, per unit.
    pub alternatives: BTreeMap<UnitId, Vec<Alternative>>,
}

/// The decisions of one group, as they are made.
struct Deciding<'a> {
    policy: Policy,
    facts: &'a Facts,
    group: &'a RosterGroup,
    /// The target of each of the champion's units, by source slot. A spread stands for the
    /// base too.
    champion_targets: BTreeMap<Slot, Option<Slot>>,
    /// The source slots of the champion's units made before its rework.
    pre_rework: Vec<Slot>,
}

impl Deciding<'_> {
    fn unit(&mut self, character: &RosterCharacter, unit: &RosterUnit) -> Option<Decision> {
        let facts = self.facts;
        let anim = self.policy.anim_reuse;
        let catalog = facts.catalogs.of(character.id)?;
        let known = facts.candidates.get(unit.id)?;
        let judge = facts.judge(character.id, unit.id)?;
        let at = |k: Slot| known.slots.get(&k).cloned().unwrap_or_else(|| judge.at(k));
        let s = unit.slot;
        let rig = facts.rigs.of(unit.id);
        let mut decision = Decision {
            unit: unit.id,
            character: character.id,
            source: s,
            policy: self.policy,
            chosen: None,
            reason: Reason::PreRework,
            origin: known.origin,
            candidates: Vec::new(),
            base_rig_origin: known.base_rig_origin,
            chroma_followers: Vec::new(),
            placed: (None, Reason::PreRework),
            taken_by: None,
            premium: known.premium,
            played: None,
            unfit: false,
            new_bin: None,
            made_before: None,
        };

        // A unit that carries its model is judged like any own rig. A sub-character goes as
        // its champion's unit does.
        let leans_on_old = match character.sub {
            false => !rig.is_some_and(|rig| rig.carries_model()),
            true => self.pre_rework.contains(&s) || self.pre_rework.contains(&Slot::BASE),
        };
        let before = facts
            .reworks
            .before(self.group.champion)
            .filter(|_| leans_on_old);
        let (chosen, reason) = if let Some(judged) = before {
            if !character.sub {
                self.pre_rework.push(s);
                decision.made_before = Some(MadeBefore {
                    champion: judged.character,
                    rework: judged.rework.clone(),
                    older: judged.older,
                    example: judged.example,
                });
            }
            (None, Reason::PreRework)
        } else if character.sub {
            let own = self.champion_targets.get(&s);
            let followed = own.or_else(|| self.champion_targets.get(&Slot::BASE));
            let t = followed.copied().flatten().unwrap_or(s);
            let there = at(t);
            if !there.game_has_bin {
                decision.new_bin = Some(t);
            }
            let candidate = Candidate::judge(&there, Considered::ChampionTarget, anim);
            let feasible = candidate.feasible;
            decision.candidates.push(candidate);
            match (feasible, t.is_base()) {
                (true, _) => (Some(t), Reason::SubFollows),
                (false, true) => (None, Reason::SubLeftToBase),
                (false, false) => {
                    decision.unfit = true;
                    (Some(t), Reason::SubFollows)
                }
            }
        } else {
            match self.policy.target {
                TargetPolicy::Auto if rig.is_none() => (Some(s), Reason::Unjudged),
                TargetPolicy::Auto if !s.is_base() && !known.spread => (Some(s), Reason::Slot),
                TargetPolicy::Auto => self.auto(&mut decision, known.premium, &at),
                forced => {
                    let t = match forced {
                        TargetPolicy::Base | TargetPolicy::Auto => Slot::BASE,
                        TargetPolicy::Origin => known.premium.unwrap_or(Slot::BASE),
                        TargetPolicy::Slot => s,
                        TargetPolicy::Fixed(slot) => slot,
                    };
                    let candidate = Candidate::judge(&at(t), Considered::Policy, anim);
                    decision.unfit = !candidate.feasible;
                    decision.candidates.push(candidate);
                    (Some(t), Reason::Forced)
                }
            }
        };
        if !character.sub {
            self.champion_targets.insert(s, chosen);
            if known.spread {
                self.champion_targets.entry(Slot::BASE).or_insert(chosen);
            }
        }
        decision.chosen = chosen;
        decision.reason = reason;
        decision.placed = (chosen, reason);
        decision.chroma_followers = chosen.map_or(Vec::new(), |t| catalog.chromas_of(t));
        Some(decision)
    }

    /// A skin0 unit or a spread under `auto`: skin0 first, then the premium origin, then the
    /// skin whose clips it mostly plays, else skin0 for a person to decide.
    fn auto(
        &self,
        decision: &mut Decision,
        premium: Option<Slot>,
        at: &dyn Fn(Slot) -> SlotFacts,
    ) -> (Option<Slot>, Reason) {
        let anim = self.policy.anim_reuse;
        let base_facts = at(Slot::BASE);
        let base = Candidate::judge(&base_facts, Considered::BaseFirst, anim);
        let fits_base = base.feasible;
        let skeleton_of = base.checks.iter().find_map(|check| match &check.fit.why {
            Why::PremiumSkeleton { of } => Some(of.clone()),
            _ => None,
        });
        decision.candidates.push(base);
        if fits_base {
            let reason = if premium.is_some() {
                Reason::PasteCleaned
            } else {
                Reason::Base
            };
            return (Some(Slot::BASE), reason);
        }
        if let Some(origin) = premium {
            let candidate = Candidate::judge(&at(origin), Considered::PremiumOrigin, anim);
            let feasible = candidate.feasible;
            decision.candidates.push(candidate);
            if feasible {
                return (Some(origin), Reason::Fallback);
            }
        }
        // One borrowed clip does not make a mod that skin's (#ADR-0015).
        let played = base_facts.clips.mostly_from();
        if let Some((skin, clips)) =
            played.filter(|(skin, _)| !skin.is_base() && Some(*skin) != premium)
        {
            let candidate = Candidate::judge(&at(skin), Considered::ClipsSkin, anim);
            let feasible = candidate.feasible;
            decision.candidates.push(candidate);
            if feasible {
                decision.played = Some(clips);
                return (Some(skin), Reason::ClipsOrigin);
            }
        }
        // #ADR-0062: the first skin of the skeleton's that the work fits.
        let weighed = |decision: &Decision, skin: Slot| {
            let mut candidates = decision.candidates.iter();
            candidates.any(|candidate| candidate.slot == skin)
        };
        for skin in skeleton_of.into_iter().flatten() {
            if weighed(decision, skin) {
                continue;
            }
            let candidate = Candidate::judge(&at(skin), Considered::SkeletonSkin, anim);
            let feasible = candidate.feasible;
            decision.candidates.push(candidate);
            if feasible {
                return (Some(skin), Reason::SkeletonOrigin);
            }
        }
        (Some(Slot::BASE), Reason::Infeasible)
    }
}

impl Policy {
    /// The stage `decide`: every representative unit's decision under this policy. A pure
    /// function of the facts.
    pub fn decide(&self, facts: &Facts) -> Targets {
        let mut decisions: BTreeMap<UnitId, Decision> = BTreeMap::new();
        let mut conflicts = Vec::new();
        for group in &facts.roster.groups {
            let mut deciding = Deciding {
                policy: *self,
                facts,
                group,
                champion_targets: BTreeMap::new(),
                pre_rework: Vec::new(),
            };
            for character in &group.characters {
                let representatives = character
                    .units
                    .iter()
                    .filter(|unit| facts.copies.is_representative(unit.id));
                for unit in representatives {
                    if let Some(decision) = deciding.unit(character, unit) {
                        decisions.insert(unit.id, decision);
                    }
                }
                conflicts.extend(supersede(facts, character, &mut decisions));
            }
        }
        Targets {
            policy: *self,
            decisions,
            conflicts,
            alternatives: BTreeMap::new(),
        }
    }
}

/// Leaves one unit on each slot two units of a character target: a unit whose skin entry reads
/// before one whose does not, then the unit written for the slot, then a skin0 unit, then one
/// that is no spread. The others get no target. When the best two tie, no unit keeps the slot
/// (#ADR-0076).
fn supersede(
    facts: &Facts,
    character: &RosterCharacter,
    decisions: &mut BTreeMap<UnitId, Decision>,
) -> Vec<Conflict> {
    let mut taken: BTreeMap<Slot, Vec<&RosterUnit>> = BTreeMap::new();
    for unit in &character.units {
        if let Some(t) = decisions.get(&unit.id).and_then(|decision| decision.chosen) {
            taken.entry(t).or_default().push(unit);
        }
    }
    let mut conflicts = Vec::new();
    for (t, mut units) in taken.into_iter().filter(|(_, units)| units.len() > 1) {
        let rank = |unit: &&RosterUnit| {
            let spread = facts.copies.is_spread(unit.id);
            let by_slot = decisions
                .get(&unit.id)
                .is_some_and(|d| d.reason == Reason::Slot);
            let written_for = by_slot || (unit.slot == t && !spread);
            (!unit.has_entry, !written_for, !unit.slot.is_base(), spread)
        };
        units.sort_by_key(rank);
        if rank(&units[0]) == rank(&units[1]) {
            for unit in &units {
                if let Some(decision) = decisions.get_mut(&unit.id) {
                    decision.chosen = None;
                    decision.reason = Reason::Conflict;
                }
            }
            conflicts.push(Conflict {
                character: character.id,
                slot: t,
                units: units.iter().map(|unit| unit.id).collect(),
            });
            continue;
        }
        let keeper = units[0].id;
        for loser in &units[1..] {
            if let Some(decision) = decisions.get_mut(&loser.id) {
                decision.chosen = None;
                decision.reason = Reason::Superseded;
                decision.taken_by = Some(keeper);
            }
        }
    }
    conflicts
}

impl Targets {
    /// Adds what other policies choose, each decided from the same facts.
    #[must_use]
    pub fn compared(mut self, facts: &Facts, policies: &[Policy]) -> Self {
        for policy in policies {
            for (unit, decision) in policy.decide(facts).decisions {
                self.alternatives
                    .entry(unit)
                    .or_default()
                    .push(Alternative {
                        policy: *policy,
                        chosen: decision.chosen,
                        reason: decision.reason,
                    });
            }
        }
        self
    }

    /// A unit's target. None for a copy, and for a unit left to the game.
    pub fn target(&self, unit: UnitId) -> Option<Slot> {
        self.decisions
            .get(&unit)
            .and_then(|decision| decision.chosen)
    }

    /// Raises what the decisions say, unit by unit, then the conflicts.
    pub fn report(&self, sink: &mut Sink<'_>) {
        for decision in self.decisions.values() {
            let unit = decision.unit;
            match decision.placed {
                (None, Reason::SubLeftToBase) => {
                    sink.raise(SubLeft { unit });
                }
                (None, _) => {
                    if let Some(made) = &decision.made_before {
                        sink.raise(PreRework {
                            unit,
                            champion: made.champion,
                            rework: made.rework.clone(),
                            older: made.older,
                            example: made.example,
                        });
                    }
                }
                (Some(slot), Reason::Unjudged) => {
                    sink.raise(Unjudged { unit, slot });
                }
                (Some(_), Reason::Infeasible) => {
                    sink.raise(Infeasible {
                        unit,
                        origin: decision.premium,
                    });
                }
                (Some(slot), reason) if decision.unfit => {
                    sink.raise(ForcedUnfit {
                        unit,
                        slot,
                        follows: reason == Reason::SubFollows,
                    });
                }
                (Some(_), Reason::Forced) => {}
                (Some(slot), reason) => {
                    sink.raise(TargetChosen {
                        unit,
                        slot,
                        reason,
                        origin: decision.premium,
                        clips: decision.played,
                    });
                }
            }
            if let (Some(by), (Some(slot), _)) = (decision.taken_by, decision.placed) {
                sink.raise(Superseded { unit, slot, by });
            }
        }
        for conflict in &self.conflicts {
            sink.raise(TargetConflict {
                character: conflict.character,
                slot: conflict.slot,
                units: conflict.units.clone(),
            });
        }
    }
}

impl Dump for Targets {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = String::new();
        let slot =
            |slot: Option<Slot>| slot.map_or("none".to_string(), |slot| format!("skin{slot}"));
        for decision in self.decisions.values() {
            let name = cx.module.map_or(String::new(), |m| {
                format!(" {}", clean(&m.unit_name(decision.unit)))
            });
            let base_rig = if decision.base_rig_origin {
                " origin-on-base-rig"
            } else {
                ""
            };
            let _ = writeln!(
                out,
                "target {}{name} = {} [{}] under {} origin={}{base_rig}",
                decision.unit,
                slot(decision.chosen),
                decision.reason.as_str(),
                decision.policy.label(),
                decision.origin.text()
            );
            for candidate in &decision.candidates {
                let feasible = if candidate.feasible {
                    "feasible"
                } else {
                    "not feasible"
                };
                let _ = writeln!(
                    out,
                    "   skin{} ({}) {feasible}",
                    candidate.slot,
                    candidate.why.as_str()
                );
                for check in &candidate.checks {
                    let _ = writeln!(out, "      {}", check.line());
                }
            }
            let others = self.alternatives.get(&decision.unit).into_iter().flatten();
            let differing = others
                .filter(|other| (other.chosen, other.reason) != (decision.chosen, decision.reason));
            let differing: Vec<String> = differing
                .map(|other| {
                    format!(
                        "{}: {} [{}]",
                        other.policy.label(),
                        slot(other.chosen),
                        other.reason.as_str()
                    )
                })
                .collect();
            if !differing.is_empty() {
                let _ = writeln!(out, "   other policies: {}", differing.join(", "));
            }
            if !decision.chroma_followers.is_empty() {
                let chromas: Vec<String> = decision
                    .chroma_followers
                    .iter()
                    .map(Slot::to_string)
                    .collect();
                let _ = writeln!(out, "   chromas of the target: {}", chromas.join("/"));
            }
        }
        out
    }
}
