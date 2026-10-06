//! What a decision and a plan say: one type per code of `docs/design/CODES.md`, "copies,
//! origin, rig, target", "unit, object, ref" and "audio, hud".

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde::{Serialize, Serializer};

use super::decide::Reason;
use super::shipping::Left;
use crate::diag::{Channel, Code, Diag, Message, Record, Severity, Subject};
use crate::facts::LeftOut;
use crate::model::entity::{CharId, FileId, UnitId};
use crate::model::{BinHash, Layer, Slot, WadHash};

/// Every diagnostic code a decision and a plan raise.
pub const CODES: &[Code] = &[
    UnitsLeft::CODE,
    Retargeted::CODE,
    GameCarried::CODE,
    LinkDangling::CODE,
    Unreached::CODE,
    RefDangling::CODE,
    RefUnmounted::CODE,
    Carried::CODE,
    HudCarried::CODE,
    AudioCarried::CODE,
    AudioNotCarried::CODE,
    AudioUnitsLeftOut::CODE,
    LiteralsRenamed::CODE,
    TargetChosen::CODE,
    ForcedUnfit::CODE,
    Superseded::CODE,
    TargetConflict::CODE,
    SubLeft::CODE,
    Unjudged::CODE,
    Infeasible::CODE,
    PreRework::CODE,
];

const fn code(name: &'static str, severity: Severity) -> Code {
    Code::new(name, Channel::Decision, severity)
}

/// A unit's target, and the reason for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetChosen {
    /// The unit.
    pub unit: UnitId,
    /// The slot it goes to.
    pub slot: Slot,
    /// Why.
    pub reason: Reason,
    /// The premium skin the unit derives from, when its origin is one.
    pub origin: Option<Slot>,
    /// For a unit that goes to the skin whose clips it plays: how many of its clips are that
    /// skin's.
    pub clips: Option<u32>,
}

impl Diag for TargetChosen {
    const CODE: Code = code("target.chosen", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let slot = self.slot;
        let _ = match (self.reason, self.origin) {
            (Reason::PasteCleaned, Some(origin)) => write!(
                f,
                "pasted from skin {origin}, and its work fits the base: stays on skin0"
            ),
            (Reason::Slot, _) => write!(f, "a mod for skin{slot}: stays on skin{slot}"),
            (Reason::Fallback, _) => {
                write!(f, "its work only fits skin {slot}: a mod for skin{slot}")
            }
            (Reason::ClipsOrigin, _) => write!(
                f,
                "plays skin {slot}'s animations ({} of its clips) and fits nothing else: a mod for skin{slot}",
                self.clips.unwrap_or(0)
            ),
            (Reason::SkeletonOrigin, _) => write!(
                f,
                "on skin {slot}'s skeleton file, which the base does not take: a mod for skin{slot}"
            ),
            (Reason::SubFollows, _) => write!(f, "follows the champion to skin{slot}"),
            _ => write!(f, "skin{slot}"),
        };
    }
}

/// A target a person or the champion forced, which the work does not fit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForcedUnfit {
    /// The unit.
    pub unit: UnitId,
    /// The slot.
    pub slot: Slot,
    /// Whether the unit follows its champion there, and no person asked.
    pub follows: bool,
}

impl Diag for ForcedUnfit {
    const CODE: Code = code("target.forced-unfit", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let slot = self.slot;
        let _ = match self.follows {
            true => write!(
                f,
                "follows the champion to skin{slot} though its model does not fit there"
            ),
            false => write!(f, "skin{slot} as asked, though the work does not fit it"),
        };
    }
}

/// Another unit of the character took the slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Superseded {
    /// The unit left without a target.
    pub unit: UnitId,
    /// The slot.
    pub slot: Slot,
    /// The unit that keeps it.
    pub by: UnitId,
}

impl Diag for Superseded {
    const CODE: Code = code("target.superseded", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(f, "skin{} is taken by ", self.slot);
        f.unit(self.by);
    }
}

/// Two units of one character, of the same rank, end on one slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetConflict {
    /// The character.
    pub character: CharId,
    /// The slot.
    pub slot: Slot,
    /// The units that target it, the best ranked first.
    pub units: Vec<UnitId>,
}

impl Diag for TargetConflict {
    const CODE: Code = code("target.conflict", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Character(self.character)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(f, "{} units target skin{}: ", self.units.len(), self.slot);
        f.list(&self.units, |f, unit| {
            f.unit(*unit);
        });
    }
}

/// A sub-character whose model does not fit the base, left to the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubLeft {
    /// The unit.
    pub unit: UnitId,
}

impl Diag for SubLeft {
    const CODE: Code = code("target.sub-left", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("its model does not fit the base; left to the game's base");
    }
}

/// A unit with no rig facts, left at its slot unjudged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unjudged {
    /// The unit.
    pub unit: UnitId,
    /// Its slot.
    pub slot: Slot,
}

impl Diag for Unjudged {
    const CODE: Code = code("target.unjudged", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "no rig facts (its skin bin did not read): left at skin{} unjudged",
            self.slot
        );
    }
}

/// A unit whose work fits no slot. It is written at skin0 for a person to decide (#ADR-0016).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Infeasible {
    /// The unit.
    pub unit: UnitId,
    /// The premium skin the unit derives from, which it does not fit either.
    pub origin: Option<Slot>,
}

impl Diag for Infeasible {
    const CODE: Code = code("target.infeasible", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = match self.origin {
            Some(origin) => write!(
                f,
                "fits neither skin0 nor its origin skin {origin}; skin0 is kept for a person to decide"
            ),
            None => write!(
                f,
                "does not fit skin0 and no premium origin is known; skin0 is kept for a person to decide"
            ),
        };
    }
}

/// A unit made for its champion's model before the rework, left to the game (#ADR-0019).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreRework {
    /// The unit.
    pub unit: UnitId,
    /// The champion.
    pub champion: CharId,
    /// The patch the champion's new model shipped in.
    pub rework: String,
    /// The mod's files at the champion's paths the game dropped before the rework.
    pub older: u32,
    /// One of them.
    pub example: Option<FileId>,
}

impl Diag for PreRework {
    const CODE: Code = code("target.pre-rework", Severity::Error);
    const RECORD: Option<Record> = Some(Record::game_change(1));

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("made for ").character(self.champion);
        let _ = write!(
            f,
            "'s model before the rework of {}, which replaced its mesh, skeleton and effects ({} files at paths dropped before it",
            self.rework, self.older
        );
        if let Some(example) = self.example {
            f.text(", such as ").file(example);
        }
        f.text("); left to the game");
    }
}

const fn plan(name: &'static str) -> Code {
    Code::new(name, Channel::Output, Severity::Note)
}

/// An entry hash as eight hex digits.
pub(super) fn entry_hex<S: Serializer>(hash: &BinHash, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(&format_args!("{:08x}", hash.0))
}

/// Entry hashes as eight hex digits each.
pub(super) fn hashes_hex<S: Serializer>(
    hashes: &BTreeSet<BinHash>,
    s: S,
) -> Result<S::Ok, S::Error> {
    s.collect_seq(hashes.iter().map(|hash| format!("{:08x}", hash.0)))
}

/// Entry or field hashes as eight hex digits each, in the order held.
pub(super) fn hashes_hex_list<S: Serializer>(hashes: &[BinHash], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(hashes.iter().map(|hash| format!("{:08x}", hash.0)))
}

fn paths_hex<S: Serializer>(hashes: &[WadHash], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(hashes.iter().map(|hash| format!("{:016x}", hash.0)))
}

/// A unit that leaves the port, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LeftRow {
    /// The unit.
    pub unit: UnitId,
    /// Why it leaves.
    #[serde(flatten)]
    pub why: Left,
}

/// The units of a character that leave the port (#ADR-0020).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnitsLeft {
    /// The character.
    pub character: CharId,
    /// Its units that leave, in unit order.
    pub units: Vec<LeftRow>,
}

impl Diag for UnitsLeft {
    const CODE: Code = plan("unit.left");

    fn subject(&self) -> Subject {
        Subject::Character(self.character)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.units.len(), "unit").text(" leave the port: ");
        f.list(&self.units, |f, row| {
            f.unit(row.unit).text(" (").text(row.why.as_str()).text(")");
        });
    }
}

/// A unit whose names moved from its slot to its target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Retargeted {
    /// The unit.
    pub unit: UnitId,
    /// The slot its bin sits in.
    pub from: Slot,
    /// Its target.
    pub to: Slot,
}

impl Diag for Retargeted {
    const CODE: Code = plan("unit.retargeted");

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(f, "skin{} names become skin{}'s", self.from, self.to);
    }
}

/// Game objects a unit carries because they name files the mod overrides at the game's paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameCarried {
    /// The unit.
    pub unit: UnitId,
    /// How many objects.
    pub objects: usize,
}

impl Diag for GameCarried {
    const CODE: Code = plan("object.game-carried");

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.objects, "object")
            .text(" of the game's carried: they name files the mod overrides at the game's paths");
    }
}

/// Names a unit's entry reaches that nothing loaded at its target holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkDangling {
    /// The unit.
    pub unit: UnitId,
    /// Its target.
    pub slot: Slot,
    /// The names, by their hashes.
    #[serde(serialize_with = "hashes_hex_list")]
    pub names: Vec<BinHash>,
}

impl Diag for LinkDangling {
    const CODE: Code = plan("object.link-dangling");

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.names.len(), "name");
        let _ = write!(f, " resolve in nothing loaded at skin{}: ", self.slot);
        f.list(&self.names, |f, name| {
            f.entry(*name);
        });
    }
}

/// Objects of a unit's load that its entry does not reach. They are not carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unreached {
    /// The unit.
    pub unit: UnitId,
    /// How many objects.
    pub objects: usize,
}

impl Diag for Unreached {
    const CODE: Code = Code::new("object.unreached", Channel::Input, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.objects, "object")
            .text(" of the load reached by nothing; not carried");
    }
}

/// Files a unit's port names that nothing holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefDangling {
    /// The unit.
    pub unit: UnitId,
    /// The files, by their path hashes.
    #[serde(serialize_with = "paths_hex")]
    pub files: Vec<WadHash>,
    /// How many of them the game shipped at earlier builds.
    pub former: usize,
}

impl Diag for RefDangling {
    const CODE: Code = plan("ref.dangling");

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file")
            .text(" named that nothing holds");
        if self.former > 0 {
            let _ = write!(
                f,
                " ({} of them the game shipped at earlier builds)",
                self.former
            );
        }
        f.text(": ");
        f.list(&self.files, |f, file| {
            f.path(*file);
        });
    }
}

/// Files a unit's port names that only a WAD not mounted with the champion holds
/// (#GAME-BEHAVIOR-0007).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RefUnmounted {
    /// The unit.
    pub unit: UnitId,
    /// The files, by their path hashes.
    #[serde(serialize_with = "paths_hex")]
    pub files: Vec<WadHash>,
}

impl Diag for RefUnmounted {
    const CODE: Code = plan("ref.unmounted");
    const RECORD: Option<Record> = Some(Record::game_behavior(7));

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file")
            .text(" named that only a WAD not mounted with the champion holds: ");
        f.list(&self.files, |f, file| {
            f.path(*file);
        });
    }
}

/// What a port carries under its namespace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Carried {
    /// Objects carried under a name of their own.
    pub objects: usize,
    /// Files shipped.
    pub files: usize,
    /// References pointed at a file of the game's.
    pub repointed: usize,
}

impl Diag for Carried {
    const CODE: Code = plan("object.carried");

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.objects, "object")
            .text(" carried under the namespace, ")
            .count(self.files, "file")
            .text(" shipped, ")
            .count(self.repointed, "reference")
            .text(" pointed at the game's files");
    }
}

/// The spell icons and HUD textures of a champion shipped in the HUD layer (#ADR-0029).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HudCarried {
    /// The champion.
    pub character: CharId,
    /// How many files ship.
    pub files: usize,
    /// How many objects of the root bin name them.
    pub objects: usize,
    /// How many fields are edited.
    pub records: usize,
    /// How many objects are left out, a field on the way to the file unnamed.
    pub unspelled: usize,
}

impl Diag for HudCarried {
    const CODE: Code = plan("hud.carried");
    const RECORD: Option<Record> = Some(Record::game_behavior(4));

    fn subject(&self) -> Subject {
        Subject::Character(self.character)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files, "spell icon")
            .text(" and HUD textures the mod overrides, named by ")
            .count(self.objects, "object")
            .text(" of the root bin (")
            .count(self.records, "record")
            .text("), in the layer hud: they show for the champion under every skin");
        if self.unspelled > 0 {
            f.text("; ")
                .count(self.unspelled, "object")
                .text(" left out, a field on the way unnamed");
        }
    }
}

/// How the clips of a carried bank play.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum Carry {
    /// The bank sits at a path the game's units at the target load: the game's events play
    /// its clips, and the port's events bank holds nothing.
    Overlay,
    /// A unit of its own: the events that reach an own clip, cut from an events bank.
    Own {
        /// The tag its events are renamed under.
        tag: String,
        /// How many events of the source are kept.
        events: usize,
        /// Whether the events are cut from the mod's own events bank (#ADR-0030).
        from_mod: bool,
        /// How many kept events of the mod's bank the game's bank at the path defines too.
        shared: usize,
        /// How many sounds under the kept events play nothing: their clips are the game's.
        silent: usize,
        /// The kept events no unit's list names, by id: they keep their ids.
        unnamed: Vec<u32>,
    },
}

/// One bank of the mod whose own clips a port carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CarriedBank {
    /// The mod's bank unit.
    pub source: String,
    /// The bank unit the port adds.
    pub name: String,
    /// How many own clips it holds.
    pub clips: usize,
    /// The layer it goes to.
    pub layer: Layer,
    /// How its clips play.
    pub carry: Carry,
    /// The game's unit it stands in for (#MOD-ARTIFACT-0028).
    pub replaces: Option<String>,
}

/// A locale WAD's copy of a voice bank that holds the champion WAD's own clips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CarriedOnce {
    /// The mod's bank unit.
    pub source: String,
    /// The locale of the copy.
    pub locale: String,
}

/// The own clips of a unit the port carries, in bank units after the game's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioCarried {
    /// The unit.
    pub unit: UnitId,
    /// The banks carried, in the order of the mod's audio block.
    pub banks: Vec<CarriedBank>,
    /// The locale copies carried once, from the champion's WAD.
    pub once: Vec<CarriedOnce>,
}

impl Diag for AudioCarried {
    const CODE: Code = plan("audio.carried");

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let clips: usize = self.banks.iter().map(|bank| bank.clips).sum();
        f.count(clips, "own clip")
            .text(" carried in ")
            .count(self.banks.len(), "unit")
            .text(" after the game's");
        if !self.banks.is_empty() {
            f.text(": ");
        }
        f.list(&self.banks, |f, bank| {
            let _ = write!(f, "{} of ", bank.clips);
            f.spelled(&bank.source).text(" as ").spelled(&bank.name);
            match &bank.carry {
                Carry::Overlay => {
                    f.text(" (the game's events play them)");
                }
                Carry::Own {
                    tag,
                    events,
                    from_mod,
                    shared,
                    silent,
                    unnamed,
                } => {
                    f.text(" under the tag ").spelled(tag);
                    let _ = write!(f, " ({events} events kept");
                    if *from_mod {
                        let _ = write!(f, " from the mod's own events bank, {shared} shared");
                    }
                    if *silent > 0 {
                        let _ = write!(f, ", {silent} sounds silent");
                    }
                    if !unnamed.is_empty() {
                        let _ = write!(f, ", {} under their own ids", unnamed.len());
                    }
                    f.text(")");
                }
            }
            if bank.layer != Layer::Base {
                let _ = write!(f, " in the layer {}", bank.layer.name());
            }
            if let Some(replaces) = &bank.replaces {
                f.text(" in place of the game's ").spelled(replaces);
            }
        });
        if !self.once.is_empty() {
            f.text("; ")
                .count(self.once.len(), "locale copy")
                .text(" of the champion WAD's voice carried once");
        }
    }
}

/// Why own clips are not carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "why", rename_all = "kebab-case")]
pub enum NotCarried {
    /// The game's skin at the target has no audio block with an events bank: there is no
    /// header of the client's version to write a bank with.
    NoGameAudio {
        /// The own clips of the unit.
        clips: usize,
        /// The target.
        target: Slot,
    },
    /// A locale WAD's copy holds other own clips than the champion WAD's copy.
    LocaleDiffers {
        /// The own clips of the copy.
        clips: usize,
        /// The mod's bank unit.
        source: String,
        /// The locale.
        locale: String,
    },
    /// The bank sits at a path the target's units do not load, and neither the game nor the
    /// mod has an events bank of the client's version to cut.
    NoEventsBank {
        /// The own clips of the bank.
        clips: usize,
        /// The mod's bank unit.
        source: String,
    },
    /// No event of the events bank plays the clips.
    NoEvent {
        /// The own clips of the bank.
        clips: usize,
        /// The mod's bank unit.
        source: String,
    },
    /// The game's entry has no bank unit to model the port's on: the layer's units are not
    /// written.
    NoUnitToModel {
        /// The layer.
        layer: Layer,
    },
    /// The unit's skin bin is written whole (#ADR-0017), and a bin of the package takes no
    /// patch: its voice layers are not written.
    NewBin,
}

/// The game's bank units at the target the port leaves out of the block, as the mod's own block
/// does (#ADR-0063).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioUnitsLeftOut {
    /// The unit.
    pub unit: UnitId,
    /// The game's units, each with the first build the game's skin held it.
    pub units: Vec<LeftOut>,
}

impl Diag for AudioUnitsLeftOut {
    const CODE: Code = plan("audio.units-left-out");

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the game's units the mod's audio block leaves out, which the game's skin held ")
            .text("when the mod was made, are left out: ");
        f.list(&self.units, |f, left| {
            f.spelled(&left.name);
        });
    }
}

/// Own clips of a unit the port does not carry, each with why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioNotCarried {
    /// The unit.
    pub unit: UnitId,
    /// What is not carried.
    pub left: Vec<NotCarried>,
}

impl Diag for AudioNotCarried {
    const CODE: Code = Code::new("audio.not-carried", Channel::Output, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("not carried: ");
        f.list(&self.left, |f, left| match left {
            NotCarried::NoGameAudio { clips, target } => {
                let _ = write!(
                    f,
                    "{clips} own clips: the game's skin{target} has no audio block with an \
                     events bank to take a header from"
                );
            }
            NotCarried::LocaleDiffers {
                clips,
                source,
                locale,
            } => {
                let _ = write!(f, "{clips} own clips of ");
                f.spelled(source);
                let _ = write!(
                    f,
                    " in the {locale} WAD, other than the champion WAD's copy"
                );
            }
            NotCarried::NoEventsBank { clips, source } => {
                let _ = write!(f, "{clips} own clips of ");
                f.spelled(source).text(
                    " at a premium skin's path: no events bank of the client's version to cut",
                );
            }
            NotCarried::NoEvent { clips, source } => {
                let _ = write!(f, "{clips} own clips of ");
                f.spelled(source)
                    .text(": no event of its events bank plays them");
            }
            NotCarried::NoUnitToModel { layer } => {
                let _ = write!(
                    f,
                    "the units of the layer {}: the game's entry has no bank unit to model \
                     them on",
                    layer.name()
                );
            }
            NotCarried::NewBin => {
                f.text("the unit's voice layers: its skin bin is written whole and takes no patch");
            }
        });
    }
}

/// Literal sound names of a unit's carried objects, rewritten for the target's audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LiteralsRenamed {
    /// The unit.
    pub unit: UnitId,
    /// How many now name an event the port renamed under the mod's tag.
    pub to_mod: usize,
    /// How many now name the base's event.
    pub to_base: usize,
}

impl Diag for LiteralsRenamed {
    const CODE: Code = plan("audio.literals-renamed");

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "{} sound names rewritten: {} to the mod's tag, {} to the base's",
            self.to_mod + self.to_base,
            self.to_mod,
            self.to_base
        );
    }
}
