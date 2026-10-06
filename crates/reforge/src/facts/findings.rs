//! What the facts find: one type per code of `docs/design/CODES.md` for copies, origins,
//! rigs and the banks that do not read.

use std::fmt::Write as _;

use serde::Serialize;

use super::copies::compact;
use super::rig::Visibility;
use crate::diag::{Channel, Code, Diag, Message, Record, Severity, Subject};
use crate::model::entity::{CharId, FileId, UnitId};
use crate::model::{Slot, Spelled};

/// Every diagnostic code the facts raise.
pub const CODES: &[Code] = &[
    CatalogCapped::CODE,
    CopiesFound::CODE,
    NearCopy::CODE,
    DistinctUnits::CODE,
    OriginAmbiguous::CODE,
    OriginConflicting::CODE,
    OriginNone::CODE,
    SubmeshesCopied::CODE,
    RigUnreadable::CODE,
    BankUnreadable::CODE,
];

/// The game's skins of a character whose bins link more than the cap read of one skin: the
/// bins past it are not known to the facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogCapped {
    /// The character.
    pub character: CharId,
    /// The skins.
    pub slots: Vec<Slot>,
    /// The cap on the bins read of one skin.
    pub cap: usize,
}

impl Diag for CatalogCapped {
    const CODE: Code = Code::new("catalog.capped", Channel::Input, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Character(self.character)
    }

    fn message(&self, f: &mut Message<'_>) {
        let slots: Vec<String> = self.slots.iter().map(Slot::to_string).collect();
        let _ = write!(
            f,
            "the game's skin {} link more bins than the {} read of one skin; the rest are not read",
            slots.join("/"),
            self.cap
        );
    }
}

/// The copies of a representative unit, at other slots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CopiesFound {
    /// The representative.
    pub unit: UnitId,
    /// Its copies.
    pub copies: Vec<UnitId>,
    /// The slots of the copies.
    pub slots: Vec<Slot>,
}

impl Diag for CopiesFound {
    const CODE: Code = Code::new("copies.found", Channel::Input, Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(1));

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let noun = if self.copies.len() == 1 {
            "copy"
        } else {
            "copies"
        };
        let n = self.copies.len();
        let _ = write!(f, "{n} {noun} at other slots: {}", compact(&self.slots));
    }
}

/// A near copy: another build of the representative's work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NearCopy {
    /// The copy.
    pub unit: UnitId,
    /// The representative.
    pub of: UnitId,
    /// The representative's slot.
    pub of_slot: Slot,
    /// The objects both bins hold, slot names normalised.
    pub shared: u32,
    /// The objects one of them holds.
    pub differing: u32,
}

impl Diag for NearCopy {
    const CODE: Code = Code::new("copies.near", Channel::Input, Severity::Note);
    const RECORD: Option<Record> = Some(Record::mod_artifact(1));

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "a near copy of skin{} ({} objects shared, {} differ)",
            self.of_slot, self.shared, self.differing
        );
    }
}

/// Several distinct skin bins in one character: each is a unit of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DistinctUnits {
    /// The character.
    pub character: CharId,
    /// How many representatives it has.
    pub units: usize,
}

impl Diag for DistinctUnits {
    const CODE: Code = Code::new("copies.distinct-units", Channel::Input, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Character(self.character)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "{} distinct skin bins: each is a unit of its own",
            self.units
        );
    }
}

/// Premium skins tie on the evidence of a unit's origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OriginAmbiguous {
    /// The unit.
    pub unit: UnitId,
    /// The skins that weigh the same, the one taken first.
    pub slots: Vec<Slot>,
}

impl Diag for OriginAmbiguous {
    const CODE: Code = Code::new("origin.ambiguous", Channel::Decision, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let slots: Vec<String> = self.slots.iter().map(Slot::to_string).collect();
        let _ = write!(f, "skins {} weigh the same", slots.join(", "));
    }
}

/// Strong evidence for both the base and a premium skin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OriginConflicting {
    /// The unit.
    pub unit: UnitId,
    /// The premium skin.
    pub slot: Slot,
}

impl Diag for OriginConflicting {
    const CODE: Code = Code::new("origin.conflicting", Channel::Decision, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "strong evidence for both the base and skin {}",
            self.slot
        );
    }
}

/// No evidence decides a unit's origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OriginNone {
    /// The unit.
    pub unit: UnitId,
}

impl Diag for OriginNone {
    const CODE: Code = Code::new("origin.none", Channel::Decision, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("no evidence decides between the base and a premium skin");
    }
}

/// A submesh of the mod's own mesh that is a premium skin's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CopiedSubmesh {
    /// Its name, as the mesh spells it.
    pub name: Spelled,
    /// The skin whose mesh holds its vertices.
    pub from: Slot,
    /// Whether the skin shows it.
    pub visibility: Visibility,
}

/// Submeshes copied whole from a premium skin into the mod's own mesh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubmeshesCopied {
    /// The unit.
    pub unit: UnitId,
    /// The submeshes, in the mesh's order.
    pub submeshes: Vec<CopiedSubmesh>,
}

impl Diag for SubmeshesCopied {
    const CODE: Code = Code::new("rig.submesh-copied", Channel::Decision, Severity::Warning);
    const RECORD: Option<Record> = Some(Record::mod_artifact(24));

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let n = self.submeshes.len();
        let _ = write!(
            f,
            "{n} submesh{} copied from a premium skin: ",
            if n == 1 { "" } else { "es" }
        );
        f.list(&self.submeshes, |f, submesh| {
            f.spelled(submesh.name.raw());
            let _ = write!(
                f,
                " (skin {}, {})",
                submesh.from,
                submesh.visibility.as_str()
            );
        });
    }
}

/// A file of a unit's rig.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RigPart {
    /// The mesh.
    Mesh,
    /// The skeleton.
    Skeleton,
}

impl RigPart {
    /// The part's name in a message.
    pub fn as_str(self) -> &'static str {
        match self {
            RigPart::Mesh => "mesh",
            RigPart::Skeleton => "skeleton",
        }
    }
}

/// Files of a unit's rig that do not parse. The checks they feed are not judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RigUnreadable {
    /// The unit.
    pub unit: UnitId,
    /// The parts that do not parse.
    pub parts: Vec<RigPart>,
}

impl Diag for RigUnreadable {
    const CODE: Code = Code::new("rig.unreadable", Channel::Decision, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let parts: Vec<&str> = self.parts.iter().map(|part| part.as_str()).collect();
        let _ = write!(f, "the {} does not parse", parts.join(" and the "));
    }
}

/// A bank that does not read, with why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnreadBank {
    /// The bank.
    pub file: FileId,
    /// What the reader refused.
    pub reason: String,
}

/// Banks a unit's audio block names that do not read. Their clips are not judged, so none of
/// them is carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BankUnreadable {
    /// The unit.
    pub unit: UnitId,
    /// The banks, in the order the block names them.
    pub banks: Vec<UnreadBank>,
}

impl Diag for BankUnreadable {
    const CODE: Code = Code::new("audio.bank-unreadable", Channel::Output, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.banks.len(), "bank")
            .text(" of its audio block that do not read, their clips not judged: ");
        f.list(&self.banks, |f, bank| {
            f.file(bank.file).text(" (").spelled(&bank.reason).text(")");
        });
    }
}
