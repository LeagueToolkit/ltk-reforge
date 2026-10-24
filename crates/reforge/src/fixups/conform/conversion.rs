//! The conversion catalogue: each way a value is put in the type the target build reads, as
//! a named item with one function. [`conform`] picks the conversion a value and a type ask
//! for, and applies it to the property whole.
//!
//! A conversion changes how a value is written and never what it names. One that hashes a
//! path away gives the path to the run's names.

use std::fmt;

use ltk_hash::{HashAlgorithm, HashWidth, Hasher};
use ltk_meta::property::values;
use ltk_meta::{PropertyKind as Kind, PropertyValueEnum as V};
use serde::Serialize;

use crate::fixups::FixCx;
use crate::game::{FieldType, NameHashError, Names, Speller};
use crate::model::entity::Character;
use crate::model::{Arena, BinHash, Record, WadHash, name_hash, path_hash, value};

/// One way a value is put in the type the target build reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Conversion {
    /// The same encoding under another tag.
    Retag,
    /// A path as the hash of the chunk it names.
    HashValue,
    /// The name hash of a path as the hash of the chunk the path names.
    Rehash,
    /// A string as the 8-byte hash the build reads.
    WideHash,
    /// A map with its keys, and its values, each converted.
    HashKey,
    /// An integer in a wider kind that holds every value of its own.
    Widen,
    /// An integer in another kind that holds its value.
    Narrow,
    /// A present option's value in the item kind the build reads.
    Optional,
    /// No value as a pointer to nothing.
    NullPointer,
    /// An empty option as an empty option of another item kind.
    EmptyOption,
}

impl Conversion {
    /// Every conversion, in the catalogue's order.
    pub const ALL: [Self; 10] = [
        Self::Retag,
        Self::HashValue,
        Self::Rehash,
        Self::WideHash,
        Self::HashKey,
        Self::Widen,
        Self::Narrow,
        Self::Optional,
        Self::NullPointer,
        Self::EmptyOption,
    ];

    /// The name a listing shows.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Retag => "retag",
            Self::HashValue => "hash-value",
            Self::Rehash => "rehash",
            Self::WideHash => "wide-hash",
            Self::HashKey => "hash-key",
            Self::Widen => "widen",
            Self::Narrow => "narrow",
            Self::Optional => "optional",
            Self::NullPointer => "null-pointer",
            Self::EmptyOption => "empty-option",
        }
    }

    /// The kinds it takes a value between, as the meta schema names kinds.
    pub const fn takes(self) -> &'static str {
        match self {
            Self::Retag => "Embed and Pointer, List and List2, Bool and Flag, Link and Hash",
            Self::HashValue => "String to File",
            Self::Rehash => "Hash to File",
            Self::WideHash => "String to Hash of 8 bytes",
            Self::HashKey => "Map to Map of other key and value kinds",
            Self::Widen => "an integer to a wider integer of every value it has",
            Self::Narrow => "an integer to any other integer",
            Self::Optional => "a present Option to Option of a kind its value converts to",
            Self::NullPointer => "None to Pointer",
            Self::EmptyOption => "an empty Option to Option of another item kind",
        }
    }

    /// What it does to the value.
    pub const fn how(self) -> &'static str {
        match self {
            Self::Retag => "the same encoding under the other tag; a list's items are converted",
            Self::HashValue => "the xxh64 of the lowercased path; an empty string is the null hash",
            Self::Rehash => "the path the names spell under that name hash, hashed as a file",
            Self::WideHash => "the xxh3-64 of the lowercased string; an empty string is left",
            Self::HashKey => "each key and each value by its own conversion; one refuses the map",
            Self::Widen => "the same number",
            Self::Narrow => "the same number, where the kind holds it",
            Self::Optional => "the value by its own conversion, present",
            Self::NullPointer => "a pointer of the zero class",
            Self::EmptyOption => "the item's tag alone",
        }
    }

    /// The records it follows: the change of Riot's that made the old kind wrong, or the
    /// tool's practice that wrote it.
    pub const fn records(self) -> &'static [Record] {
        const FILES_AS_HASHES: &[Record] = &[Record::game_change(13)];
        const LINKS_AS_HASHES: &[Record] = &[Record::game_change(19)];
        const INTEGERS_TOO_WIDE: &[Record] = &[Record::game_change(20), Record::mod_artifact(21)];
        const NAMES_AS_WIDE_HASHES: &[Record] = &[Record::game_change(28)];
        match self {
            Self::Retag => LINKS_AS_HASHES,
            Self::HashValue | Self::Rehash | Self::HashKey => FILES_AS_HASHES,
            Self::WideHash => NAMES_AS_WIDE_HASHES,
            Self::Narrow => INTEGERS_TOO_WIDE,
            Self::Widen | Self::Optional | Self::NullPointer | Self::EmptyOption => &[],
        }
    }

    /// Whether the mod manager makes it as it installs a mod (#MOD-ARTIFACT-0022), for a
    /// value of the kind `from`: the author may have seen the value converted so.
    pub fn at_install(self, from: Kind) -> bool {
        match self {
            Self::Retag => !matches!(from, Kind::ObjectLink | Kind::Hash),
            // The manager's rule predates the 8-byte hash.
            Self::Narrow | Self::Optional | Self::WideHash => false,
            _ => true,
        }
    }

    /// The conversion that takes a value of one kind to another kind. None when no conversion
    /// does. A value whose kind is the one read is not asked here: its items go each by this,
    /// a map by [`Conversion::HashKey`], a present option by [`Conversion::Optional`] and an
    /// empty one by [`Conversion::EmptyOption`].
    pub fn between(from: Kind, to: Kind) -> Option<Self> {
        use Kind as K;
        Some(match (from, to) {
            // #GAME-CHANGE-0021: Riot put no value into the option it made of a field.
            (_, K::Optional) => return None,
            (K::String, K::WadChunkLink) => Self::HashValue,
            (K::Hash, K::WadChunkLink) => Self::Rehash,
            (K::String, K::Hash) => Self::WideHash,
            (K::None, K::Struct) => Self::NullPointer,
            (K::Embedded, K::Struct)
            | (K::Struct, K::Embedded)
            | (K::Container, K::UnorderedContainer)
            | (K::UnorderedContainer, K::Container)
            | (K::Bool, K::BitBool)
            | (K::BitBool, K::Bool)
            | (K::ObjectLink, K::Hash)
            | (K::Hash, K::ObjectLink) => Self::Retag,
            _ if Int::of(to)?.widens(Int::of(from)?) => Self::Widen,
            _ => Self::Narrow,
        })
    }

    /// The value by this conversion, as the type the build reads. None where the conversion
    /// does not take the value.
    pub(super) fn apply(self, value: &V, want: &FieldType, names: &mut PathNames<'_>) -> Option<V> {
        match self {
            Self::Retag => retag(value, want, names),
            Self::HashValue => hash_value(value, names),
            Self::Rehash => rehash(value, names),
            Self::WideHash => wide_hash(value, want),
            Self::HashKey => hash_key(value, want, names),
            Self::Widen => widen(value, want.kind),
            Self::Narrow => narrow(value, want.kind),
            Self::Optional => optional(value, want, names),
            Self::NullPointer => null_pointer(value),
            Self::EmptyOption => empty_option(value, want),
        }
    }
}

impl fmt::Display for Conversion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

/// An integer kind.
#[derive(Clone, Copy)]
struct Int {
    bits: u8,
    signed: bool,
}

impl Int {
    fn of(kind: Kind) -> Option<Self> {
        let (bits, signed) = match kind {
            Kind::U8 => (8, false),
            Kind::U16 => (16, false),
            Kind::U32 => (32, false),
            Kind::U64 => (64, false),
            Kind::I8 => (8, true),
            Kind::I16 => (16, true),
            Kind::I32 => (32, true),
            Kind::I64 => (64, true),
            _ => return None,
        };
        Some(Self { bits, signed })
    }

    /// Whether this kind is wider than another and holds every value of it.
    fn widens(self, from: Self) -> bool {
        self.bits > from.bits && (self.signed || !from.signed)
    }
}

/// The type of one item a collection of type `of` holds: a kind with no item and no class, and
/// the collection's hash width.
fn item_of(of: &FieldType, kind: Kind) -> FieldType {
    FieldType {
        kind,
        key: None,
        item: None,
        class: None,
        width: of.width,
    }
}

/// One value as a type: itself when it is of the type's kind, else by the conversion between
/// the two.
fn value_as(value: &V, to: &FieldType, names: &mut PathNames<'_>) -> Option<V> {
    if value.kind() == to.kind {
        return Some(value.clone());
    }
    Conversion::between(value.kind(), to.kind)?.apply(value, to, names)
}

/// Every value as a type, or none of them.
fn values_as<'v>(
    items: impl IntoIterator<Item = &'v V>,
    to: &FieldType,
    names: &mut PathNames<'_>,
) -> Option<Vec<V>> {
    let mut items = items.into_iter();
    items.try_fold(Vec::new(), |mut out, item| {
        out.push(value_as(item, to, names)?);
        Some(out)
    })
}

/// A list of type `of` with its items as a kind.
fn list_as(
    list: &values::Container,
    of: &FieldType,
    item: Kind,
    names: &mut PathNames<'_>,
) -> Option<values::Container> {
    let items = values_as(list.items(), &item_of(of, item), names)?;
    values::Container::new(item, items).ok()
}

fn retag(value: &V, want: &FieldType, names: &mut PathNames<'_>) -> Option<V> {
    use Kind as K;
    Some(match (value, want.kind) {
        (V::Embedded(embed), K::Struct) => V::Struct(embed.0.clone()),
        (V::Struct(pointer), K::Embedded) => V::Embedded(values::Embedded(pointer.clone())),
        (V::Container(list), K::UnorderedContainer) => {
            let item = want.item.unwrap_or(list.item_kind());
            let list = list_as(list, want, item, names)?;
            V::UnorderedContainer(values::UnorderedContainer(list))
        }
        (V::UnorderedContainer(list), K::Container) => {
            let item = want.item.unwrap_or(list.0.item_kind());
            V::Container(list_as(&list.0, want, item, names)?)
        }
        (V::Bool(flag), K::BitBool) => V::BitBool(values::BitBool::new(flag.value)),
        (V::BitBool(flag), K::Bool) => V::Bool(values::Bool::new(flag.value)),
        (V::ObjectLink(link), K::Hash) => V::Hash(values::Hash::new(link.value)),
        (V::Hash(hash), K::ObjectLink) => {
            V::ObjectLink(values::ObjectLink::new(value::entry_hash(hash.value)?))
        }
        _ => return None,
    })
}

fn hash_value(value: &V, names: &mut PathNames<'_>) -> Option<V> {
    match value {
        V::String(path) => Some(names.file(&path.value)),
        _ => None,
    }
}

fn rehash(value: &V, names: &mut PathNames<'_>) -> Option<V> {
    let V::Hash(hash) = value else {
        return None;
    };
    let hash = value::entry_hash(hash.value)?;
    if hash.0 == 0 {
        return Some(names.file(""));
    }
    let path = names.path_named(hash)?;
    Some(names.file(&path))
}

/// The 8-byte hash of a material's name, as the client stores it (#GAME-CHANGE-0028).
const WIDE_NAME: Hasher = Hasher {
    width: HashWidth::W8,
    algorithm: HashAlgorithm::Xxh3,
    lowercased: true,
};

fn wide_hash(value: &V, want: &FieldType) -> Option<V> {
    let V::String(text) = value else {
        return None;
    };
    // #GAME-CHANGE-0028 shows the conversion for the 8-byte width alone, and no game value
    // shows what an empty string became.
    if want.width != Some(HashWidth::W8) || text.value.is_empty() {
        return None;
    }
    Some(V::Hash(values::Hash::new(WIDE_NAME.hash_str(&text.value))))
}

fn hash_key(value: &V, want: &FieldType, names: &mut PathNames<'_>) -> Option<V> {
    let V::Map(map) = value else {
        return None;
    };
    let key = want.key.unwrap_or(map.key_kind());
    let item = want.item.unwrap_or(map.value_kind());
    let keys = map.entries().iter().map(|(key, _)| key);
    let keys = values_as(keys, &item_of(want, key), names)?;
    let items = map.entries().iter().map(|(_, item)| item);
    let items = values_as(items, &item_of(want, item), names)?;
    let entries = keys.into_iter().zip(items).collect();
    values::Map::new(key, item, entries).ok().map(V::Map)
}

/// The number an integer value holds.
fn number(value: &V) -> Option<i128> {
    Some(match value {
        V::U8(x) => x.value.into(),
        V::U16(x) => x.value.into(),
        V::U32(x) => x.value.into(),
        V::U64(x) => x.value.into(),
        V::I8(x) => x.value.into(),
        V::I16(x) => x.value.into(),
        V::I32(x) => x.value.into(),
        V::I64(x) => x.value.into(),
        _ => return None,
    })
}

/// A number as an integer kind, where the kind holds it.
fn integer(number: i128, to: Kind) -> Option<V> {
    Some(match to {
        Kind::U8 => V::U8(values::U8::new(number.try_into().ok()?)),
        Kind::U16 => V::U16(values::U16::new(number.try_into().ok()?)),
        Kind::U32 => V::U32(values::U32::new(number.try_into().ok()?)),
        Kind::U64 => V::U64(values::U64::new(number.try_into().ok()?)),
        Kind::I8 => V::I8(values::I8::new(number.try_into().ok()?)),
        Kind::I16 => V::I16(values::I16::new(number.try_into().ok()?)),
        Kind::I32 => V::I32(values::I32::new(number.try_into().ok()?)),
        Kind::I64 => V::I64(values::I64::new(number.try_into().ok()?)),
        _ => return None,
    })
}

fn widen(value: &V, to: Kind) -> Option<V> {
    let wider = Int::of(to)?.widens(Int::of(value.kind())?);
    wider.then(|| integer(number(value)?, to)).flatten()
}

fn narrow(value: &V, to: Kind) -> Option<V> {
    integer(number(value)?, to)
}

fn optional(value: &V, want: &FieldType, names: &mut PathNames<'_>) -> Option<V> {
    let item = want.item?;
    let present = value_as(value, &item_of(want, item), names)?;
    let option = values::Optional::new(item, Some(present));
    option.ok().map(V::Optional)
}

fn null_pointer(value: &V) -> Option<V> {
    matches!(value, V::None(_)).then(|| V::Struct(values::Struct::default()))
}

fn empty_option(value: &V, want: &FieldType) -> Option<V> {
    match value {
        V::Optional(option) if option.value().is_none() => {
            values::Optional::empty(want.item?).ok().map(V::Optional)
        }
        _ => None,
    }
}

/// A value as the type the build reads, the property whole. `Ok(None)` when the value has
/// that type, `Err` with the kind no conversion takes it to. A struct under the value is not
/// entered: the walk visits it.
pub(super) fn conform(
    value: &V,
    want: &FieldType,
    names: &mut PathNames<'_>,
) -> Result<Option<V>, Kind> {
    if value.kind() != want.kind {
        let road = Conversion::between(value.kind(), want.kind);
        let converted = road.and_then(|road| road.apply(value, want, names));
        // A list has a conversion to the other list: one that stays has an item with none.
        let listed = matches!(value, V::Container(_) | V::UnorderedContainer(_));
        let wants = match want.item {
            Some(item) if listed && road == Some(Conversion::Retag) => item,
            _ => want.kind,
        };
        return converted.map(Some).ok_or(wants);
    }
    // The kind the build reads inside, where it is known and is not the one held.
    let other = |held: Kind, read: Option<Kind>| read.filter(|read| *read != held);
    match value {
        V::Container(list) => {
            let Some(item) = other(list.item_kind(), want.item) else {
                return Ok(None);
            };
            let list = list_as(list, want, item, names).map(V::Container);
            list.map(Some).ok_or(item)
        }
        V::UnorderedContainer(list) => {
            let Some(item) = other(list.0.item_kind(), want.item) else {
                return Ok(None);
            };
            let list = list_as(&list.0, want, item, names).map(values::UnorderedContainer);
            list.map(|list| Some(V::UnorderedContainer(list)))
                .ok_or(item)
        }
        V::Optional(option) => {
            let Some(item) = other(option.item_kind(), want.item) else {
                return Ok(None);
            };
            let converted = match option.value() {
                Some(present) => Conversion::Optional.apply(present, want, names),
                None => Conversion::EmptyOption.apply(value, want, names),
            };
            converted.map(Some).ok_or(item)
        }
        V::Map(map) => {
            let key = other(map.key_kind(), want.key);
            let Some(wants) = key.or(other(map.value_kind(), want.item)) else {
                return Ok(None);
            };
            let converted = Conversion::HashKey.apply(value, want, names);
            converted.map(Some).ok_or(wants)
        }
        _ => Ok(None),
    }
}

/// The paths the conversions hash away and look for by name hash.
pub(super) struct PathNames<'a> {
    speller: Speller<'a>,
    tables: &'a Names,
    characters: &'a Arena<Character>,
    hashed: Vec<String>,
    fault: Option<NameHashError>,
}

impl<'a> PathNames<'a> {
    /// The names of a run, with no path hashed.
    pub(super) fn of(cx: &'a FixCx<'_>) -> Self {
        Self {
            speller: cx.speller(),
            tables: cx.tables(),
            characters: cx.characters(),
            hashed: Vec::new(),
            fault: None,
        }
    }

    /// A path as the file it names. The path is kept for the run's names.
    fn file(&mut self, path: &str) -> V {
        let hash = if path.is_empty() {
            WadHash(0)
        } else {
            self.hashed.push(path.to_ascii_lowercase());
            path_hash(path)
        };
        V::WadChunkLink(values::WadChunkLink::new(hash))
    }

    /// The path a name hash is of: the plaintext the names hold for the hash, else the first
    /// path the run learnt with that name hash, else the one path the tables spell with it,
    /// else a path with it under a folder of the mod's characters.
    fn path_named(&mut self, hash: BinHash) -> Option<String> {
        if let Some(name) = self.speller.hash(hash) {
            return Some(name);
        }
        match self.speller.path_by_name_hash(hash) {
            Ok(Some(path)) => return Some(path),
            Ok(None) => {}
            Err(error) => {
                self.fault.get_or_insert(error);
                return None;
            }
        }
        // Two paths of the tables share the name hash, or none has it. Under the characters'
        // own folders few paths are left to share one.
        let folders = self.characters.values().flat_map(|character| {
            let name = &character.name;
            [
                format!("assets/characters/{name}/"),
                format!("data/characters/{name}/"),
            ]
        });
        let paths = folders.flat_map(|folder| self.tables.paths_with_prefix(&folder));
        paths.filter(|path| name_hash(path) == hash).last()
    }

    /// The paths hashed away, lowercased, and the first fault of a name table.
    pub(super) fn finish(self) -> (Vec<String>, Option<NameHashError>) {
        (self.hashed, self.fault)
    }
}
