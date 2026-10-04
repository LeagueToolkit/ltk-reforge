//! The stage `check`: a built package read as the client reads it. A check is an object that
//! describes itself ([`CheckInfo`]) and is chosen as a fixup is: a [`CheckSelection`] is the
//! choice as data, a [`CheckSet`] the checks of a run. What a check finds is a diagnostic and
//! does not stop the write (#ADR-0003).

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use indexmap::IndexMap;
use ltk_meta::{Bin, BinObject, PropertyValueEnum};
use serde::{Deserialize, Serialize};

use super::findings::{
    Against, Coverage, Done, FieldUndeclared, Place, RoundTripFailed, RoundTripProblem, Shape,
    TypeMismatch, TypeRow, UndeclaredRow,
};
use super::{Origin, Package, WrittenBin};
use crate::diag::{Record, Sink};
use crate::fixups::{TargetTypes, TypeSource};
use crate::game::FieldType;
use crate::model::{BinHash, Fields, bin, panic_text};

/// The name a caller turns a check on and off by: `round-trip`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CheckId(Cow<'static, str>);

impl CheckId {
    /// The id of a built-in check.
    pub const fn built_in(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CheckId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&self.0)
    }
}

impl From<&str> for CheckId {
    fn from(id: &str) -> Self {
        Self(Cow::Owned(id.to_string()))
    }
}

/// What a caller lists and toggles a check by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckInfo {
    /// The name it is turned on and off by.
    pub id: CheckId,
    /// One line for a person: what it checks.
    pub title: Cow<'static, str>,
    /// The record it rests on, when it has one.
    pub record: Option<Record>,
    /// Whether the standard set holds it.
    pub default: bool,
}

impl CheckInfo {
    /// The description of a built-in check of the standard set.
    pub const fn built_in(id: &'static str, title: &'static str, record: Option<Record>) -> Self {
        Self {
            id: CheckId::built_in(id),
            title: Cow::Borrowed(title),
            record,
            default: true,
        }
    }
}

/// What a check reads: the package, and the types the target build reads.
pub struct CheckCx<'a> {
    /// The package. A check changes nothing in it.
    pub package: &'a Package,
    /// The target build's type of each field of each class.
    pub types: &'a TargetTypes<'a>,
}

/// A test of a built package.
pub trait Check: Send + Sync {
    /// What a caller lists and toggles it by.
    fn info(&self) -> &CheckInfo;

    /// Reads the package and raises what it finds, and what it covered.
    fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>);
}

/// Every built-in check, in the order they run.
fn built_in() -> Vec<Arc<dyn Check>> {
    vec![Arc::new(Types), Arc::new(RoundTrip)]
}

/// A choice of checks, as data: saved and loaded beside a choice of fixups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSelection {
    /// Checks turned off: left out of the set whatever their default.
    #[serde(default)]
    pub off: BTreeSet<CheckId>,
    /// Checks turned on that the standard set leaves out.
    #[serde(default)]
    pub on: BTreeSet<CheckId>,
}

impl CheckSelection {
    /// The standard set: nothing turned off or on.
    pub fn standard() -> Self {
        Self::default()
    }

    /// Turns a check off.
    #[must_use]
    pub fn without(mut self, id: impl Into<CheckId>) -> Self {
        let id = id.into();
        self.on.remove(&id);
        self.off.insert(id);
        self
    }

    /// Turns a check on.
    #[must_use]
    pub fn with(mut self, id: impl Into<CheckId>) -> Self {
        let id = id.into();
        self.off.remove(&id);
        self.on.insert(id);
        self
    }

    /// The ids turned off or on that name no built-in check: a caller reports them, and the
    /// set leaves them out.
    pub fn unknown(&self) -> Vec<&CheckId> {
        let catalog = CheckSet::catalog();
        let known = |id: &&CheckId| catalog.iter().any(|info| info.id == **id);
        let named = self.off.iter().chain(&self.on);
        named.filter(|id| !known(id)).collect()
    }

    fn takes(&self, info: &CheckInfo) -> bool {
        !self.off.contains(&info.id) && (info.default || self.on.contains(&info.id))
    }
}

/// The checks of a run, in the order they run.
#[derive(Clone)]
pub struct CheckSet {
    checks: Vec<Arc<dyn Check>>,
}

impl Default for CheckSet {
    fn default() -> Self {
        Self::standard()
    }
}

impl CheckSet {
    /// A set with no check in it.
    pub fn empty() -> Self {
        Self { checks: Vec::new() }
    }

    /// Every built-in check whose default is on.
    pub fn standard() -> Self {
        Self::from(&CheckSelection::standard())
    }

    /// The description of every built-in check, in the order they run.
    pub fn catalog() -> Vec<CheckInfo> {
        let checks = built_in();
        checks.iter().map(|check| check.info().clone()).collect()
    }

    /// Adds a check after the ones the set holds. One with the id of a check the set holds
    /// takes its place.
    #[must_use]
    pub fn with(mut self, check: impl Check + 'static) -> Self {
        let id = &check.info().id;
        match self.checks.iter().position(|held| held.info().id == *id) {
            Some(place) => self.checks[place] = Arc::new(check),
            None => self.checks.push(Arc::new(check)),
        }
        self
    }

    /// The description of every check of the set, in the order they run.
    pub fn infos(&self) -> Vec<&CheckInfo> {
        self.checks.iter().map(|check| check.info()).collect()
    }

    /// The stage `check`: runs each check on the package, each through a sink that names the
    /// check and the record it rests on.
    pub fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>) {
        let _span = tracing::info_span!("check", checks = self.checks.len()).entered();
        for check in &self.checks {
            let info = check.info();
            check.run(cx, &mut sink.by(info.id.as_str(), info.record));
        }
    }
}

/// The set a selection gives: every built-in check it does not turn off, whose default is on
/// or which it turns on.
impl From<&CheckSelection> for CheckSet {
    fn from(selection: &CheckSelection) -> Self {
        let checks = built_in().into_iter();
        Self {
            checks: checks
                .filter(|check| selection.takes(check.info()))
                .collect(),
        }
    }
}

impl fmt::Debug for CheckSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ids = self.checks.iter().map(|check| check.info().id.as_str());
        f.debug_list().entries(ids).finish()
    }
}

/// Every bin written again reads back, holds the entries it was built with, and holds each
/// changed object as it was built.
#[derive(Debug, Clone, Copy, Default)]
pub struct RoundTrip;

static ROUND_TRIP: CheckInfo = CheckInfo::built_in(
    "round-trip",
    "A bin written again reads back as it was built",
    None,
);

impl RoundTrip {
    /// How the bytes of a bin differ from what it was built to hold.
    fn problems(bytes: &[u8], written: &WrittenBin) -> Vec<RoundTripProblem> {
        // The bytes are this run's own, and the reader is another crate's.
        let read = catch_unwind(AssertUnwindSafe(|| {
            Bin::from_reader(&mut Cursor::new(bytes)).map_err(|error| error.to_string())
        }));
        let back = match read.unwrap_or_else(|payload| Err(panic_text(&*payload))) {
            Ok(back) => back,
            Err(reason) => return vec![RoundTripProblem::Unreadable { reason }],
        };
        let mut problems = Vec::new();
        let read: BTreeSet<BinHash> = back.objects.keys().copied().collect();
        if read != written.entries {
            problems.push(RoundTripProblem::Entries {
                built: written.entries.len(),
                read: read.len(),
            });
        }
        for object in &written.edited {
            if back.objects.get(&object.path_hash) != Some(object) {
                problems.push(RoundTripProblem::Object {
                    entry: object.path_hash,
                });
            }
        }
        problems
    }
}

impl Check for RoundTrip {
    fn info(&self) -> &CheckInfo {
        &ROUND_TRIP
    }

    fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>) {
        let (mut bins, mut objects) = (0, 0);
        for (_, key, chunk) in cx.package.chunks() {
            let Origin::Rewritten { written, .. } = &chunk.origin else {
                continue;
            };
            bins += 1;
            objects += written.edited.len();
            let problems = Self::problems(&chunk.bytes, written);
            if !problems.is_empty() {
                sink.raise(RoundTripFailed {
                    wad: key.wad.clone(),
                    path: key.path.clone(),
                    problems,
                });
            }
        }
        sink.raise(Done(Coverage::RoundTrip { bins, objects }));
    }
}

/// Every value of every bin the package ships is of the type the target build reads there,
/// on a field its class declares. The types are the schema's at the install's build, and for
/// a field the schema does not describe what the install's bins hold. Without a schema
/// nothing is said of undeclared fields.
#[derive(Debug, Clone, Copy, Default)]
pub struct Types;

static TYPES: CheckInfo = CheckInfo::built_in(
    "types",
    "Every value written is typed as the target build reads it",
    Some(Record::game_behavior(2)),
);

impl Check for Types {
    fn info(&self) -> &CheckInfo {
        &TYPES
    }

    fn run(&self, cx: &CheckCx<'_>, sink: &mut Sink<'_>) {
        let mut typing = Typing {
            types: cx.types,
            fields: 0,
            wrong: IndexMap::new(),
            undeclared: IndexMap::new(),
        };
        let mut bins = 0;
        for (_, key, chunk) in cx.package.bins() {
            // A bin that does not read is `load`'s finding, and the client reads none of it.
            let Ok(parsed) = bin::parse(&chunk.bytes) else {
                continue;
            };
            bins += 1;
            for object in &parsed.objects {
                let place = Place {
                    wad: key.wad.clone(),
                    path: key.path.clone(),
                    entry: object.value.path_hash,
                };
                typing.object(&object.value, &place);
            }
        }
        if !typing.wrong.is_empty() {
            let fields: Vec<TypeRow> = typing.wrong.into_values().collect();
            sink.raise(TypeMismatch {
                values: fields.iter().map(|row| row.values).sum(),
                fields,
            });
        }
        if !typing.undeclared.is_empty() {
            let fields: Vec<UndeclaredRow> = typing.undeclared.into_values().collect();
            sink.raise(FieldUndeclared {
                values: fields.iter().map(|row| row.values).sum(),
                fields,
            });
        }
        let against = match cx.types.schema().and(cx.types.build()) {
            Some(build) => Against::Schema { build },
            None => Against::Install,
        };
        sink.raise(Done(Coverage::Types {
            bins,
            fields: typing.fields,
            against,
        }));
    }
}

/// The walk of one package's values against the target's types.
struct Typing<'a> {
    types: &'a TargetTypes<'a>,
    /// How many fields were typed.
    fields: usize,
    /// The values of another type, by class, field and the two shapes, in the order found.
    wrong: IndexMap<(BinHash, BinHash, Shape, Shape), TypeRow>,
    /// The values on a field its class does not declare, by class and field.
    undeclared: IndexMap<(BinHash, BinHash), UndeclaredRow>,
}

impl Typing<'_> {
    /// Whether a value of the shape `found` reads as `want`.
    fn reads_as(&self, want: &FieldType, found: &FieldType, source: TypeSource) -> bool {
        let differs = |want: Option<_>, found: Option<_>| want.is_some() && want != found;
        if want.kind != found.kind || differs(want.key, found.key) || differs(want.item, found.item)
        {
            return false;
        }
        // A pointer or an embed of a derived class reads as its base. The install's bins
        // show one class a field held and not every class it may hold.
        match (want.class, found.class, source) {
            (Some(want), Some(found), TypeSource::Schema) if want != found => {
                let (Some(schema), Some(build)) = (self.types.schema(), self.types.build()) else {
                    return true;
                };
                schema.derives_from(found, want, build) || !schema.knows_class(found)
            }
            _ => true,
        }
    }

    fn object(&mut self, object: &BinObject, place: &Place) {
        self.structure(object.class_hash, &object.properties, place);
    }

    fn structure(&mut self, class: BinHash, fields: &Fields, place: &Place) {
        for (field, value) in fields {
            self.field((class, *field), value, place);
        }
    }

    fn field(
        &mut self,
        (class, field): (BinHash, BinHash),
        value: &PropertyValueEnum,
        place: &Place,
    ) {
        self.fields += 1;
        match self.types.type_of(class, field) {
            Some((want, source)) => {
                let found = FieldType::of(value);
                if !self.reads_as(&want, &found, source) {
                    let shapes = (Shape::of(&found), Shape::of(&want));
                    let row = self
                        .wrong
                        .entry((class, field, shapes.0, shapes.1))
                        .or_insert_with(|| TypeRow {
                            class,
                            field,
                            found: shapes.0,
                            wants: shapes.1,
                            values: 0,
                            first: place.clone(),
                        });
                    row.values += 1;
                    return;
                }
            }
            None => {
                let schema = self.types.schema().filter(|_| self.types.build().is_some());
                if schema.is_some_and(|schema| schema.knows_class(class)) {
                    let row =
                        self.undeclared
                            .entry((class, field))
                            .or_insert_with(|| UndeclaredRow {
                                class,
                                field,
                                values: 0,
                                first: place.clone(),
                            });
                    row.values += 1;
                    return;
                }
            }
        }
        self.value(value, place);
    }

    /// Goes on into the structs a value holds.
    fn value(&mut self, value: &PropertyValueEnum, place: &Place) {
        use PropertyValueEnum as V;
        match value {
            V::Embedded(embedded) => {
                self.structure(embedded.0.class_hash, &embedded.0.properties, place);
            }
            V::Struct(pointer) if pointer.class_hash.0 != 0 => {
                self.structure(pointer.class_hash, &pointer.properties, place);
            }
            V::Container(list) => list.items().iter().for_each(|v| self.value(v, place)),
            V::UnorderedContainer(list) => {
                list.0.items().iter().for_each(|v| self.value(v, place));
            }
            V::Optional(option) => {
                if let Some(inner) = option.value() {
                    self.value(inner, place);
                }
            }
            V::Map(map) => map.entries().iter().for_each(|(_, v)| self.value(v, place)),
            _ => {}
        }
    }
}
