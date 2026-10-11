//! The hash path of a property, and the node it reaches.
//!
//! Copied from LTK Manager's `crates/ltk-manager-bin/src/bin_document/path.rs`. Cut:
//! `HashPath::of` and the steps an edit builds. A key's text is `ltk_meta`'s `MapKey` as it
//! writes itself, the form `ltk_meta`'s `ValuePath` writes, so a place's path and a row's path
//! name one node alike.

use std::fmt::{self, Write as _};

use ltk_meta::path::MapKey;
use ltk_meta::property::values;
use ltk_meta::walk::TreeValue as _;
use ltk_meta::{BinObject, PropertyValueEnum};
use reforge::model::BinHash;

/// Whether an optional draws what it holds in place of a row of its own.
///
/// An optional is one value or none, so a leaf inside one is a row the reader has to open to
/// learn nothing the option row did not already say. What holds rows of its own keeps its
/// `[0]`, because those rows have to hang off something.
pub(super) fn inlines(value: &PropertyValueEnum) -> bool {
    !matches!(
        value,
        PropertyValueEnum::Container(_)
            | PropertyValueEnum::UnorderedContainer(_)
            | PropertyValueEnum::Map(_)
            | PropertyValueEnum::Optional(_)
            | PropertyValueEnum::Struct(_)
            | PropertyValueEnum::Embedded(_)
    )
}

/// Whether a `Struct` is the null pointer, which the format writes as a class hash of zero.
pub(super) fn is_null(inner: &values::Struct) -> bool {
    inner.class_hash.0 == 0
}

/// The struct a pointer or an embed holds, and `None` for any other value and a null pointer.
pub(super) fn as_struct(value: &PropertyValueEnum) -> Option<&values::Struct> {
    match value {
        PropertyValueEnum::Struct(inner) if !is_null(inner) => Some(inner),
        PropertyValueEnum::Embedded(values::Embedded(inner)) => Some(inner),
        _ => None,
    }
}

/// The items of a list, ordered or not, and `None` for any other value.
pub(super) fn as_list(value: &PropertyValueEnum) -> Option<&[PropertyValueEnum]> {
    match value {
        PropertyValueEnum::Container(items)
        | PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(items)) => {
            Some(items.items())
        }
        _ => None,
    }
}

/// The separator before a field segment: none at the start of a path.
pub(super) fn dot(prefix: &str) -> &'static str {
    if prefix.is_empty() { "" } else { "." }
}

/// One step of a hash path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Step {
    Field(BinHash),
    Index(usize),
    Key(EntryKey),
}

/// The step to one map entry: its key as [`key_text`] writes it, and which entry of that key.
///
/// A map the file writes with one key twice holds two entries a key alone cannot tell apart.
/// The first is `{key}`, which every address of a map without repeats keeps, and the later
/// ones are `{key}#1`, `{key}#2` and on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EntryKey {
    text: String,
    /// How many earlier entries of the map hold the same key.
    occurrence: usize,
}

impl EntryKey {
    /// The `occurrence`th entry of a map whose key is written `text`.
    pub(super) fn new(text: String, occurrence: usize) -> Self {
        Self { text, occurrence }
    }

    /// Where in `entries` the entry this names sits, or `None` where no entry is it.
    fn position(&self, entries: &[(PropertyValueEnum, PropertyValueEnum)]) -> Option<usize> {
        entries
            .iter()
            .enumerate()
            .filter(|(_, (key, _))| key_text(key) == self.text)
            .nth(self.occurrence)
            .map(|(at, _)| at)
    }
}

/// The step as a hash path writes it: `{key}`, then `#n` for a repeat.
impl fmt::Display for EntryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{{}}}", self.text)?;
        if self.occurrence > 0 {
            write!(f, "#{}", self.occurrence)?;
        }
        Ok(())
    }
}

/// A hash path written one step at a time, in the grammar [`parse_steps`] reads.
#[derive(Debug, Clone, Default)]
pub(super) struct HashPath(String);

impl HashPath {
    /// The path that the steps under the node at `path` extend.
    pub(super) fn under(path: &str) -> Self {
        Self(path.to_owned())
    }

    /// The path one step further, into the property `field`.
    #[must_use]
    pub(super) fn field(mut self, field: BinHash) -> Self {
        self.0.push_str(dot(&self.0));
        let _ = write!(self.0, "{:08x}", field.0);
        self
    }

    /// The path one step further, into the element `[index]`.
    #[must_use]
    pub(super) fn index(mut self, index: usize) -> Self {
        let _ = write!(self.0, "[{index}]");
        self
    }

    /// The path one step further, into the map entry `key` names.
    #[must_use]
    pub(super) fn key(mut self, key: &EntryKey) -> Self {
        let _ = write!(self.0, "{key}");
        self
    }
}

impl From<HashPath> for String {
    fn from(path: HashPath) -> Self {
        path.0
    }
}

/// The steps of a hash path, or `None` where the text is not one.
///
/// The grammar is `ltk_meta`'s `ValuePath`: `.` before every field but the first, eight hex
/// digits per field, `[i]` for an index and `{key}` for a map entry. A repeated key takes `#n`
/// after its braces, which [`EntryKey`] describes.
pub(super) fn parse_steps(path: &str) -> Option<Vec<Step>> {
    let mut steps = Vec::new();
    let mut rest = path;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('[') {
            let (digits, tail) = after.split_once(']')?;
            steps.push(Step::Index(digits.parse().ok()?));
            rest = tail;
        } else if let Some(after) = rest.strip_prefix('{') {
            let (key, tail) = split_key(after)?;
            let (occurrence, tail) = match tail.strip_prefix('#') {
                Some(count) => {
                    let end = count
                        .find(|character: char| !character.is_ascii_digit())
                        .unwrap_or(count.len());
                    (count[..end].parse().ok()?, &count[end..])
                }
                None => (0, tail),
            };
            steps.push(Step::Key(EntryKey::new(key.to_owned(), occurrence)));
            rest = tail;
        } else {
            let after = match (rest.strip_prefix('.'), steps.is_empty()) {
                (Some(after), false) => after,
                (None, true) => rest,
                _ => return None,
            };
            let (digits, tail) = after.split_at_checked(8)?;
            if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return None;
            }
            steps.push(Step::Field(BinHash(u32::from_str_radix(digits, 16).ok()?)));
            rest = tail;
        }
    }
    Some(steps)
}

/// The key text inside `{...}`, and what follows the closing brace.
///
/// A string key is a JSON string and may hold a brace. Its closing quote ends the key.
pub(super) fn split_key(text: &str) -> Option<(&str, &str)> {
    if !text.starts_with('"') {
        return text.split_once('}');
    }
    let mut escaped = false;
    for (at, character) in text.char_indices().skip(1) {
        match character {
            '\\' if !escaped => escaped = true,
            '"' if !escaped => {
                let end = at + 1;
                let tail = text[end..].strip_prefix('}')?;
                return Some((&text[..end], tail));
            }
            _ => escaped = false,
        }
    }
    None
}

/// A node a path resolves to.
#[derive(Clone, Copy)]
pub(super) enum Node<'a> {
    Object(&'a BinObject),
    Value(&'a PropertyValueEnum),
}

/// The properties of a struct, by field hash and in the file's order.
pub(super) type Properties = indexmap::IndexMap<BinHash, PropertyValueEnum>;

impl<'a> Node<'a> {
    /// The properties a field step reads. `None` for a leaf, a container and a null struct.
    pub(super) fn properties(self) -> Option<&'a Properties> {
        match self {
            Self::Object(object) => Some(&object.properties),
            Self::Value(value) => as_struct(value).map(|inner| &inner.properties),
        }
    }
}

/// What a step passed through, for the readable path of the node it reached.
pub(super) enum Trace<'a> {
    Field(BinHash),
    Index(usize),
    Key(&'a PropertyValueEnum),
}

/// Walk `steps` down from `node`, or `None` where a step reaches nothing.
pub(super) fn descend<'a>(
    mut node: Node<'a>,
    steps: &[Step],
) -> Option<(Node<'a>, Vec<Trace<'a>>)> {
    let mut trace = Vec::with_capacity(steps.len());
    for step in steps {
        node = match (step, node) {
            (Step::Field(field), node) => {
                let value = node.properties()?.get(field)?;
                trace.push(Trace::Field(*field));
                Node::Value(value)
            }
            (Step::Index(index), Node::Value(value)) => {
                let item = element(value, *index)?;
                trace.push(Trace::Index(*index));
                Node::Value(item)
            }
            (Step::Key(wanted), Node::Value(PropertyValueEnum::Map(map))) => {
                let (key, value) = &map.entries()[wanted.position(map.entries())?];
                trace.push(Trace::Key(key));
                Node::Value(value)
            }
            _ => return None,
        };
    }
    Some((node, trace))
}

/// The element `[index]` of a container, or the value of a present optional at `[0]`.
fn element(value: &PropertyValueEnum, index: usize) -> Option<&PropertyValueEnum> {
    match value {
        PropertyValueEnum::Optional(optional) if index == 0 => optional.value(),
        value => as_list(value)?.get(index),
    }
}

/// The text inside `{}` of a hash path, as `ltk_meta`'s `ValuePath` writes it. A key of a kind
/// no map is keyed by is `?`.
pub(super) fn key_text(key: &PropertyValueEnum) -> String {
    match key.map_key() {
        Ok(key) => key.to_string(),
        Err(_) => "?".to_owned(),
    }
}

/// A map key, owned, where the value is one a map is keyed by.
pub(super) fn map_key(key: &PropertyValueEnum) -> Option<MapKey> {
    key.map_key().ok()
}
