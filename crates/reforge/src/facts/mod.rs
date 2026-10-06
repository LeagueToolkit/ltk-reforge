//! What is true of a fixed mod, gathered once in a fixed order: which units are copies, whose
//! its objects are, the skin each unit derives from, its rig, its clips at each slot it could
//! go to, whether it was made for a model before a rework, and which clips of its banks are
//! its own. Facts choose nothing
//! (#ADR-0010): a fact holds under every policy.
//!
//! Each result has a constructor that takes what it is found from, so the order of
//! [`Facts::gather`] is the dependency graph. The game is asked typed questions and no chunk
//! is read here (#ADR-0043).
//!
//! `docs/design/ARCHITECTURE.md`, "Facts".

pub mod clips;
pub mod copies;
mod findings;
pub mod judge;
pub mod objects;
pub mod origin;
pub mod rework;
pub mod rig;
pub mod sounds;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;

use serde::Serializer;

pub use self::clips::{Clips, ClipsAt, Reused, UnitClips};
pub use self::copies::{Copies, CopyKind, Member, NEAR_JACCARD, UnitRole};
pub use self::findings::{
    BankUnreadable, CODES, CatalogCapped, CopiedSubmesh, CopiesFound, DistinctUnits, NearCopy,
    OriginAmbiguous, OriginConflicting, OriginNone, RigPart, RigUnreadable, SubmeshesCopied,
    UnreadBank,
};
pub use self::judge::{
    Aspect, ClipBlock, Copied, Fit, Outcome, RigCheck, SlotFacts, UnitCandidates, UnitJudge, Why,
};
pub use self::objects::{ObjectOrigin, ObjectProvenance, Via};
pub use self::origin::{Evidence, Origin, OriginEvidence, OriginInputs, Origins, Verdict, Weight};
pub use self::rework::{Judged, Made, Reworks};
pub use self::rig::{COPIED_SHARE, RigInputs, Rigs, Source, UnitRig, Visibility};
pub use self::sounds::{
    BankFile, ClipOrigin, LeftOut, SoundClip, SoundInputs, SoundUnit, Sounds, UnitSounds,
};
use crate::diag::Sink;
use crate::env::Env;
use crate::fixups::FixedMod;
use crate::game::{GameError, MAX_CHAIN, SkinCatalog};
use crate::history::HistoryError;
use crate::load::{Dating, FileProvenance, ReworkTable};
use crate::model::entity::{CharId, CharKind, Mod, ObjectId, SkinUnit, UnitId};
use crate::model::{BinHash, Dump, DumpCx, SideTable, Slot, Spell, clean};

/// An input of the run that did not answer while the facts were gathered: a fault of the
/// machine or the tool, and no diagnostic.
#[derive(Debug, thiserror::Error)]
pub enum FactsError {
    /// A chunk the game holds did not read.
    #[error("the game did not read")]
    Game(#[from] GameError),
    /// The history's index did not answer a question.
    #[error("the history did not answer")]
    History(#[from] HistoryError),
}

fn entry_hex<S: Serializer>(entry: &BinHash, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(&format_args!("{:08x}", entry.0))
}

/// Every object the client loads for a unit, in load order, the first under each entry hash.
fn load_pool(m: &Mod, unit: &SkinUnit) -> Vec<ObjectId> {
    let mut seen = BTreeSet::new();
    let objects = unit.loads.iter().flat_map(|&bin| &m.bins[bin].objects);
    objects
        .copied()
        .filter(|&object| seen.insert(m.objects[object].entry))
        .collect()
}

/// The object of a unit's load under an entry hash.
fn pooled(m: &Mod, pool: &[ObjectId], entry: BinHash) -> Option<ObjectId> {
    pool.iter()
        .copied()
        .find(|&object| m.objects[object].entry == entry)
}

/// The game's skins of each character of the mod.
#[derive(Debug, Default)]
pub struct Catalogs {
    characters: BTreeMap<CharId, Arc<SkinCatalog>>,
}

impl Catalogs {
    /// The catalogs of a mod's characters, asked of the game.
    pub fn find(fixed: &FixedMod, env: &Env) -> Result<Self, FactsError> {
        let mut characters = BTreeMap::new();
        for (id, character) in fixed.characters.iter() {
            characters.insert(id, env.game().catalog(&character.name)?);
        }
        Ok(Self { characters })
    }

    /// A character's catalog.
    pub fn of(&self, character: CharId) -> Option<&SkinCatalog> {
        self.characters.get(&character).map(|catalog| &**catalog)
    }

    /// Raises the characters whose skins link more bins than a catalog reads.
    pub fn report(&self, sink: &mut Sink<'_>) {
        for (&character, catalog) in &self.characters {
            if !catalog.capped().is_empty() {
                sink.raise(CatalogCapped {
                    character,
                    slots: catalog.capped().iter().copied().collect(),
                    cap: MAX_CHAIN,
                });
            }
        }
    }
}

/// A unit as a policy needs to know it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RosterUnit {
    /// The unit.
    pub id: UnitId,
    /// The slot its bin sits in.
    pub slot: Slot,
    /// Whether it has a skin entry that reads.
    pub has_entry: bool,
}

/// A character with its units, by slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterCharacter {
    /// The character.
    pub id: CharId,
    /// Whether it is a sub-character.
    pub sub: bool,
    /// Its units.
    pub units: Vec<RosterUnit>,
}

/// A champion and its sub-characters, the champion first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterGroup {
    /// The champion.
    pub champion: CharId,
    /// The characters.
    pub characters: Vec<RosterCharacter>,
}

/// The groups, characters and units of the mod: what a policy walks in place of the mod.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roster {
    /// The groups, in the mod's order.
    pub groups: Vec<RosterGroup>,
}

impl Roster {
    fn of(m: &Mod) -> Self {
        let unit = |&id: &UnitId| RosterUnit {
            id,
            slot: m.units[id].source_slot,
            has_entry: m.units[id].entry.is_some(),
        };
        let character = |&id: &CharId| RosterCharacter {
            id,
            sub: m.characters[id].kind != CharKind::Champion,
            units: m.characters[id].units.iter().map(unit).collect(),
        };
        let group = |group: &crate::model::entity::Group| RosterGroup {
            champion: group.champion,
            characters: group.characters.iter().map(character).collect(),
        };
        Self {
            groups: m.groups.iter().map(group).collect(),
        }
    }
}

/// What the facts are gathered from, besides the mod.
pub struct FactInputs<'a> {
    /// The game and the history.
    pub env: &'a Env,
    /// The mod's dating.
    pub dating: &'a Dating,
    /// Whose bytes the mod's files are.
    pub files: &'a FileProvenance,
    /// The names of the run: they spell a file a bin names by its hash.
    pub names: &'a dyn Spell,
    /// The champions whose model was replaced.
    pub reworks: &'a ReworkTable,
}

/// What is true of a mod.
pub struct Facts {
    /// The groups, characters and units.
    pub roster: Roster,
    /// The game's skins of each character.
    pub catalogs: Catalogs,
    /// Which units are copies of which.
    pub copies: Copies,
    /// Whose the objects are.
    pub objects: ObjectProvenance,
    /// The skin each representative unit derives from.
    pub origins: Origins,
    /// The rig of each representative unit.
    pub rigs: Rigs,
    /// The clips of each representative unit, at each candidate slot.
    pub clips: Clips,
    /// The facts of placing each representative unit at each candidate slot.
    pub candidates: SideTable<SkinUnit, UnitCandidates>,
    /// The reworked champions, judged.
    pub reworks: Reworks,
    /// The banks of each representative unit's audio block, and whose their clips are.
    pub sounds: Sounds,
}

impl Facts {
    /// The stage `facts`: every result, in the order of what each is found from.
    pub fn gather(fixed: &FixedMod, inputs: &FactInputs<'_>) -> Result<Self, FactsError> {
        let _span = tracing::info_span!("facts").entered();
        let FactInputs {
            env,
            dating,
            files,
            names,
            reworks,
        } = *inputs;
        let catalogs = Catalogs::find(fixed, env)?;
        let copies = Copies::find(fixed, env)?;
        let objects = ObjectProvenance::find(fixed, env, &catalogs)?;
        let origin_inputs = OriginInputs {
            catalogs: &catalogs,
            copies: &copies,
            files,
            objects: &objects,
            names,
        };
        let origins = Origins::find(fixed, &origin_inputs);
        let rig_inputs = RigInputs {
            env,
            catalogs: &catalogs,
            copies: &copies,
            files,
            origins: &origins,
            names,
        };
        let rigs = Rigs::find(fixed, &rig_inputs)?;
        let clips = Clips::find(fixed, env, &catalogs, &copies, &origins, &rigs)?;
        let reworks = Reworks::find(fixed, env.history(), dating, reworks)?;
        let sound_inputs = SoundInputs {
            env,
            copies: &copies,
            catalogs: &catalogs,
            dating,
        };
        let sounds = Sounds::find(fixed, &sound_inputs)?;
        let mut facts = Self {
            roster: Roster::of(fixed),
            catalogs,
            copies,
            objects,
            origins,
            rigs,
            clips,
            candidates: SideTable::default(),
            reworks,
            sounds,
        };
        facts.candidates = facts.find_candidates(fixed);
        Ok(facts)
    }

    fn find_candidates(&self, m: &Mod) -> SideTable<SkinUnit, UnitCandidates> {
        let mut out = SideTable::default();
        for id in self.copies.representatives() {
            let (Some(judge), Some(catalog)) = (
                self.judge(m.units[id].character, id),
                self.catalogs.of(m.units[id].character),
            ) else {
                continue;
            };
            let premium = self.origins.premium(id);
            let rig = |slot: Slot| catalog.skin(slot).map(|skin| (skin.mesh, skin.skeleton));
            let base_rig_origin = premium.is_some_and(|origin| {
                catalog.base().is_some_and(|base| base.mesh.is_some())
                    && rig(Slot::BASE) == rig(origin)
            });
            let slots = clips::candidate_slots(m, catalog, &self.origins, &self.copies, id);
            let candidates = UnitCandidates {
                origin: self
                    .origins
                    .of(id)
                    .map_or(Verdict::Unknown, |origin| origin.verdict),
                premium,
                spread: self.copies.is_spread(id),
                base_rig_origin,
                slots: slots.into_iter().map(|k| (k, judge.at(k))).collect(),
            };
            out.insert(id, candidates);
        }
        out
    }

    /// Judges a unit of a character at any slot.
    pub fn judge(&self, character: CharId, unit: UnitId) -> Option<UnitJudge<'_>> {
        let characters = self
            .roster
            .groups
            .iter()
            .flat_map(|group| &group.characters);
        let sub = characters.into_iter().any(|c| c.id == character && c.sub);
        Some(UnitJudge {
            catalog: self.catalogs.of(character)?,
            rigs: &self.rigs,
            character,
            sub,
            rig: self.rigs.of(unit),
            clips: self.clips.of(unit),
        })
    }

    /// Raises what the facts found, in the order they were gathered.
    pub fn report(&self, sink: &mut Sink<'_>) {
        self.catalogs.report(sink);
        self.copies.report(sink);
        self.origins.report(sink);
        self.rigs.report(sink);
        self.sounds.report(sink);
    }
}

impl Dump for Facts {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = String::new();
        let Some(m) = cx.module else {
            return out;
        };
        for (unit, role) in self.copies.iter() {
            let UnitRole::Representative { members, spread } = role else {
                continue;
            };
            let slots: Vec<Slot> = members.iter().map(|member| member.slot).collect();
            let kinds: BTreeSet<&str> = members.iter().map(|member| member.kind.as_str()).collect();
            let kinds: Vec<&str> = kinds.into_iter().collect();
            let spread = if *spread { " spread" } else { "" };
            let _ = writeln!(
                out,
                "copies {unit} {} representative of {} ({}) at [{}]{spread}",
                clean(&m.unit_name(unit)),
                members.len(),
                kinds.join(", "),
                copies::compact(&slots)
            );
        }
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for (_, origin) in self.objects.iter() {
            let place = |via: &Via| match via {
                Via::Install => "install",
                Via::History { .. } => "history",
            };
            let slot = origin
                .slot()
                .map_or(String::new(), |slot| format!(", skin {slot}"));
            let key = match origin {
                ObjectOrigin::GameExact { via, .. } => format!("game exact ({}){slot}", place(via)),
                ObjectOrigin::GameRenamed { via, .. } => {
                    format!("game renamed ({}){slot}", place(via))
                }
                ObjectOrigin::Own => "own".to_string(),
            };
            *counts.entry(key).or_default() += 1;
        }
        for (key, n) in counts {
            let _ = writeln!(out, "objects {key}: {n}");
        }
        out.push_str(&self.origins.dump_text(m, cx.names));
        for (unit, rig) in self.rigs.iter() {
            let source = |file: &rig::Named| match file.source {
                Source::Mod(_) if file.own => "own",
                Source::Mod(_) => "game copy",
                Source::Game if file.moved.is_some() => "game (moved)",
                Source::Game => "game",
                Source::Missing => "missing",
            };
            let slots = |slots: &BTreeSet<Slot>| {
                let slots: Vec<String> = slots.iter().map(Slot::to_string).collect();
                slots.join("/")
            };
            let joints = rig.skeleton.joints.as_ref().map_or(0, Vec::len);
            let own = rig.clips.iter().filter(|clip| clip.file.own).count();
            let _ = writeln!(
                out,
                "rig {unit} mesh={} [{}] submeshes={} skeleton={} [{}] joints={joints} clips={} own={own} graph={}",
                source(&rig.mesh.file),
                slots(&rig.mesh.file.game_slots),
                rig.mesh.submeshes.len(),
                source(&rig.skeleton.file),
                slots(&rig.skeleton.file.game_slots),
                rig.clips.len(),
                match rig.graph {
                    Some(rig::GraphFrom::Archive) => "archive",
                    Some(rig::GraphFrom::Game) => "game",
                    None => "none",
                }
            );
        }
        for (unit, candidates) in self.candidates.iter() {
            let base_rig = if candidates.base_rig_origin {
                " origin-on-base-rig"
            } else {
                ""
            };
            let _ = writeln!(out, "candidates {unit}{base_rig}");
            for facts in candidates.slots.values() {
                let bin = if facts.game_has_bin {
                    ""
                } else {
                    " (no game bin)"
                };
                let _ = writeln!(out, "   skin{}{bin}", facts.slot);
                for check in &facts.checks {
                    let _ = writeln!(out, "      {}", check.line());
                }
                let reused: Vec<String> = facts
                    .clips
                    .reused
                    .iter()
                    .map(|r| format!("{} of skin {}", r.clips, r.from))
                    .collect();
                let reused = if reused.is_empty() {
                    String::new()
                } else {
                    format!(", reused: {}", reused.join(", "))
                };
                let _ = writeln!(
                    out,
                    "      clips     {} of {} fit{reused}",
                    facts.clips.fits, facts.clips.total
                );
            }
        }
        for judged in self.reworks.iter() {
            let made = match judged.made {
                Made::Before => "before",
                Made::After => "after",
                Made::Unknown => "unknown",
            };
            let _ = writeln!(
                out,
                "rework {} {} made={made} older={} newer={}",
                judged.character, judged.rework, judged.older, judged.newer
            );
        }
        out.push_str(&self.sounds.dump(cx));
        out
    }
}
