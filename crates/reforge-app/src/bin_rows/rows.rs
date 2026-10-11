//! A node's children, as the rows a viewer draws, and the names and values on them.
//!
//! Copied from LTK Manager's `crates/ltk-manager-bin/src/bin_document/rows.rs` and the
//! value half of `names.rs` (`Named::value_of`, `leaf_of`). Cut: the schema's lens, the
//! declared kind and the batched name tables. A name is asked of reforge's speller one hash
//! at a time, and every text a person reads is cleaned, as the mod may have spelled it.

use std::collections::HashMap;
use std::fmt::Write as _;

use ltk_meta::PropertyValueEnum;
use ltk_meta::path::MapKey;
use ltk_meta::property::values;
use ltk_meta::walk::{Leaf, TreeValue as _};
use reforge::game::Speller;
use reforge::model::{BinHash, Spell, WadHash, clean};

use super::path::{
    EntryKey, HashPath, Node, Trace, as_list, dot, inlines, is_null, key_text, map_key,
};
use super::types::{BinValue, RowNode};

/// A child of a node, with the segment that reaches it.
#[derive(Clone, Copy)]
pub(super) enum Child<'a> {
    Field(BinHash, &'a PropertyValueEnum),
    Element(usize, &'a PropertyValueEnum),
    /// A key, its value, and how many earlier entries of the map hold the same key.
    Entry(&'a PropertyValueEnum, &'a PropertyValueEnum, usize),
}

/// One child as its row names it: the segment that reaches it, in both forms.
pub(super) struct Segment<'a> {
    pub(super) value: &'a PropertyValueEnum,
    pub(super) node: RowNode,
    pub(super) name: String,
    pub(super) unnamed: bool,
    /// The child's hash path.
    pub(super) path: String,
    /// The segment for a person, with its separator.
    pub(super) readable: String,
}

impl<'a> Segment<'a> {
    /// The segment of `child` under the node at `path`, whose readable path is `parent_label`.
    pub(super) fn of(child: Child<'a>, path: &str, parent_label: &str, names: &Names<'_>) -> Self {
        match child {
            Child::Field(field, value) => {
                let (name, unnamed) = names.field(field);
                Self {
                    value,
                    node: RowNode::Property,
                    path: HashPath::under(path).field(field).into(),
                    readable: format!("{}{name}", dot(parent_label)),
                    name,
                    unnamed,
                }
            }
            Child::Element(index, value) => {
                let text = format!("[{index}]");
                Self {
                    value,
                    node: RowNode::Element,
                    name: text.clone(),
                    unnamed: false,
                    path: HashPath::under(path).index(index).into(),
                    readable: text,
                }
            }
            Child::Entry(key, value, occurrence) => {
                let (text, unnamed) = key_label(key, names);
                let repeat = if occurrence > 0 {
                    format!("#{occurrence}")
                } else {
                    String::new()
                };
                Self {
                    value,
                    node: RowNode::Entry,
                    path: HashPath::under(path)
                        .key(&EntryKey::new(key_text(key), occurrence))
                        .into(),
                    readable: format!("{{{text}}}{repeat}"),
                    name: text,
                    unnamed,
                }
            }
        }
    }
}

/// The children of `node`, in file order.
pub(super) fn children_of(node: Node<'_>) -> Vec<Child<'_>> {
    if let Some(properties) = node.properties() {
        return properties
            .iter()
            .map(|(field, value)| Child::Field(*field, value))
            .collect();
    }
    let Node::Value(value) = node else {
        return Vec::new();
    };
    if let Some(items) = as_list(value) {
        return items
            .iter()
            .enumerate()
            .map(|(index, value)| Child::Element(index, value))
            .collect();
    }

    match value {
        PropertyValueEnum::Optional(optional) => optional
            .value()
            .filter(|value| !inlines(value))
            .map(|value| Child::Element(0, value))
            .into_iter()
            .collect(),
        PropertyValueEnum::Map(map) => {
            let mut seen: HashMap<String, usize> = HashMap::new();
            map.entries()
                .iter()
                .map(|(key, value)| {
                    let count = seen.entry(key_text(key)).or_default();
                    let occurrence = *count;
                    *count += 1;
                    Child::Entry(key, value, occurrence)
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// The text inside `{}` for a person, and whether it is a hash nothing names.
///
/// A named `Hash` key is its string as a JSON literal. An unnamed one is `0x` and its hex
/// digits. Every other kind reads as it does in the hash path.
fn key_label(key: &PropertyValueEnum, names: &Names<'_>) -> (String, bool) {
    match map_key(key) {
        Some(MapKey::Hash(hash)) => match names.value(hash) {
            Some(name) => (json_string(&name), false),
            None => (hex(hash), true),
        },
        _ => (clean(&key_text(key)).into_owned(), false),
    }
}

/// A string as a JSON literal, cleaned.
fn json_string(text: &str) -> String {
    let json = serde_json::Value::String(text.to_owned()).to_string();
    clean(&json).into_owned()
}

/// The readable path of the node `trace` reached from a node whose readable path is `base`.
pub(super) fn label_of(base: &str, trace: &[Trace<'_>], names: &Names<'_>) -> String {
    let mut label = base.to_owned();
    for step in trace {
        match step {
            Trace::Field(field) => {
                label.push_str(dot(&label));
                label.push_str(&names.field(*field).0);
            }
            Trace::Index(index) => {
                let _ = write!(label, "[{index}]");
            }
            Trace::Key(key) => {
                let _ = write!(label, "{{{}}}", key_label(key, names).0);
            }
        }
    }
    label
}

/// A hash as a row prints one, which is `0x` and eight digits.
pub(super) fn hex(hash: BinHash) -> String {
    format!("0x{:08x}", hash.0)
}

/// The names a row reads: the run's names over the tables, through reforge's speller. Every
/// name is cleaned, as a name the run learnt can be the mod's own text.
pub(super) struct Names<'a> {
    speller: Speller<'a>,
}

impl<'a> Names<'a> {
    /// The names `speller` gives.
    pub(super) fn new(speller: &Speller<'a>) -> Self {
        Self { speller: *speller }
    }

    /// An object's path, or its hex and the flag that says so.
    pub(super) fn entry(&self, hash: BinHash) -> (String, bool) {
        named(self.speller.entry(hash), hash)
    }

    /// A property's name, or its hex and the flag that says so.
    pub(super) fn field(&self, hash: BinHash) -> (String, bool) {
        named(self.speller.field(hash), hash)
    }

    /// A class's name, where one is known.
    fn class(&self, hash: BinHash) -> Option<String> {
        self.speller.class(hash).map(cleaned)
    }

    /// The string behind a `Hash` value, where one is known.
    fn value(&self, hash: BinHash) -> Option<String> {
        self.speller.hash(hash).map(cleaned)
    }

    /// A chunk's path, where one is known.
    fn chunk(&self, hash: WadHash) -> Option<String> {
        self.speller.path(hash).map(cleaned)
    }

    /// `value` in the shape its widget draws.
    pub(super) fn value_of(&self, value: &PropertyValueEnum) -> BinValue {
        match value {
            PropertyValueEnum::Container(items)
            | PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(items)) => {
                BinValue::Container {
                    len: items.items().len(),
                    item_kind: items.item_kind().into(),
                }
            }
            PropertyValueEnum::Optional(optional) => match optional.value() {
                Some(inner) if inlines(inner) => self.value_of(inner),
                _ => BinValue::Optional {
                    present: optional.is_some(),
                    item_kind: optional.item_kind().into(),
                },
            },
            PropertyValueEnum::Map(map) => BinValue::Map {
                len: map.entries().len(),
                key_kind: map.key_kind().into(),
                value_kind: map.value_kind().into(),
            },
            PropertyValueEnum::Struct(inner) if is_null(inner) => BinValue::Null,
            PropertyValueEnum::Struct(inner)
            | PropertyValueEnum::Embedded(values::Embedded(inner)) => self.class_of(inner),
            leaf => self.leaf_of(leaf.as_leaf().ok().flatten()),
        }
    }

    /// A struct as its row's value: its class and how many properties it holds.
    pub(super) fn class_of(&self, inner: &values::Struct) -> BinValue {
        self.struct_value(inner.class_hash, inner.properties.len())
    }

    /// An object or a struct of `class` holding `len` properties, as its row's value.
    pub(super) fn struct_value(&self, class: BinHash, len: usize) -> BinValue {
        BinValue::Struct {
            class_hash: hex(class),
            class: self.class(class),
            len,
        }
    }

    fn leaf_of(&self, leaf: Option<Leaf<'_>>) -> BinValue {
        match leaf {
            None | Some(Leaf::None) => BinValue::None,
            Some(Leaf::Bool(value) | Leaf::Flag(value)) => BinValue::Bool { value },
            Some(Leaf::I8(value)) => integer(value),
            Some(Leaf::U8(value)) => integer(value),
            Some(Leaf::I16(value)) => integer(value),
            Some(Leaf::U16(value)) => integer(value),
            Some(Leaf::I32(value)) => integer(value),
            Some(Leaf::U32(value)) => integer(value),
            Some(Leaf::I64(value)) => integer(value),
            Some(Leaf::U64(value)) => integer(value),
            Some(Leaf::F32(value)) => BinValue::Float { value },
            Some(Leaf::Vector2(vector)) => BinValue::Vector {
                values: vector.to_array().to_vec(),
            },
            Some(Leaf::Vector3(vector)) => BinValue::Vector {
                values: vector.to_array().to_vec(),
            },
            Some(Leaf::Vector4(vector)) => BinValue::Vector {
                values: vector.to_array().to_vec(),
            },
            Some(Leaf::Matrix44(matrix)) => BinValue::Matrix {
                values: matrix.transpose().to_cols_array().to_vec(),
            },
            Some(Leaf::Color(color)) => BinValue::Color {
                r: color.r,
                g: color.g,
                b: color.b,
                a: color.a,
            },
            Some(Leaf::String(text)) => BinValue::String {
                value: cleaned(text.to_owned()),
            },
            Some(Leaf::Hash(hash)) => BinValue::Hash {
                hash: hex(hash),
                name: self.value(hash),
            },
            Some(Leaf::File(hash)) => BinValue::WadChunkLink {
                hash: format!("{:016x}", hash.0),
                path: self.chunk(hash),
            },
            Some(Leaf::Link(hash)) => BinValue::ObjectLink {
                hash: hex(hash),
                name: self.speller.entry(hash).map(cleaned),
            },
            Some(_) => BinValue::Undrawn,
        }
    }
}

/// A name cleaned, or the hash's hex and the flag that says it is one.
fn named(name: Option<String>, hash: BinHash) -> (String, bool) {
    match name {
        Some(name) => (cleaned(name), false),
        None => (hex(hash), true),
    }
}

/// `text` with every character a mod could hide behind escaped.
pub(super) fn cleaned(text: String) -> String {
    match clean(&text) {
        std::borrow::Cow::Borrowed(_) => text,
        std::borrow::Cow::Owned(escaped) => escaped,
    }
}

fn integer(value: impl std::fmt::Display) -> BinValue {
    BinValue::Integer {
        text: value.to_string(),
    }
}
