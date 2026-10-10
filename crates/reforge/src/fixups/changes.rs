//! What the fixups did, as records: one per value, with the fixup, the object, the field path
//! and the value before and after. An object's edited state, a report's counts and the
//! `fixup.*` findings are all read off these.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ltk_meta::PropertyValueEnum;
use serde::Serialize;

use crate::model::entity::ObjectId;
use crate::model::value::{brief, kind_name};
use crate::model::{Dump, DumpCx, FieldPath, FixupId, Spell, clean};

/// A value as a change records it: its kind and one bounded line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    /// The value's kind, as the meta schema names kinds.
    pub kind: &'static str,
    /// The value on one line. A string in it is the mod's own text.
    pub text: String,
}

impl Snapshot {
    /// The snapshot of a value, with the names it is spelled with.
    pub fn of(value: &PropertyValueEnum, names: &dyn Spell) -> Self {
        Self {
            kind: kind_name(value.kind()),
            text: brief(value, names),
        }
    }
}

/// What a fixup left where a value was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "as", rename_all = "kebab-case")]
pub enum After {
    /// The value in the form the target build reads, at the same place. With no value before,
    /// a value the fixup added.
    Value(Snapshot),
    /// The same value, under another field or in another struct.
    Moved {
        /// Where the value is.
        to: FieldPath,
    },
    /// Nothing: the value is removed, and nothing is lost by it.
    Removed,
    /// Nothing: the value is removed, and the mod's own build read it.
    Lost,
    /// The value as it was: it has no conversion to the kind the target build reads.
    Unconvertible {
        /// The kind the target build reads there.
        wants: &'static str,
    },
}

/// Which `fixup.*` finding a change counts under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// A value kept, in the form the target build reads.
    Changed,
    /// A value removed with nothing lost.
    Removed,
    /// A value removed that the mod's own build read.
    Lost,
    /// A value with no conversion, left as it is.
    Unconvertible,
}

impl Outcome {
    /// Every outcome, in the order the findings are raised.
    pub const ALL: [Outcome; 4] = [
        Outcome::Changed,
        Outcome::Removed,
        Outcome::Lost,
        Outcome::Unconvertible,
    ];
}

/// What a fixup did at one place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    /// The fixup.
    pub fixup: FixupId,
    /// The object the value is in.
    pub object: ObjectId,
    /// Where the value was, or for a value added where it is.
    pub path: FieldPath,
    /// The value before. None for a value the fixup added.
    pub before: Option<Snapshot>,
    /// What is there after.
    pub after: After,
}

impl Change {
    /// The finding the change counts under.
    pub fn outcome(&self) -> Outcome {
        match self.after {
            After::Value(_) | After::Moved { .. } => Outcome::Changed,
            After::Removed => Outcome::Removed,
            After::Lost => Outcome::Lost,
            After::Unconvertible { .. } => Outcome::Unconvertible,
        }
    }

    /// Whether the object differs by it. A value with no conversion stays as it was.
    pub fn edits(&self) -> bool {
        self.outcome() != Outcome::Unconvertible
    }
}

/// How many changes of each outcome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    /// Values kept, in the form the target build reads.
    pub changed: usize,
    /// Values removed with nothing lost.
    pub removed: usize,
    /// Values removed that the mod's own build read.
    pub lost: usize,
    /// Values with no conversion, left as they are.
    pub unconvertible: usize,
}

impl Counts {
    /// The counts of some changes.
    pub fn of<'a>(changes: impl IntoIterator<Item = &'a Change>) -> Self {
        let mut counts = Self::default();
        for change in changes {
            match change.outcome() {
                Outcome::Changed => counts.changed += 1,
                Outcome::Removed => counts.removed += 1,
                Outcome::Lost => counts.lost += 1,
                Outcome::Unconvertible => counts.unconvertible += 1,
            }
        }
        counts
    }

    /// How many values differ: every change but the values left as they are.
    pub fn edits(&self) -> usize {
        self.changed + self.removed + self.lost
    }
}

/// Every change of one run of a fixup set, in the order made: fixup by fixup in the set's
/// order, object by object within a fixup, and in the walk's order within an object.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Changes {
    items: Vec<Change>,
}

impl Changes {
    pub(super) fn push(&mut self, change: Change) {
        self.items.push(change);
    }

    /// Every change, in the order made.
    pub fn iter(&self) -> impl Iterator<Item = &Change> {
        self.items.iter()
    }

    /// How many changes were recorded, the values left as they are included.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The changes grouped by fixup, in the set's order.
    pub fn by_fixup(&self) -> impl Iterator<Item = (&FixupId, &[Change])> {
        self.items
            .chunk_by(|a, b| a.fixup == b.fixup)
            .map(|group| (&group[0].fixup, group))
    }

    /// The changes one fixup made. Empty for a fixup that made none, or did not run.
    pub fn of(&self, fixup: &str) -> &[Change] {
        let mut groups = self.by_fixup();
        groups
            .find(|(id, _)| id.as_str() == fixup)
            .map_or(&[], |(_, group)| group)
    }

    /// How many changes of each outcome.
    pub fn counts(&self) -> Counts {
        Counts::of(&self.items)
    }

    /// The objects that differ from what the archive ships.
    pub fn edited(&self) -> BTreeSet<ObjectId> {
        let edits = self.items.iter().filter(|change| change.edits());
        edits.map(|change| change.object).collect()
    }

    /// Whether an object differs from what the archive ships.
    pub fn edits(&self, object: ObjectId) -> bool {
        let mut of_object = self.items.iter().filter(|change| change.object == object);
        of_object.any(Change::edits)
    }
}

/// The changes of a run read back, in the order they were made.
impl FromIterator<Change> for Changes {
    fn from_iter<I: IntoIterator<Item = Change>>(changes: I) -> Self {
        Self {
            items: changes.into_iter().collect(),
        }
    }
}

impl Dump for Changes {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = String::new();
        for (fixup, changes) in self.by_fixup() {
            let counts = Counts::of(changes);
            let _ = writeln!(
                out,
                "fixup {fixup} changed={} removed={} lost={} unconvertible={}",
                counts.changed, counts.removed, counts.lost, counts.unconvertible
            );
            for change in changes {
                let before = change.before.as_ref();
                let before = before.map_or("-", |snapshot| snapshot.text.as_str());
                let after = match &change.after {
                    After::Value(snapshot) => snapshot.text.clone(),
                    After::Moved { to } => format!("at {}", to.spelled(cx.names)),
                    After::Removed => "removed".to_string(),
                    After::Lost => "lost".to_string(),
                    After::Unconvertible { wants } => format!("left, the build reads {wants}"),
                };
                let _ = writeln!(
                    out,
                    "     {} {}: {} -> {}",
                    change.object,
                    clean(&change.path.spelled(cx.names)),
                    clean(before),
                    clean(&after)
                );
            }
        }
        out
    }
}
