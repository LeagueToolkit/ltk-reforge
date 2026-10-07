//! The meta schema database: the type every field of every class has at every build, as the LTK
//! Meta Wiki publishes it and the League Toolkit programs keep it beside the hash cache
//! (`%LOCALAPPDATA%/LeagueToolkit/meta/meta-schema.json`). Read only.
//!
//! ```json
//! { "formatVersion": 1, "latest": 8217343, "versions": [{ "patch": "16.19", "build": 8217343 }],
//!   "classes": { "0x9b67e9f6": { "name": "SkinCharacterDataProperties",
//!       "revisions": [{ "from": 5229820, "bases": [] }],
//!       "properties": { "0x89aff69": { "name": "iconAvatar", "revisions": [
//!           { "from": 5229820, "to": 8049184, "type": ["String", "0x0", "0x0", "0x0"] },
//!           { "from": 8104348, "type": ["File", "0x0", "0x0", "0x0"] } ] } } } } }
//! ```
//!
//! A type is `[kind, map key kind, item or value kind, class]`. A slot that does not apply holds
//! a hex number. A build is its number, `8217343`.

use std::collections::HashMap;
use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use ltk_meta::{PropertyKind as Kind, PropertyValueEnum};
use serde::{Deserialize, Serialize};

use super::{Build, mimir_cache_dir};
use crate::diag::{Channel, Code, Diag, Input, Message, Severity, Sink, Subject};
use crate::model::BinHash;

/// The one format this reader reads.
const FORMAT: u32 = 1;

/// The database's file in the folder the League Toolkit programs keep it in.
pub const SCHEMA_FILE: &str = "meta-schema.json";

/// The folder the League Toolkit programs keep the schema database in on this machine: `meta`
/// beside the hash cache, as LTK Manager places it. The folder may not exist. `None` when the
/// platform names no data directory.
pub fn meta_cache_dir() -> Option<Utf8PathBuf> {
    Some(mimir_cache_dir()?.parent()?.join("meta"))
}

/// Why a meta schema database does not read.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// The file does not read.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// The file is not the JSON of a schema database.
    #[error("not a meta schema database")]
    Json(#[from] serde_json::Error),
    /// The database is of a format this reader does not read.
    #[error("schema format {found}, and this reads format {FORMAT}")]
    Format {
        /// The format the database states.
        found: u32,
    },
    /// A class, a field or a base is not written as a hex hash.
    #[error("{text:?} is no hash")]
    Hash {
        /// The text in the hash's place.
        text: String,
    },
    /// A type names a kind this reader does not know. Skipping it would read the field as one
    /// the class does not declare.
    #[error("{name:?} is no kind of value")]
    Kind {
        /// The kind's name.
        name: String,
    },
}

/// The type a build reads for one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldType {
    /// The value's kind.
    pub kind: Kind,
    /// A map's key kind.
    pub key: Option<Kind>,
    /// A container's or an option's item kind, a map's value kind.
    pub item: Option<Kind>,
    /// The class of an embed or a pointer, or of a container's items.
    pub class: Option<BinHash>,
}

impl FieldType {
    /// The type a value has by its shape: its kind, a collection's key and item kinds, and
    /// the class of the struct it is or of the first struct it holds. What a bin of the game
    /// witnesses of a field the database does not describe.
    pub fn of(value: &PropertyValueEnum) -> Self {
        use PropertyValueEnum as V;
        let class_of = |value: &V| match value {
            V::Embedded(e) => Some(e.0.class_hash),
            V::Struct(s) => Some(s.class_hash),
            _ => None,
        };
        let first_class = |items: &[V]| items.iter().filter_map(class_of).find(|c| c.0 != 0);
        let (key, item, class) = match value {
            V::Container(c) => (None, Some(c.item_kind()), first_class(c.items())),
            V::UnorderedContainer(c) => (None, Some(c.0.item_kind()), first_class(c.0.items())),
            V::Optional(o) => (
                None,
                Some(o.item_kind()),
                o.value().and_then(class_of).filter(|c| c.0 != 0),
            ),
            V::Map(m) => {
                let first = m.entries().first().and_then(|(_, value)| class_of(value));
                (
                    Some(m.key_kind()),
                    Some(m.value_kind()),
                    first.filter(|c| c.0 != 0),
                )
            }
            V::Embedded(e) => (None, None, Some(e.0.class_hash)),
            V::Struct(s) => (None, None, Some(s.class_hash).filter(|c| c.0 != 0)),
            _ => (None, None, None),
        };
        Self {
            kind: value.kind(),
            key,
            item,
            class,
        }
    }

    fn parse(slots: &[String; 4]) -> Result<Self, SchemaError> {
        let [kind, key, item, class] = slots;
        let kind = kind_named(kind)?;
        Ok(Self {
            kind,
            // A list's second slot holds a number that is no key kind.
            key: (kind == Kind::Map).then(|| kind_named(key)).transpose()?,
            item: if item.starts_with("0x") {
                None
            } else {
                Some(kind_named(item)?)
            },
            class: Some(hash(class)?).filter(|class| class.0 != 0),
        })
    }
}

fn hash(text: &str) -> Result<BinHash, SchemaError> {
    text.strip_prefix("0x")
        .and_then(|digits| u32::from_str_radix(digits, 16).ok())
        .map(BinHash)
        .ok_or_else(|| SchemaError::Hash {
            text: text.to_string(),
        })
}

/// The database's name for a kind, as `ltk_meta` knows the kind.
fn kind_named(name: &str) -> Result<Kind, SchemaError> {
    Ok(match name {
        "None" => Kind::None,
        "Bool" => Kind::Bool,
        "I8" => Kind::I8,
        "U8" => Kind::U8,
        "I16" => Kind::I16,
        "U16" => Kind::U16,
        "I32" => Kind::I32,
        "U32" => Kind::U32,
        "I64" => Kind::I64,
        "U64" => Kind::U64,
        "F32" => Kind::F32,
        "Vec2" => Kind::Vector2,
        "Vec3" => Kind::Vector3,
        "Vec4" => Kind::Vector4,
        "Mtx44" => Kind::Matrix44,
        "Color" => Kind::Color,
        "String" => Kind::String,
        "Hash" => Kind::Hash,
        "File" => Kind::WadChunkLink,
        "List" => Kind::Container,
        "List2" => Kind::UnorderedContainer,
        "Pointer" => Kind::Struct,
        "Embed" => Kind::Embedded,
        "Link" => Kind::ObjectLink,
        "Option" => Kind::Optional,
        "Map" => Kind::Map,
        "Flag" => Kind::BitBool,
        _ => {
            return Err(SchemaError::Kind {
                name: name.to_string(),
            });
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDb {
    format_version: u32,
    latest: u64,
    #[serde(default)]
    hash_source: RawHashSource,
    classes: HashMap<String, RawClass>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawHashSource {
    fetched_at: Option<String>,
}

#[derive(Deserialize)]
struct RawClass {
    #[serde(default)]
    revisions: Vec<RawClassRevision>,
    #[serde(default)]
    properties: HashMap<String, RawProperty>,
}

#[derive(Deserialize)]
struct RawClassRevision {
    from: u64,
    to: Option<u64>,
    #[serde(default)]
    bases: Vec<String>,
}

#[derive(Deserialize)]
struct RawProperty {
    revisions: Vec<RawPropertyRevision>,
}

#[derive(Deserialize)]
struct RawPropertyRevision {
    from: u64,
    to: Option<u64>,
    #[serde(rename = "type")]
    slots: [String; 4],
}

/// A value that holds from one build to another, both included, or to the newest.
struct Span<T> {
    from: u64,
    to: Option<u64>,
    value: T,
}

impl<T> Span<T> {
    /// Whether the value holds at some build from `lo` to `hi`.
    fn overlaps(&self, lo: u64, hi: u64) -> bool {
        self.from <= hi && self.to.is_none_or(|to| to >= lo)
    }
}

/// A class and its bases over a range of builds, each once, the class itself first.
struct Lineage<'a> {
    schema: &'a Schema,
    lo: u64,
    hi: u64,
    stack: Vec<BinHash>,
    seen: Vec<BinHash>,
}

impl Iterator for Lineage<'_> {
    type Item = BinHash;

    fn next(&mut self) -> Option<BinHash> {
        loop {
            let class = self.stack.pop()?;
            // A class reached by two bases is read once, and a cycle ends.
            if self.seen.contains(&class) {
                continue;
            }
            self.seen.push(class);
            let revisions = self.schema.bases.get(&class).into_iter().flatten();
            for revision in revisions.filter(|span| span.overlaps(self.lo, self.hi)) {
                self.stack.extend(&revision.value);
            }
            return Some(class);
        }
    }
}

/// The meta schema database, parsed.
pub struct Schema {
    latest: u64,
    earliest: u64,
    generation: Option<String>,
    /// A class's revisions, each with its bases. A class with no entry is not known.
    bases: HashMap<BinHash, Vec<Span<Vec<BinHash>>>>,
    /// The revisions of a field declared by a class itself, keyed by class and field.
    fields: HashMap<(BinHash, BinHash), Vec<Span<FieldType>>>,
}

impl Schema {
    /// The copy of the database the League Toolkit programs share on this machine, when there
    /// is one.
    pub fn shared_copy() -> Option<Utf8PathBuf> {
        let file = meta_cache_dir()?.join(SCHEMA_FILE);
        file.is_file().then_some(file)
    }

    /// Reads the database in a file.
    pub fn open(path: &Utf8Path) -> Result<Self, SchemaError> {
        let _span = tracing::info_span!("schema", %path).entered();
        Self::parse(&fs::read(path)?)
    }

    /// Reads the database a caller chose and raises what that finds: `schema.absent` when none
    /// was chosen, `schema.unreadable` when the one chosen does not read.
    pub fn open_reported(path: Option<&Utf8Path>, sink: &mut Sink<'_>) -> Option<Self> {
        let Some(path) = path else {
            sink.raise(SchemaAbsent);
            return None;
        };
        match Self::open(path) {
            Ok(schema) => Some(schema),
            Err(error) => {
                sink.raise(SchemaUnreadable {
                    reason: super::error_chain(&error),
                });
                None
            }
        }
    }

    /// Reads a database from its JSON.
    pub fn parse(json: &[u8]) -> Result<Self, SchemaError> {
        let raw: RawDb = serde_json::from_slice(json)?;
        if raw.format_version != FORMAT {
            return Err(SchemaError::Format {
                found: raw.format_version,
            });
        }
        let mut bases = HashMap::new();
        let mut fields = HashMap::new();
        for (class, raw_class) in raw.classes {
            let class = hash(&class)?;
            let mut revisions = Vec::with_capacity(raw_class.revisions.len());
            for revision in raw_class.revisions {
                revisions.push(Span {
                    from: revision.from,
                    to: revision.to,
                    value: revision
                        .bases
                        .iter()
                        .map(|base| hash(base))
                        .collect::<Result<Vec<_>, _>>()?,
                });
            }
            bases.insert(class, revisions);
            for (field, property) in raw_class.properties {
                let mut revisions = Vec::with_capacity(property.revisions.len());
                for revision in property.revisions {
                    revisions.push(Span {
                        from: revision.from,
                        to: revision.to,
                        value: FieldType::parse(&revision.slots)?,
                    });
                }
                fields.insert((class, hash(&field)?), revisions);
            }
        }
        let earliest = bases
            .values()
            .flatten()
            .map(|revision| revision.from)
            .min()
            .unwrap_or(raw.latest);
        tracing::debug!(
            classes = bases.len(),
            fields = fields.len(),
            latest = raw.latest,
            "schema read"
        );
        Ok(Self {
            latest: raw.latest,
            earliest,
            generation: raw.hash_source.fetched_at,
            bases,
            fields,
        })
    }

    /// The newest build the schema describes.
    pub fn latest(&self) -> u64 {
        self.latest
    }

    /// The oldest build the schema describes. Nothing is known of the types before it.
    pub fn earliest(&self) -> u64 {
        self.earliest
    }

    /// When the database's hash tables were fetched, as it records it.
    pub fn generation(&self) -> Option<&str> {
        self.generation.as_deref()
    }

    /// The build the schema is read at for `build`: the build itself, or the newest the schema
    /// describes when `build` is past it.
    pub fn at(&self, build: u64) -> u64 {
        build.min(self.latest)
    }

    /// The build the schema is read at for an install: the install's, or the newest the schema
    /// describes when the install is past it or does not say its build.
    pub fn at_install(&self, install: Option<&Build>) -> u64 {
        install.map_or(self.latest, |build| self.at(build.number()))
    }

    fn lineage(&self, class: BinHash, lo: u64, hi: u64) -> Lineage<'_> {
        Lineage {
            schema: self,
            lo,
            hi,
            stack: vec![class],
            seen: Vec::new(),
        }
    }

    /// A field's revisions that hold at some build from `lo` to `hi`, as the class itself
    /// declares it.
    fn declared(
        &self,
        class: BinHash,
        field: BinHash,
        lo: u64,
        hi: u64,
    ) -> impl Iterator<Item = FieldType> {
        let revisions = self.fields.get(&(class, field)).into_iter().flatten();
        revisions
            .filter(move |span| span.overlaps(lo, hi))
            .map(|span| span.value)
    }

    /// The type `build` reads for a field of a class, declared by the class or by a base of it.
    pub fn field_type(&self, class: BinHash, field: BinHash, build: u64) -> Option<FieldType> {
        let at = self.at(build);
        self.lineage(class, at, at)
            .find_map(|class| self.declared(class, field, at, at).next())
    }

    /// Every type a field of a class had from build `lo` to build `hi`, through the bases the
    /// class had in those builds. `None` when the range starts before what the schema describes.
    pub fn field_types_between(
        &self,
        class: BinHash,
        field: BinHash,
        lo: u64,
        hi: u64,
    ) -> Option<Vec<FieldType>> {
        if lo < self.earliest {
            return None;
        }
        let (lo, hi) = (self.at(lo), self.at(hi));
        let mut types = Vec::new();
        for class in self.lineage(class, lo, hi) {
            for field_type in self.declared(class, field, lo, hi) {
                if !types.contains(&field_type) {
                    types.push(field_type);
                }
            }
        }
        Some(types)
    }

    /// Whether the class exists at `build`.
    pub fn has_class_at(&self, class: BinHash, build: u64) -> bool {
        let at = self.at(build);
        let mut revisions = self.bases.get(&class).into_iter().flatten();
        revisions.any(|span| span.overlaps(at, at))
    }

    /// Whether `class` is `base` or derives from it at `build`.
    pub fn derives_from(&self, class: BinHash, base: BinHash, build: u64) -> bool {
        let at = self.at(build);
        self.lineage(class, at, at).any(|class| class == base)
    }

    /// Whether the schema has the class at any build.
    pub fn knows_class(&self, class: BinHash) -> bool {
        self.bases.contains_key(&class)
    }

    /// Raises `schema.install-newer` when the install is past what the schema describes.
    pub fn report(&self, install: Option<&Build>, sink: &mut Sink<'_>) {
        if let Some(build) = install.filter(|build| build.number() > self.latest) {
            sink.raise(SchemaInstallNewer {
                install: build.clone(),
                latest: self.latest,
            });
        }
    }
}

impl std::fmt::Debug for Schema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Schema")
            .field("earliest", &self.earliest)
            .field("latest", &self.latest)
            .field("generation", &self.generation)
            .field("classes", &self.bases.len())
            .finish()
    }
}

/// No meta schema database was given. Types come from the install's bins.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaAbsent;

impl Diag for SchemaAbsent {
    const CODE: Code = Code::new("schema.absent", Channel::Run, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Input(Input::Schema)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("no meta schema database; types come from the install's bins");
    }
}

/// The meta schema database given does not read. Types come from the install's bins.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaUnreadable {
    /// What failed.
    pub reason: String,
}

impl Diag for SchemaUnreadable {
    const CODE: Code = Code::new("schema.unreadable", Channel::Run, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Input(Input::Schema)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the meta schema database does not read: ")
            .spelled(&self.reason)
            .text("; types come from the install's bins");
    }
}

/// The install is past the newest build the schema describes. Types changed since are known
/// from the install's bins only.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaInstallNewer {
    /// The install's build.
    pub install: Build,
    /// The newest build the schema describes, by its number.
    pub latest: u64,
}

impl Diag for SchemaInstallNewer {
    const CODE: Code = Code::new("schema.install-newer", Channel::Run, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Input(Input::Schema)
    }

    fn message(&self, f: &mut Message<'_>) {
        use std::fmt::Write as _;
        let _ = write!(
            f,
            "the install is build {}, and the schema describes up to {}; types changed since are \
             known from the install's bins only",
            self.install.number(),
            self.latest
        );
    }
}
