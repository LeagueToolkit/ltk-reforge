//! Table rules: a fixup as data. A rule removes fields, renames fields or moves fields into a
//! child struct, in the structs of one class. It is saved and loaded as JSON, and the engine
//! runs it as any other fixup.
//!
//! No rule changes a struct twice. Each shape acts only on a field the target build does not
//! read where it is, and leaves it where the target reads it or removes it:
//!
//! - a removal takes a field the target's class does not declare;
//! - a rename takes a field from a name the target's class does not declare to one it does;
//! - a move takes fields from a struct into a child struct of another class, which the walk
//!   then visits as that class.
//!
//! `docs/design/ARCHITECTURE.md`, "Fixups".

use std::fmt;

use ltk_meta::property::values;
use ltk_meta::{PropertyKind as Kind, PropertyValueEnum};
use serde::{Deserialize, Serialize};

use super::context::FixCx;
use super::types::TargetTypes;
use super::{BinFixup, Classes};
use crate::game::NameBook;
use crate::model::{
    BinHash, Fields, FixupId, FixupInfo, Phase, Record, StructMut, Version, name_hash,
};

/// A class or a field as a rule names it: by its name, or by its hash as `0x1e6ba0c4` where
/// no name is known.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub struct Name {
    text: String,
    hash: BinHash,
}

impl Name {
    /// The name from its text.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let hash = Self::hash_written(&text).unwrap_or_else(|| name_hash(&text));
        Self { text, hash }
    }

    /// The hash a text states, when it is written `0x` and one to eight hex digits.
    fn hash_written(text: &str) -> Option<BinHash> {
        let digits = text.strip_prefix("0x")?;
        let plain = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_hexdigit());
        (plain && digits.len() <= 8)
            .then(|| u32::from_str_radix(digits, 16).ok())
            .flatten()
            .map(BinHash)
    }

    /// The text as written.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The hash the bins hold.
    pub fn hash(&self) -> BinHash {
        self.hash
    }

    /// The name, when the rule states one and not a hash.
    pub fn spelled(&self) -> Option<&str> {
        Self::hash_written(&self.text)
            .is_none()
            .then_some(self.text.as_str())
    }
}

impl From<String> for Name {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<&str> for Name {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<Name> for String {
    fn from(name: Name) -> Self {
        name.text
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// One field of a rule's table: its name before, and its name after.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "RowRepr", into = "RowRepr")]
pub struct Row {
    /// The field's name in the old layout.
    pub from: Name,
    /// Its name in the layout the target build reads.
    pub to: Name,
}

/// A row as JSON holds it: one name for a field that keeps its name, else both.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum RowRepr {
    Kept(Name),
    Renamed { from: Name, to: Name },
}

impl From<RowRepr> for Row {
    fn from(row: RowRepr) -> Self {
        match row {
            RowRepr::Kept(name) => Row {
                from: name.clone(),
                to: name,
            },
            RowRepr::Renamed { from, to } => Row { from, to },
        }
    }
}

impl From<Row> for RowRepr {
    fn from(row: Row) -> Self {
        if row.from == row.to {
            RowRepr::Kept(row.to)
        } else {
            RowRepr::Renamed {
                from: row.from,
                to: row.to,
            }
        }
    }
}

/// The child struct a rule moves fields into.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Child {
    /// The field of the rule's class that holds the child.
    pub field: Name,
    /// The child's class.
    pub class: Name,
}

/// What a rule does in the structs of its class.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    /// Removes fields the target's class does not declare. Nothing is lost by it: the rule's
    /// record says why no build read them.
    Remove {
        /// The fields.
        fields: Vec<Name>,
    },
    /// Renames fields: each from a name the target's class does not declare to one it does.
    Rename {
        /// The fields.
        fields: Vec<Row>,
    },
    /// Moves fields into a child struct, each under the name the child's class declares.
    Move {
        /// The child.
        into: Child,
        /// The fields.
        fields: Vec<Row>,
    },
}

/// A fixup that is data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RuleRepr", into = "RuleRepr")]
pub struct Rule {
    info: FixupInfo,
    class: Name,
    op: Op,
}

/// A rule as JSON holds it.
#[derive(Serialize, Deserialize)]
struct RuleRepr {
    id: FixupId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(default)]
    records: Vec<Record>,
    #[serde(default = "undo")]
    phase: Phase,
    #[serde(default = "standard")]
    default: bool,
    class: Name,
    #[serde(flatten)]
    op: Op,
}

fn undo() -> Phase {
    Phase::Undo
}

fn standard() -> bool {
    true
}

impl From<RuleRepr> for Rule {
    fn from(repr: RuleRepr) -> Self {
        let mut rule = Rule::new(repr.class, repr.op);
        rule.info.id = repr.id;
        if let Some(title) = repr.title {
            rule.info.title = title.into();
        }
        rule.info.records = repr.records.into();
        rule.info.phase = repr.phase;
        rule.info.default = repr.default;
        rule
    }
}

impl From<Rule> for RuleRepr {
    fn from(rule: Rule) -> Self {
        Self {
            id: rule.info.id,
            title: Some(rule.info.title.into_owned()),
            records: rule.info.records.into_owned(),
            phase: rule.info.phase,
            default: rule.info.default,
            class: rule.class,
            op: rule.op,
        }
    }
}

/// A rule that moves fields, before it is told where.
#[must_use]
pub struct MoveFields {
    class: Name,
}

impl MoveFields {
    /// The child struct the fields move into: the field of the rule's class that holds it,
    /// and its class.
    pub fn into_field(self, field: impl Into<Name>, class: impl Into<Name>) -> Rule {
        let into = Child {
            field: field.into(),
            class: class.into(),
        };
        let fields = Vec::new();
        Rule::new(self.class, Op::Move { into, fields })
    }
}

impl Rule {
    /// A rule of an operation in a class, with the id and title its class and operation give,
    /// in the phase that takes a tool's damage out.
    fn new(class: Name, op: Op) -> Self {
        let (verb, what) = match &op {
            Op::Remove { .. } => ("remove", "fields removed".to_string()),
            Op::Rename { .. } => ("rename", "fields renamed".to_string()),
            Op::Move { into, .. } => ("move", format!("fields moved into {}", into.field)),
        };
        let id = match &op {
            Op::Move { into, .. } => format!("{verb}-{class}-{}", into.field),
            _ => format!("{verb}-{class}"),
        };
        let info = FixupInfo {
            id: FixupId::new(id.to_ascii_lowercase()),
            title: format!("{class}: {what}").into(),
            records: Vec::new().into(),
            phase: Phase::Undo,
            default: true,
        };
        Self { info, class, op }
    }

    /// A rule that moves fields of a class into a child struct.
    pub fn move_fields(class: impl Into<Name>) -> MoveFields {
        MoveFields {
            class: class.into(),
        }
    }

    /// A rule that renames fields of a class. `rename` adds each.
    #[must_use]
    pub fn rename_fields(class: impl Into<Name>) -> Self {
        Self::new(class.into(), Op::Rename { fields: Vec::new() })
    }

    /// A rule that removes fields of a class.
    #[must_use]
    pub fn remove_fields<N: Into<Name>>(
        class: impl Into<Name>,
        fields: impl IntoIterator<Item = N>,
    ) -> Self {
        let fields = fields.into_iter().map(Into::into).collect();
        Self::new(class.into(), Op::Remove { fields })
    }

    fn rows_mut(&mut self) -> Option<&mut Vec<Row>> {
        match &mut self.op {
            Op::Rename { fields } | Op::Move { fields, .. } => Some(fields),
            Op::Remove { .. } => None,
        }
    }

    /// Adds fields that move under the name they have. A rule that renames or removes takes
    /// none: a field that keeps its name and its place is not changed.
    #[must_use]
    pub fn keep<N: Into<Name>>(mut self, fields: impl IntoIterator<Item = N>) -> Self {
        if let Op::Move { fields: rows, .. } = &mut self.op {
            rows.extend(fields.into_iter().map(|name| {
                let name = name.into();
                Row {
                    from: name.clone(),
                    to: name,
                }
            }));
        }
        self
    }

    /// Adds a field that takes a new name. A rule that removes takes none.
    #[must_use]
    pub fn rename(mut self, from: impl Into<Name>, to: impl Into<Name>) -> Self {
        if let Some(rows) = self.rows_mut() {
            rows.push(Row {
                from: from.into(),
                to: to.into(),
            });
        }
        self
    }

    /// Names the rule: what a caller turns it on and off by.
    #[must_use]
    pub fn id(mut self, id: impl Into<FixupId>) -> Self {
        self.info.id = id.into();
        self
    }

    /// Says what the rule does, in one line for a person.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.info.title = title.into().into();
        self
    }

    /// Follows a change of Riot's: the rule runs in the layout phase, at the first build that
    /// has the change.
    #[must_use]
    pub fn at_build(mut self, build: Version) -> Self {
        self.info.phase = Phase::Layout { build };
        self
    }

    /// Runs the rule in a phase.
    #[must_use]
    pub fn phase(mut self, phase: Phase) -> Self {
        self.info.phase = phase;
        self
    }

    /// Adds a record the rule implements.
    #[must_use]
    pub fn record(mut self, record: Record) -> Self {
        self.info.records.to_mut().push(record);
        self
    }

    /// Leaves the rule out of a set unless a selection turns it on.
    #[must_use]
    pub fn off_by_default(mut self) -> Self {
        self.info.default = false;
        self
    }

    /// The class whose structs the rule changes.
    pub fn class(&self) -> &Name {
        &self.class
    }

    /// What the rule does.
    pub fn op(&self) -> &Op {
        &self.op
    }

    /// The fields the rule takes a value from, by the name each has before the rule runs.
    pub fn sources(&self) -> Vec<&Name> {
        match &self.op {
            Op::Remove { fields } => fields.iter().collect(),
            Op::Rename { fields } | Op::Move { fields, .. } => {
                fields.iter().map(|row| &row.from).collect()
            }
        }
    }

    /// Every class and field name the rule states.
    fn names(&self) -> (Vec<&Name>, Vec<&Name>) {
        let mut classes = vec![&self.class];
        let mut fields = Vec::new();
        match &self.op {
            Op::Remove { fields: removed } => fields.extend(removed),
            Op::Rename { fields: rows } => {
                fields.extend(rows.iter().flat_map(|row| [&row.from, &row.to]));
            }
            Op::Move { into, fields: rows } => {
                classes.push(&into.class);
                fields.push(&into.field);
                fields.extend(rows.iter().flat_map(|row| [&row.from, &row.to]));
            }
        }
        (classes, fields)
    }

    /// Has a run's names learn the names the rule states, so that a path of a change spells
    /// a field no table names.
    pub fn teach(&self, book: &mut NameBook) {
        let (classes, fields) = self.names();
        for name in classes.iter().filter_map(|name| name.spelled()) {
            book.learn_class(name);
        }
        for name in fields.iter().filter_map(|name| name.spelled()) {
            book.learn_field(name);
        }
    }

    /// The kind the target build holds the child of a move in: a pointer or an embed.
    fn child_kind(&self, types: &TargetTypes<'_>, into: &Child) -> Option<Kind> {
        // A struct moved into a struct of its own class would be moved again when the walk
        // enters the child.
        if into.class.hash == self.class.hash {
            return None;
        }
        types
            .kind_of(self.class.hash, into.field.hash)
            .filter(|kind| matches!(kind, Kind::Struct | Kind::Embedded))
    }

    /// Whether a rename's row is one the target build asks for: it reads the new name on the
    /// class and not the old one.
    fn renames(&self, types: &TargetTypes<'_>, row: &Row) -> bool {
        let class = self.class.hash;
        types.declares(class, row.to.hash) && !types.declares(class, row.from.hash)
    }

    /// Removes the fields the target's class does not declare. Whether any was there.
    fn remove(&self, cx: &mut FixCx<'_>, s: &mut StructMut<'_>, fields: &[Name]) -> bool {
        let mut changed = false;
        for field in fields {
            if cx.declares(self.class.hash, field.hash) {
                continue;
            }
            if let Some(value) = s.fields_mut().shift_remove(&field.hash) {
                cx.removed(s.path_of(field.hash), &value);
                changed = true;
            }
        }
        changed
    }

    /// Renames the fields the target build asks for and the mod's own build read. Whether
    /// any was there.
    fn rename_in(&self, cx: &mut FixCx<'_>, s: &mut StructMut<'_>, rows: &[Row]) -> bool {
        let class = self.class.hash;
        let mut changed = false;
        for row in rows {
            let read = |value: &PropertyValueEnum| {
                self.renames(cx.types(), row)
                    && cx.read_at_source(class, row.from.hash, value.kind())
            };
            if !s.fields().get(&row.from.hash).is_some_and(read) {
                continue;
            }
            let Some(value) = s.fields_mut().shift_remove(&row.from.hash) else {
                continue;
            };
            let from = s.path_of(row.from.hash);
            // The value the struct holds under the new name is the one the target reads.
            if s.fields().contains_key(&row.to.hash) {
                cx.lost(from, &value);
            } else {
                cx.moved(from, s.path_of(row.to.hash), &value);
                s.fields_mut().insert(row.to.hash, value);
            }
            changed = true;
        }
        changed
    }

    /// Moves into the child the fields its class declares and the mod's own build read.
    /// Whether any was there.
    fn move_in(
        &self,
        cx: &mut FixCx<'_>,
        s: &mut StructMut<'_>,
        into: &Child,
        rows: &[Row],
    ) -> bool {
        use PropertyValueEnum as V;
        let class = self.class.hash;
        let Some(kind) = self.child_kind(cx.types(), into) else {
            return false;
        };
        let moves = |row: &&Row| {
            let Some(value) = s.fields().get(&row.from.hash) else {
                return false;
            };
            // The field that holds the child moves only while it holds its old, plain value.
            let gathered =
                row.from.hash == into.field.hash && matches!(value, V::Struct(_) | V::Embedded(_));
            !gathered
                && cx.declares(into.class.hash, row.to.hash)
                && cx.read_at_source(class, row.from.hash, value.kind())
        };
        let here: Vec<&Row> = rows.iter().filter(moves).collect();
        if here.is_empty() {
            return false;
        }
        let child_path = s.path_of(into.field.hash);
        let mut inner = match s.fields().get(&into.field.hash) {
            Some(V::Struct(child) | V::Embedded(values::Embedded(child)))
                if child.class_hash == into.class.hash =>
            {
                child.properties.clone()
            }
            _ => Fields::new(),
        };
        for row in here {
            let Some(value) = s.fields_mut().shift_remove(&row.from.hash) else {
                continue;
            };
            let from = s.path_of(row.from.hash);
            // The value the child holds under the new name is the one the target reads.
            if inner.contains_key(&row.to.hash) {
                cx.lost(from, &value);
            } else {
                cx.moved(from, child_path.field(into.class.hash, row.to.hash), &value);
                inner.insert(row.to.hash, value);
            }
        }
        let child = values::Struct {
            class_hash: into.class.hash,
            properties: inner,
        };
        let child = match kind {
            Kind::Struct => V::Struct(child),
            _ => V::Embedded(values::Embedded(child)),
        };
        let replaced = s.fields_mut().insert(into.field.hash, child);
        // The child's fields were carried over, and a pointer to nothing held none. Any other
        // value in the child's field gives way to the child.
        let carried = |old: &V| match old {
            V::Struct(old) | V::Embedded(values::Embedded(old)) => {
                old.class_hash == into.class.hash || old.class_hash.0 == 0
            }
            _ => false,
        };
        if let Some(old) = replaced.filter(|old| !carried(old)) {
            cx.lost(child_path, &old);
        }
        true
    }
}

impl BinFixup for Rule {
    fn info(&self) -> &FixupInfo {
        &self.info
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        let class = self.class.hash;
        match &self.op {
            Op::Remove { fields } => fields.iter().any(|f| !types.declares(class, f.hash)),
            Op::Rename { fields } => fields.iter().any(|row| self.renames(types, row)),
            Op::Move { into, fields } => {
                self.child_kind(types, into).is_some()
                    && fields
                        .iter()
                        .any(|row| types.declares(into.class.hash, row.to.hash))
            }
        }
    }

    fn classes(&self) -> Classes {
        Classes::Only(vec![self.class.hash])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let changed = match &self.op {
            Op::Remove { fields } => self.remove(cx, &mut s, fields),
            Op::Rename { fields } => self.rename_in(cx, &mut s, fields),
            Op::Move { into, fields } => self.move_in(cx, &mut s, into, fields),
        };
        if changed {
            self.teach(cx.book());
        }
    }

    fn rule(&self) -> Option<&Rule> {
        Some(self)
    }
}
