//! What the fixups raise: one finding per fixup and outcome, counted from the changes. The
//! fixups share the four codes and are told apart by `by` (#ADR-0008), so a rule a person adds
//! needs no code of its own.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Serialize, Serializer};

use super::changes::{Change, Outcome};
use crate::diag::{Channel, Code, Diag, Message, Severity, Sink, Subject};
use crate::model::BinHash;
use crate::model::entity::ObjectId;

const fn code(name: &'static str, severity: Severity) -> Code {
    Code::new(name, Channel::Output, severity)
}

fn hex<S: Serializer>(hash: &BinHash, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(&format_args!("{:08x}", hash.0))
}

/// How many values of one field of one class a fixup changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldCount {
    /// The class of the struct the field is on.
    #[serde(serialize_with = "hex")]
    pub class: BinHash,
    /// The field the value was in, or for a value added the field it is in.
    #[serde(serialize_with = "hex")]
    pub field: BinHash,
    /// How many values.
    pub values: usize,
}

/// The changes of one fixup and one outcome, counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tally {
    /// How many values.
    pub values: usize,
    /// How many objects hold them.
    pub objects: usize,
    /// The values by class and field, the most frequent first.
    pub fields: Vec<FieldCount>,
}

impl Tally {
    /// The count of some changes. None when there is none.
    pub fn of<'a>(changes: impl IntoIterator<Item = &'a Change>) -> Option<Self> {
        let mut by_field: BTreeMap<(BinHash, BinHash), usize> = BTreeMap::new();
        let mut objects: BTreeSet<ObjectId> = BTreeSet::new();
        let mut values = 0;
        for change in changes {
            let place = change.path.last_field().unwrap_or((BinHash(0), BinHash(0)));
            *by_field.entry(place).or_default() += 1;
            objects.insert(change.object);
            values += 1;
        }
        let mut fields: Vec<FieldCount> = by_field
            .into_iter()
            .map(|((class, field), values)| FieldCount {
                class,
                field,
                values,
            })
            .collect();
        fields.sort_by_key(|row| std::cmp::Reverse(row.values));
        (values > 0).then_some(Self {
            values,
            objects: objects.len(),
            fields,
        })
    }

    /// Writes `12 values in 3 objects <what>: Class.field (8), Class.other (4)`.
    fn message(&self, f: &mut Message<'_>, what: &str) {
        f.count(self.values, "value")
            .text(" in ")
            .count(self.objects, "object")
            .text(" ")
            .text(what)
            .text(": ");
        f.list(&self.fields, |f, row| {
            f.class(row.class).text(".").field(row.field);
            let _ = write!(f, " ({})", row.values);
        });
    }
}

/// Values a fixup kept, in the form the target build reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Changed(pub Tally);

impl Diag for Changed {
    const CODE: Code = code("fixup.changed", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        self.0
            .message(f, "kept, in the form the target build reads");
    }
}

/// Values a fixup removed with nothing lost: no build since the mod's own read them, or a
/// tool wrote them and no build read them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Removed(pub Tally);

impl Diag for Removed {
    const CODE: Code = code("fixup.removed", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        self.0.message(f, "removed with nothing lost");
    }
}

/// Values a fixup removed that the mod's own build read and the target cannot hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lost(pub Tally);

impl Diag for Lost {
    const CODE: Code = code("fixup.lost", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        self.0.message(
            f,
            "removed that the mod's own build read and the target build cannot hold",
        );
    }
}

/// Values with no conversion to the kind the target build reads. They are left as they are,
/// and the client skips them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unconvertible(pub Tally);

impl Diag for Unconvertible {
    const CODE: Code = code("fixup.unconvertible", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        self.0.message(
            f,
            "left as they are, with no conversion to what the target build reads; the client \
             skips them",
        );
    }
}

/// Raises what one fixup did, one finding per outcome it has changes of.
pub(super) fn raise(changes: &[Change], sink: &mut Sink<'_>) {
    for outcome in Outcome::ALL {
        let of_outcome = changes.iter().filter(|change| change.outcome() == outcome);
        let Some(tally) = Tally::of(of_outcome) else {
            continue;
        };
        match outcome {
            Outcome::Changed => sink.raise(Changed(tally)),
            Outcome::Removed => sink.raise(Removed(tally)),
            Outcome::Lost => sink.raise(Lost(tally)),
            Outcome::Unconvertible => sink.raise(Unconvertible(tally)),
        };
    }
}
