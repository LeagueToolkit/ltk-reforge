//! The one walk over bin values: each struct of an object with the object's entry hash, the
//! struct's path from the object's root and its class, read-only or mutable. The traversal is
//! `ltk_meta`'s: the object first, then every pointer and embed under it in file order, each
//! before the structs it holds. A pointer to nothing (class 0) is no struct.
//!
//! `docs/design/ARCHITECTURE.md`, "The model".

use std::borrow::Cow;
use std::fmt;

use indexmap::IndexMap;
use ltk_meta::path::{FieldNames, MapKey, ValuePath, ValueSegment};
use ltk_meta::walk::{Node, NodeRefMut, Trail, TreeNode, Visit, Visitor, VisitorMut};
use ltk_meta::{BinObject, PropertyValueEnum};

use super::hash::BinHash;
use super::names::Spell;

/// The fields of a struct, in file order.
pub type Fields = IndexMap<BinHash, PropertyValueEnum>;

/// Where a value is inside one object: fields by hash, list items by index, map entries by
/// key. Each field carries the class of the struct it is on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FieldPath(ValuePath);

impl FieldPath {
    /// The object itself.
    pub fn root() -> Self {
        Self::default()
    }

    /// The path of a field of the struct at this path, which is of `class`.
    #[must_use]
    pub fn field(&self, class: BinHash, field: BinHash) -> Self {
        let mut path = self.0.clone();
        path.push_field(field, class);
        Self(path)
    }

    /// The path of an item of the list at this path, or of a present option's value at 0.
    #[must_use]
    pub fn item(&self, index: usize) -> Self {
        let mut path = self.0.clone();
        path.push_index(index);
        Self(path)
    }

    /// The path of the value under a key of the map at this path.
    #[must_use]
    pub fn key(&self, key: MapKey) -> Self {
        let mut path = self.0.clone();
        path.push_key(key);
        Self(path)
    }

    /// Whether the path is the object itself.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// The last field of the path with the class of the struct it is on: what a count of
    /// changes groups by.
    pub fn last_field(&self) -> Option<(BinHash, BinHash)> {
        let fields = self.0.fields();
        fields
            .last()
            .map(|(field, class)| (class.unwrap_or(BinHash(0)), field))
    }

    /// The path as `ltk_meta` holds it.
    pub fn as_value_path(&self) -> &ValuePath {
        &self.0
    }

    /// The path with every field a table names spelled, and the rest as hex. A map key is the
    /// mod's own text: a caller cleans the result before it prints it.
    pub fn spelled(&self, names: &dyn Spell) -> String {
        self.0.to_named(&ByField(names)).text
    }

    /// How many steps the path holds.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the path holds no step.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<ValuePath> for FieldPath {
    fn from(path: ValuePath) -> Self {
        Self(path)
    }
}

impl FromIterator<ValueSegment> for FieldPath {
    fn from_iter<I: IntoIterator<Item = ValueSegment>>(steps: I) -> Self {
        Self(steps.into_iter().collect())
    }
}

/// The hash form: `.` between fields, `[i]` for an index, `{key}` for a map entry, a field as
/// eight hex digits.
impl fmt::Display for FieldPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl serde::Serialize for FieldPath {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// A run's names as `ltk_meta` asks for them: a field by its hash, whatever class it is on.
struct ByField<'a>(&'a dyn Spell);

impl FieldNames for ByField<'_> {
    fn field(&self, field: BinHash, _class: Option<BinHash>) -> Option<Cow<'_, str>> {
        self.0.field(field).map(Cow::Owned)
    }
}

/// The path a trail of the owned tree stands for.
fn path_of(trail: &Trail<&PropertyValueEnum>) -> FieldPath {
    let path = trail.to_value_path().unwrap_or_else(|error| {
        panic!("the trail {trail} of an owned object does not copy: {error}")
    });
    FieldPath(path)
}

/// A struct of an object, as the walk gives it to a reader.
pub struct StructRef<'t, 'a> {
    node: &'t Node<'t, 'a, &'a PropertyValueEnum>,
}

impl<'a> StructRef<'_, 'a> {
    /// The entry hash of the object the struct is in, or is.
    pub fn entry(&self) -> BinHash {
        self.node.object_hash()
    }

    /// The struct's class.
    pub fn class(&self) -> BinHash {
        self.node.class_hash()
    }

    /// Whether the struct is the object itself.
    pub fn is_root(&self) -> bool {
        self.node.is_root()
    }

    /// Where the struct is. Built when asked: a visitor asks for the structs it reports on.
    pub fn path(&self) -> FieldPath {
        path_of(self.node.trail())
    }

    /// The struct's fields, in file order.
    pub fn fields(&self) -> impl Iterator<Item = (BinHash, &'a PropertyValueEnum)> {
        // The owned tree hands out its fields without decoding: none fails.
        self.node.inner().properties().filter_map(Result::ok)
    }

    /// The value of a field.
    pub fn get(&self, field: BinHash) -> Option<&'a PropertyValueEnum> {
        self.node.inner().get(field).ok().flatten()
    }
}

/// A struct of an object, as the walk gives it to a fixup: its fields may be edited, added and
/// removed. The walk goes on into the fields as the visitor leaves them.
pub struct StructMut<'a> {
    entry: BinHash,
    class: BinHash,
    fields: &'a mut Fields,
    trail: &'a Trail<&'a PropertyValueEnum>,
}

impl StructMut<'_> {
    /// The entry hash of the object the struct is in, or is.
    pub fn entry(&self) -> BinHash {
        self.entry
    }

    /// The struct's class.
    pub fn class(&self) -> BinHash {
        self.class
    }

    /// Whether the struct is the object itself.
    pub fn is_root(&self) -> bool {
        self.trail.is_empty()
    }

    /// Where the struct is. Built when asked: a fixup asks for the structs it changes.
    pub fn path(&self) -> FieldPath {
        path_of(self.trail)
    }

    /// The path of a field of the struct.
    pub fn path_of(&self, field: BinHash) -> FieldPath {
        self.path().field(self.class, field)
    }

    /// The struct's fields, in file order.
    pub fn fields(&self) -> &Fields {
        self.fields
    }

    /// The struct's fields, to edit.
    pub fn fields_mut(&mut self) -> &mut Fields {
        self.fields
    }
}

struct Reader<F>(F);

impl<'a, F: FnMut(&StructRef<'_, 'a>)> Visitor<'a, &'a PropertyValueEnum> for Reader<F> {
    type Error = ltk_meta::Error;

    fn enter_node(
        &mut self,
        node: &Node<'_, 'a, &'a PropertyValueEnum>,
    ) -> Result<Visit, Self::Error> {
        (self.0)(&StructRef { node });
        Ok(Visit::Continue)
    }
}

struct Editor<F>(F);

impl<F: FnMut(StructMut<'_>)> VisitorMut for Editor<F> {
    type Error = ltk_meta::Error;

    fn enter_node(&mut self, node: &mut NodeRefMut<'_>) -> Result<Visit, Self::Error> {
        let (entry, class, trail) = (node.object_hash(), node.class_hash(), node.trail());
        (self.0)(StructMut {
            entry,
            class,
            fields: node.properties_mut(),
            trail,
        });
        Ok(Visit::Continue)
    }
}

/// Visits every struct of an object: the object, then each pointer and embed under it, a
/// struct before the structs it holds.
///
/// # Panics
///
/// When `ltk_meta` fails to walk an object in memory, which no value of one makes it do.
pub fn structs<'a>(object: &'a BinObject, visit: impl FnMut(&StructRef<'_, 'a>)) {
    object.walk(&mut Reader(visit)).unwrap_or_else(|error| {
        panic!(
            "walking object {:08x} in memory failed: {error}",
            object.path_hash.0
        )
    });
}

/// Visits every struct of an object to edit it, in the order of [`structs`]. A struct a
/// visitor adds to a field is visited in its turn.
///
/// # Panics
///
/// When `ltk_meta` fails to walk an object in memory, which no value of one makes it do.
pub fn structs_mut(object: &mut BinObject, visit: impl FnMut(StructMut<'_>)) {
    let entry = object.path_hash;
    object
        .walk_mut(&mut Editor(visit))
        .unwrap_or_else(|error| panic!("walking object {:08x} in memory failed: {error}", entry.0));
}

#[cfg(test)]
mod tests {
    use ltk_meta::PropertyKind;
    use ltk_meta::property::values;

    use super::super::hash::name_hash;
    use super::super::names::NoNames;
    use super::*;

    fn pointer(class: &str, fields: Vec<(&str, PropertyValueEnum)>) -> PropertyValueEnum {
        PropertyValueEnum::Struct(values::Struct {
            class_hash: name_hash(class),
            properties: fields.into_iter().map(|(k, v)| (name_hash(k), v)).collect(),
        })
    }

    fn system() -> BinObject {
        let emitter = |name: &str| {
            pointer(
                "Emitter",
                vec![("emitterName", values::String::from(name).into())],
            )
        };
        let emitters =
            values::Container::new(PropertyKind::Struct, vec![emitter("a"), emitter("b")])
                .expect("a list of pointers");
        BinObject::builder(name_hash("System"), name_hash("VfxSystem"))
            .property(name_hash("emitters"), emitters)
            .property(
                name_hash("nothing"),
                values::Struct {
                    class_hash: BinHash(0),
                    properties: Fields::new(),
                },
            )
            .build()
    }

    #[test]
    fn a_struct_comes_with_its_object_its_path_and_its_class() {
        let object = system();
        let mut seen = Vec::new();
        structs(&object, |s| {
            assert_eq!(s.entry(), name_hash("System"));
            seen.push((s.path().to_string(), s.class(), s.is_root()));
        });
        let emitters = format!("{:08x}", name_hash("emitters").0);
        assert_eq!(
            seen,
            [
                (String::new(), name_hash("VfxSystem"), true),
                (format!("{emitters}[0]"), name_hash("Emitter"), false),
                (format!("{emitters}[1]"), name_hash("Emitter"), false),
            ]
        );
    }

    #[test]
    fn an_edit_stays_and_a_struct_added_is_visited() {
        let mut object = system();
        let mut classes = Vec::new();
        structs_mut(&mut object, |mut s| {
            classes.push(s.class());
            if s.class() == name_hash("Emitter") {
                let child = pointer("Child", vec![]);
                s.fields_mut().insert(name_hash("child"), child);
                assert_eq!(
                    s.path_of(name_hash("child")).last_field(),
                    Some((name_hash("Emitter"), name_hash("child")))
                );
            }
        });
        let (emitter, child) = (name_hash("Emitter"), name_hash("Child"));
        assert_eq!(
            classes,
            [name_hash("VfxSystem"), emitter, child, emitter, child]
        );
        let mut children = 0;
        structs(&object, |s| {
            children += usize::from(s.class() == child);
        });
        assert_eq!(children, 2);
    }

    #[test]
    fn a_path_spells_the_fields_a_table_names() {
        struct Fields;
        impl Spell for Fields {
            fn path(&self, _: crate::model::WadHash) -> Option<String> {
                None
            }
            fn entry(&self, _: BinHash) -> Option<String> {
                None
            }
            fn field(&self, hash: BinHash) -> Option<String> {
                (hash == name_hash("emitters")).then(|| "emitters".to_string())
            }
            fn class(&self, _: BinHash) -> Option<String> {
                None
            }
        }
        let path = FieldPath::root()
            .field(name_hash("VfxSystem"), name_hash("emitters"))
            .item(1)
            .field(name_hash("Emitter"), BinHash(0x1234));
        assert_eq!(path.spelled(&Fields), "emitters[1].00001234");
        assert_eq!(
            path.spelled(&NoNames),
            format!("{:08x}[1].00001234", name_hash("emitters").0)
        );
        assert_eq!(path.to_string(), path.spelled(&NoNames));
    }
}
