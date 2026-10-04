//! Reading bin values: fields by name, links, file references, digests and a value on one line.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use ltk_meta::{BinObject, PropertyKind as Kind, PropertyValueEnum};
use sha2::{Digest, Sha256};

use super::hash::{BinHash, WadHash, name_hash, path_hash};
use super::names::Spell;

/// The property at a chain of field names, through structs, embeds and options.
pub fn get<'a>(object: &'a BinObject, path: &[&str]) -> Option<&'a PropertyValueEnum> {
    let (first, rest) = path.split_first()?;
    let mut value = object.properties.get(&name_hash(first))?;
    for field in rest {
        value = field_of(value, field)?;
    }
    Some(value)
}

/// The field of a struct-like value.
pub fn field_of<'a>(value: &'a PropertyValueEnum, field: &str) -> Option<&'a PropertyValueEnum> {
    match value {
        PropertyValueEnum::Struct(s) => s.properties.get(&name_hash(field)),
        PropertyValueEnum::Embedded(e) => e.0.properties.get(&name_hash(field)),
        PropertyValueEnum::Optional(o) => o.value().and_then(|v| field_of(v, field)),
        _ => None,
    }
}

/// The hash keys of the map at an object's top-level field, such as a resolver's `resourceMap`.
pub fn map_keys(object: &BinObject, field: &str) -> Vec<BinHash> {
    match object.properties.get(&name_hash(field)) {
        Some(PropertyValueEnum::Map(map)) => map
            .entries()
            .iter()
            .filter_map(|(k, _)| match k {
                PropertyValueEnum::Hash(h) => Some(h.value),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The text of a string value, through an option.
pub fn as_str(value: &PropertyValueEnum) -> Option<&str> {
    match value {
        PropertyValueEnum::String(s) => Some(&s.value),
        PropertyValueEnum::Optional(o) => o.value().and_then(as_str),
        _ => None,
    }
}

/// The object a value names, through an option. A zero hash names nothing.
pub fn as_link(value: &PropertyValueEnum) -> Option<BinHash> {
    match value {
        PropertyValueEnum::ObjectLink(l) if l.value.0 != 0 => Some(l.value),
        PropertyValueEnum::Hash(h) if h.value.0 != 0 => Some(h.value),
        PropertyValueEnum::Optional(o) => o.value().and_then(as_link),
        _ => None,
    }
}

/// An integer value of any width up to 32 bits, through an option.
pub fn as_u32(value: &PropertyValueEnum) -> Option<u32> {
    Some(match value {
        PropertyValueEnum::U8(v) => u32::from(v.value),
        PropertyValueEnum::I8(v) => v.value as u32,
        PropertyValueEnum::U16(v) => u32::from(v.value),
        PropertyValueEnum::I16(v) => v.value as u32,
        PropertyValueEnum::U32(v) => v.value,
        PropertyValueEnum::I32(v) => v.value as u32,
        PropertyValueEnum::Optional(o) => return o.value().and_then(as_u32),
        _ => return None,
    })
}

/// The items of a list, of either kind.
pub fn items(value: &PropertyValueEnum) -> &[PropertyValueEnum] {
    match value {
        PropertyValueEnum::Container(c) => c.items(),
        PropertyValueEnum::UnorderedContainer(c) => c.0.items(),
        _ => &[],
    }
}

/// A file reference as a bin holds it: a path string, or in many fields a chunk hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileRef {
    /// A path, as the bin spells it.
    Path(String),
    /// A chunk hash.
    Hash(WadHash),
}

impl FileRef {
    /// The chunk hash the reference leads to.
    pub fn hash(&self) -> WadHash {
        match self {
            FileRef::Path(p) => path_hash(p),
            FileRef::Hash(h) => *h,
        }
    }

    /// The path, when the bin spells one.
    pub fn spelled(&self) -> Option<&str> {
        match self {
            FileRef::Path(p) => Some(p),
            FileRef::Hash(_) => None,
        }
    }
}

/// The file a value names, if it names one.
pub fn as_file(value: &PropertyValueEnum) -> Option<FileRef> {
    match value {
        PropertyValueEnum::String(s) if looks_like_path(&s.value) => {
            Some(FileRef::Path(s.value.clone()))
        }
        PropertyValueEnum::WadChunkLink(h) if h.value.0 != 0 => Some(FileRef::Hash(h.value)),
        PropertyValueEnum::Optional(o) => o.value().and_then(as_file),
        _ => None,
    }
}

/// Whether a string is taken for a file path, by its shape: a directory separator and an
/// alphanumeric extension of two to eight characters.
///
/// The history index holds what each game object names by this rule, so a change here changes
/// what an index built afterwards holds.
pub fn looks_like_path(text: &str) -> bool {
    if !text.contains('/') && !text.contains('\\') {
        return false;
    }
    let Some((_, ext)) = text.rsplit_once('.') else {
        return false;
    };
    (2..=8).contains(&ext.len()) && ext.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Every value under `value`, itself included, in pre-order.
pub fn walk<'a>(value: &'a PropertyValueEnum, visit: &mut impl FnMut(&'a PropertyValueEnum)) {
    visit(value);
    match value {
        PropertyValueEnum::Struct(s) => s.properties.values().for_each(|v| walk(v, visit)),
        PropertyValueEnum::Embedded(e) => e.0.properties.values().for_each(|v| walk(v, visit)),
        PropertyValueEnum::Container(c) => c.items().iter().for_each(|v| walk(v, visit)),
        PropertyValueEnum::UnorderedContainer(c) => c.0.items().iter().for_each(|v| walk(v, visit)),
        PropertyValueEnum::Optional(o) => {
            if let Some(v) = o.value() {
                walk(v, visit);
            }
        }
        PropertyValueEnum::Map(m) => {
            for (k, v) in m.entries() {
                walk(k, visit);
                walk(v, visit);
            }
        }
        _ => {}
    }
}

/// Every value of an object, in pre-order.
pub fn walk_object<'a>(object: &'a BinObject, visit: &mut impl FnMut(&'a PropertyValueEnum)) {
    for value in object.properties.values() {
        walk(value, visit);
    }
}

/// Every object an object names, in the pre-order of its values, without repeats. The order
/// decides which of two users of a file is its first (#ADR-0027).
pub fn links(object: &BinObject) -> Vec<BinHash> {
    let mut out = Vec::new();
    walk_object(object, &mut |v| {
        if let PropertyValueEnum::ObjectLink(l) = v
            && l.value.0 != 0
            && !out.contains(&l.value)
        {
            out.push(l.value);
        }
    });
    out
}

/// Every file an object names, in the pre-order of its values, without repeats.
pub fn file_refs(object: &BinObject) -> Vec<FileRef> {
    let mut out = Vec::new();
    walk_object(object, &mut |v| {
        // An option's value is visited itself.
        if let Some(file) = as_file(v)
            && !matches!(v, PropertyValueEnum::Optional(_))
            && !out.contains(&file)
        {
            out.push(file);
        }
    });
    out
}

/// Applies `change` to every value under `value`, itself first. A map's keys are left as they
/// are: the client looks an entry up by its key.
pub fn each_mut(value: &mut PropertyValueEnum, change: &mut impl FnMut(&mut PropertyValueEnum)) {
    change(value);
    let mut item_at = |slot: Option<ltk_meta::ValueSlot<'_>>| {
        if let Some(mut slot) = slot {
            let mut item = slot.get().clone();
            each_mut(&mut item, change);
            // The item keeps its kind, so the collection takes it back.
            let _ = slot.set(item);
        }
    };
    match value {
        PropertyValueEnum::Struct(s) => {
            let fields = s.properties.values_mut();
            fields.for_each(|v| each_mut(v, change));
        }
        PropertyValueEnum::Embedded(e) => {
            let fields = e.0.properties.values_mut();
            fields.for_each(|v| each_mut(v, change));
        }
        PropertyValueEnum::Container(c) => (0..c.len()).for_each(|i| item_at(c.slot(i))),
        PropertyValueEnum::UnorderedContainer(c) => {
            (0..c.0.len()).for_each(|i| item_at(c.0.slot(i)));
        }
        PropertyValueEnum::Optional(o) => item_at(o.slot()),
        PropertyValueEnum::Map(m) => (0..m.entries().len()).for_each(|i| item_at(m.slot(i))),
        _ => {}
    }
}

/// A copy of an object with each hash of `renames` replaced, its own entry hash included: the
/// links and the hash values it holds.
pub fn rename_hashes(object: &BinObject, renames: &[(BinHash, BinHash)]) -> BinObject {
    let map = |h: BinHash| {
        let found = renames.iter().find(|(from, _)| *from == h);
        found.map_or(h, |(_, to)| *to)
    };
    let mut out = object.clone();
    out.path_hash = map(out.path_hash);
    for value in out.properties.values_mut() {
        each_mut(value, &mut |v| match v {
            PropertyValueEnum::ObjectLink(l) => l.value = map(l.value),
            PropertyValueEnum::Hash(h) => h.value = map(h.value),
            _ => {}
        });
    }
    out
}

/// A copy of an object with each hash a table renames replaced, its own entry hash included.
/// A link always follows. A hash value that `keep` holds stays: the client looks such a value
/// up as a key and not as an object's name.
pub fn rename_hashes_by(
    object: &BinObject,
    renames: &HashMap<BinHash, BinHash>,
    keep: &dyn Fn(BinHash) -> bool,
) -> BinObject {
    let map = |h: BinHash| renames.get(&h).copied().unwrap_or(h);
    let mut out = object.clone();
    out.path_hash = map(out.path_hash);
    for value in out.properties.values_mut() {
        each_mut(value, &mut |v| match v {
            PropertyValueEnum::ObjectLink(l) => l.value = map(l.value),
            PropertyValueEnum::Hash(h) if !keep(h.value) => h.value = map(h.value),
            _ => {}
        });
    }
    out
}

/// Whether an object's entry hash, a link or a hash value of it is one of `hashes`.
pub fn names_any(object: &BinObject, hashes: &HashSet<BinHash>) -> bool {
    if hashes.contains(&object.path_hash) {
        return true;
    }
    let mut found = false;
    walk_object(object, &mut |v| match v {
        PropertyValueEnum::ObjectLink(l) => found |= hashes.contains(&l.value),
        PropertyValueEnum::Hash(h) => found |= hashes.contains(&h.value),
        _ => {}
    });
    found
}

/// Points each file reference of an object where `to` says, and keeps the reference's type:
/// a path string takes the path, a chunk hash the path's hash. Gives how many references
/// changed.
pub fn rewrite_files(object: &mut BinObject, to: &dyn Fn(WadHash) -> Option<String>) -> usize {
    let mut changed = 0;
    for value in object.properties.values_mut() {
        each_mut(value, &mut |v| match v {
            PropertyValueEnum::String(s) if looks_like_path(&s.value) => {
                if let Some(path) = to(path_hash(&s.value)) {
                    s.value = path;
                    changed += 1;
                }
            }
            PropertyValueEnum::WadChunkLink(h) if h.value.0 != 0 => {
                if let Some(path) = to(h.value) {
                    h.value = path_hash(&path);
                    changed += 1;
                }
            }
            _ => {}
        });
    }
    changed
}

/// Replaces each string an object holds for which `to` gives another, and gives how many
/// changed. A map's keys are left as they are.
pub fn rewrite_strings(
    object: &mut BinObject,
    to: &mut dyn FnMut(&str) -> Option<String>,
) -> usize {
    let mut changed = 0;
    for value in object.properties.values_mut() {
        each_mut(value, &mut |v| {
            if let PropertyValueEnum::String(s) = v
                && let Some(new) = to(&s.value)
            {
                s.value = new;
                changed += 1;
            }
        });
    }
    changed
}

/// Whether an object holds a string that `wanted` takes, a map's keys included.
pub fn any_string(object: &BinObject, wanted: &dyn Fn(&str) -> bool) -> bool {
    let mut found = false;
    walk_object(object, &mut |v| {
        if let PropertyValueEnum::String(s) = v {
            found = found || wanted(&s.value);
        }
    });
    found
}

/// A copy of a value with every file reference blanked, so that a copy whose files were moved
/// compares equal.
pub fn without_paths(value: &PropertyValueEnum) -> PropertyValueEnum {
    let mut out = value.clone();
    each_mut(&mut out, &mut |v| match v {
        PropertyValueEnum::String(s) if looks_like_path(&s.value) => s.value.clear(),
        PropertyValueEnum::WadChunkLink(h) => h.value.0 = 0,
        _ => {}
    });
    out
}

/// The name of a kind as the meta schema database and a report write it.
pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::None => "None",
        Kind::Bool => "Bool",
        Kind::I8 => "I8",
        Kind::U8 => "U8",
        Kind::I16 => "I16",
        Kind::U16 => "U16",
        Kind::I32 => "I32",
        Kind::U32 => "U32",
        Kind::I64 => "I64",
        Kind::U64 => "U64",
        Kind::F32 => "F32",
        Kind::Vector2 => "Vec2",
        Kind::Vector3 => "Vec3",
        Kind::Vector4 => "Vec4",
        Kind::Matrix44 => "Mtx44",
        Kind::Color => "Color",
        Kind::String => "String",
        Kind::Hash => "Hash",
        Kind::WadChunkLink => "File",
        Kind::Container => "List",
        Kind::UnorderedContainer => "List2",
        Kind::Struct => "Pointer",
        Kind::Embedded => "Embed",
        Kind::ObjectLink => "Link",
        Kind::Optional => "Option",
        Kind::Map => "Map",
        Kind::BitBool => "Flag",
    }
}

/// How many characters of a string [`brief`] shows before it cuts it.
pub const BRIEF_TEXT: usize = 96;

/// A value on one bounded line, for a person: a scalar whole, a string up to [`BRIEF_TEXT`]
/// characters, a struct as its class and how many fields it holds, a collection as how many
/// items of which kind. A string is the mod's own text: a caller cleans the line before it
/// prints it.
pub fn brief(value: &PropertyValueEnum, names: &dyn Spell) -> String {
    use PropertyValueEnum as V;
    let strukt = |s: &ltk_meta::property::values::Struct| match s.class_hash.0 {
        0 => "null".to_string(),
        _ => {
            let n = s.properties.len();
            let noun = if n == 1 { "field" } else { "fields" };
            format!("{} {{{n} {noun}}}", names.class_or_hex(s.class_hash))
        }
    };
    match value {
        V::None(_) => "none".to_string(),
        V::Bool(v) => v.value.to_string(),
        V::BitBool(v) => v.value.to_string(),
        V::I8(v) => v.value.to_string(),
        V::U8(v) => v.value.to_string(),
        V::I16(v) => v.value.to_string(),
        V::U16(v) => v.value.to_string(),
        V::I32(v) => v.value.to_string(),
        V::U32(v) => v.value.to_string(),
        V::I64(v) => v.value.to_string(),
        V::U64(v) => v.value.to_string(),
        V::F32(v) => format!("{:?}", v.value),
        V::Vector2(v) => format!("{:?}", v.value.to_array()),
        V::Vector3(v) => format!("{:?}", v.value.to_array()),
        V::Vector4(v) => format!("{:?}", v.value.to_array()),
        V::Matrix44(v) => format!("{:?}", v.value.to_cols_array()),
        V::Color(v) => format!("{:?}", v.value),
        V::String(v) if v.value.chars().count() > BRIEF_TEXT => {
            let head: String = v.value.chars().take(BRIEF_TEXT).collect();
            format!("{head:?}...")
        }
        V::String(v) => format!("{:?}", v.value),
        V::Hash(v) => format!("{:08x}", v.value.0),
        V::ObjectLink(v) => names.entry_or_hex(v.value),
        V::WadChunkLink(v) => names.path_or_hex(v.value),
        V::Struct(s) => strukt(s),
        V::Embedded(e) => strukt(&e.0),
        V::Container(c) => format!("[{} {}]", c.len(), kind_name(c.item_kind())),
        V::UnorderedContainer(c) => format!("[{} {}]", c.0.len(), kind_name(c.0.item_kind())),
        V::Optional(o) => match o.value() {
            Some(inner) => format!("some({})", brief(inner, names)),
            None => "none".to_string(),
        },
        V::Map(m) => format!(
            "{{{} {} to {}}}",
            m.entries().len(),
            kind_name(m.key_kind()),
            kind_name(m.value_kind())
        ),
    }
}

/// The SHA-256 of an object as `ltk_meta` writes it: equal objects give equal digests whatever
/// bin or build they came from.
///
/// # Panics
///
/// When `ltk_meta` fails to write into memory, which no value of an object makes it do.
pub fn digest(object: &BinObject) -> [u8; 32] {
    let mut out = Cursor::new(Vec::new());
    object.to_writer(&mut out).unwrap_or_else(|e| {
        panic!(
            "writing object {:08x} into memory failed: {e}",
            object.path_hash.0
        )
    });
    Sha256::digest(out.get_ref()).into()
}

#[cfg(test)]
mod tests {
    use ltk_meta::property::values;

    use super::*;

    fn object(entry: &str, link: &str) -> BinObject {
        BinObject::builder(name_hash(entry), name_hash("SkinCharacterDataProperties"))
            .property(
                name_hash("mResourceResolver"),
                values::ObjectLink::new(name_hash(link)),
            )
            .property(
                name_hash("championSkinName"),
                values::String::from("LuxSkin07"),
            )
            .property(
                name_hash("iconCircle"),
                values::String::from("ASSETS/Characters/Lux/HUD/Lux_Circle_7.tex"),
            )
            .build()
    }

    #[test]
    fn path_shape() {
        assert!(looks_like_path("ASSETS/Characters/Lux/Skins/Base/Lux.skn"));
        assert!(looks_like_path("data/x.bin"));
        assert!(!looks_like_path("LuxSkin07"));
        assert!(!looks_like_path("a/b.c"));
        assert!(!looks_like_path("a/b.toolongext"));
    }

    #[test]
    fn links_and_files() {
        let o = object(
            "Characters/Lux/Skins/Skin7",
            "Characters/Lux/Skins/Skin7/Resources",
        );
        assert_eq!(
            links(&o),
            vec![name_hash("Characters/Lux/Skins/Skin7/Resources")]
        );
        assert_eq!(file_refs(&o).len(), 1);
        assert_eq!(
            as_str(get(&o, &["championSkinName"]).unwrap()),
            Some("LuxSkin07")
        );
    }
}
