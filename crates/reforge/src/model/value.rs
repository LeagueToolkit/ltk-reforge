//! Reading bin values: fields by name, links, file references and digests.

use std::io::Cursor;

use ltk_meta::{BinObject, PropertyValueEnum};
use sha2::{Digest, Sha256};

use super::hash::{BinHash, WadHash, name_hash, path_hash};

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

/// Every object an object names, in order of appearance, without repeats.
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

/// Every file an object names, in order of appearance, without repeats.
pub fn file_refs(object: &BinObject) -> Vec<FileRef> {
    let mut out = Vec::new();
    walk_object(object, &mut |v| {
        if let Some(file) = as_file(v)
            && !matches!(v, PropertyValueEnum::Optional(_))
            && !out.contains(&file)
        {
            out.push(file);
        }
    });
    out
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
