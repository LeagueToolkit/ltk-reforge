//! What the build, the checks and the write raise: one type per code of
//! `docs/design/CODES.md`, "package, check, write".

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ltk_meta::PropertyKind as Kind;
use serde::{Serialize, Serializer};

use super::texture::TexRefusal;
use super::wired::LeftBehind;
use super::write::WriterKind;
use crate::diag::{Channel, Code, Diag, DiagId, Message, Record, Severity, Subject};
use crate::game::FieldType;
use crate::model::value::kind_name;
use crate::model::{BinHash, ChunkPath, WadName};

const fn code(name: &'static str, severity: Severity) -> Code {
    Code::new(name, Channel::Output, severity)
}

fn hex<S: Serializer>(hash: &BinHash, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(&format_args!("{:08x}", hash.0))
}

fn hex_or_none<S: Serializer>(hash: &Option<BinHash>, s: S) -> Result<S::Ok, S::Error> {
    match hash {
        Some(hash) => s.collect_str(&format_args!("{:08x}", hash.0)),
        None => s.serialize_none(),
    }
}

/// DDS textures at `.tex` paths: the ones put in TEX's container with their blocks as they
/// are, and the ones kept as DDS, by why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextureContained {
    /// How many textures went into TEX's container.
    pub contained: usize,
    /// How many stay DDS, by why TEX's container does not hold them as they are.
    pub kept: BTreeMap<TexRefusal, usize>,
}

impl Diag for TextureContained {
    const CODE: Code = code("package.texture-contained", Severity::Note);
    const RECORD: Option<Record> = Some(Record::game_change(7));

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.contained, "DDS texture")
            .text(" at .tex paths put in TEX's container, their blocks as they are");
        if !self.kept.is_empty() {
            f.text("; kept as DDS, which the client reads too: ");
            let kept: Vec<(&TexRefusal, &usize)> = self.kept.iter().collect();
            f.list(&kept, |f, (why, n)| {
                let _ = write!(f, "{n} {}", why.as_str());
            });
        }
    }
}

/// A repair's output: the mod's files at their own paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InPlace {
    /// Files shipped at their paths, the bins written again included.
    pub kept: usize,
    /// Bins written again: an object of each was changed.
    pub rewritten: usize,
    /// Of those, the bins written whole, their numbering too old for the delta writer.
    pub whole: usize,
    /// Files shipped that a file fixup made.
    pub made: usize,
    /// Files left out, which the client does not reach (#ADR-0039), by the reason.
    #[serde(flatten)]
    pub left: LeftBehind,
    /// Files shipped under their hash: no table spells their path.
    pub unnamed: usize,
}

impl Diag for InPlace {
    const CODE: Code = code("package.in-place", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.kept, "file")
            .text(" kept at their paths, ")
            .count(self.rewritten, "bin")
            .text(" written again");
        if self.whole > 0 {
            let _ = write!(
                f,
                " ({} whole, their numbering too old for the delta writer)",
                self.whole
            );
        }
        if self.made > 0 {
            let _ = write!(f, ", {} made by a file fixup", self.made);
        }
        if self.left.total() > 0 {
            f.text("; ")
                .count(self.left.total(), "file")
                .text(" left out: ");
            f.list(&self.left.reasons(), |f, (n, reason)| {
                let _ = write!(f, "{n} {reason}");
            });
        }
        if self.unnamed > 0 {
            f.text("; ")
                .count(self.unnamed, "file")
                .text(" no table names, kept under their hash");
        }
    }
}

/// The package holds nothing: there is nothing to write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Empty {
    /// How many files the mod holds, none of which the client uses.
    pub files: usize,
}

impl Diag for Empty {
    const CODE: Code = code("package.empty", Severity::Fatal);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the package holds nothing: the client uses none of the mod's ")
            .count(self.files, "file");
    }
}

/// A type as a finding holds it: the kinds by their names, the class by its hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Shape {
    /// The value's kind.
    pub kind: &'static str,
    /// A map's key kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<&'static str>,
    /// A container's or an option's item kind, a map's value kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<&'static str>,
    /// The class of an embed or a pointer, or of a container's items.
    #[serde(
        serialize_with = "hex_or_none",
        skip_serializing_if = "Option::is_none"
    )]
    pub class: Option<BinHash>,
}

impl Shape {
    /// The shape of a type.
    pub fn of(field_type: &FieldType) -> Self {
        Self {
            kind: kind_name(field_type.kind),
            key: field_type.key.map(kind_name),
            item: field_type.item.map(kind_name),
            class: field_type.class,
        }
    }

    /// Writes `Map<Hash, Embed<Class>>`: the class beside the kind that holds a struct.
    fn message(&self, f: &mut Message<'_>) {
        let strukt = [kind_name(Kind::Embedded), kind_name(Kind::Struct)];
        let item = |f: &mut Message<'_>, kind: &str| {
            f.text(kind);
            if let Some(class) = self.class.filter(|_| strukt.contains(&kind)) {
                f.text("<").class(class).text(">");
            }
        };
        match (self.key, self.item) {
            (Some(key), Some(value)) => {
                f.text(self.kind).text("<").text(key).text(", ");
                item(f, value);
                f.text(">");
            }
            (None, Some(value)) => {
                f.text(self.kind).text("<");
                item(f, value);
                f.text(">");
            }
            _ => item(f, self.kind),
        }
    }
}

/// Where a check found a value: the chunk and the object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Place {
    /// The WAD of the bin.
    pub wad: WadName,
    /// The bin's path.
    pub path: ChunkPath,
    /// The entry hash of the object.
    #[serde(serialize_with = "hex")]
    pub entry: BinHash,
}

/// The values of one field of one class written in one shape, where the build reads another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeRow {
    /// The class of the struct the field is on.
    #[serde(serialize_with = "hex")]
    pub class: BinHash,
    /// The field.
    #[serde(serialize_with = "hex")]
    pub field: BinHash,
    /// The shape written.
    pub found: Shape,
    /// The shape the target build reads.
    pub wants: Shape,
    /// How many values.
    pub values: usize,
    /// The first of them.
    pub first: Place,
}

/// Values written of another type than the target build declares. The client skips them
/// (#GAME-BEHAVIOR-0002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeMismatch {
    /// How many values.
    pub values: usize,
    /// The values by class, field and the two shapes, in the order found.
    pub fields: Vec<TypeRow>,
}

impl Diag for TypeMismatch {
    const CODE: Code = code("check.type-mismatch", Severity::Error);
    const RECORD: Option<Record> = Some(Record::game_behavior(2));

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.values, "value")
            .text(" the client skips, of another type than the target build declares: ");
        f.list(&self.fields, |f, row| {
            f.class(row.class)
                .text(".")
                .field(row.field)
                .text(" holds ");
            row.found.message(f);
            f.text(", the build reads ");
            row.wants.message(f);
            let _ = write!(f, " ({})", row.values);
        });
    }
}

/// The values of one field written on a class that does not declare it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UndeclaredRow {
    /// The class of the struct the field is on.
    #[serde(serialize_with = "hex")]
    pub class: BinHash,
    /// The field.
    #[serde(serialize_with = "hex")]
    pub field: BinHash,
    /// How many values.
    pub values: usize,
    /// The first of them.
    pub first: Place,
}

/// Fields written that the target build's class does not declare. The client does not read
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldUndeclared {
    /// How many values.
    pub values: usize,
    /// The values by class and field, in the order found.
    pub fields: Vec<UndeclaredRow>,
}

impl Diag for FieldUndeclared {
    const CODE: Code = code("check.field-undeclared", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.values, "field")
            .text(" the client does not read, undeclared at the target build: ");
        f.list(&self.fields, |f, row| {
            f.class(row.class).text(".").field(row.field);
            let _ = write!(f, " ({})", row.values);
        });
    }
}

/// How a bin written again differs from what it was built to hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "problem", rename_all = "kebab-case")]
pub enum RoundTripProblem {
    /// The bytes do not read as a bin.
    Unreadable {
        /// Why, as the reader says it.
        reason: String,
    },
    /// The bin read back holds other entries than it was built with.
    Entries {
        /// How many entries the bin was built with.
        built: usize,
        /// How many entries read back.
        read: usize,
    },
    /// An object a fixup changed reads back as another value, or not at all.
    Object {
        /// The object's entry hash.
        #[serde(serialize_with = "hex")]
        entry: BinHash,
    },
}

/// A bin written again that does not read back as it was built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoundTripFailed {
    /// The WAD of the bin.
    pub wad: WadName,
    /// The bin's path.
    pub path: ChunkPath,
    /// Each difference.
    pub problems: Vec<RoundTripProblem>,
}

impl Diag for RoundTripFailed {
    const CODE: Code = code("check.round-trip-failed", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Chunk(self.path.clone())
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the bin written again does not read back as built: ");
        f.list(&self.problems, |f, problem| match problem {
            RoundTripProblem::Unreadable { reason } => {
                f.text("it does not read (").spelled(reason).text(")");
            }
            RoundTripProblem::Entries { built, read } => {
                let _ = write!(f, "it holds {read} entries, built with {built}");
            }
            RoundTripProblem::Object { entry } => {
                f.text("the object ")
                    .entry(*entry)
                    .text(" reads back otherwise");
            }
        });
    }
}

/// What the types of a check were read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum Against {
    /// The meta schema at a build, and the install's bins for what it does not describe.
    Schema {
        /// The build the schema was read at.
        build: u64,
    },
    /// The install's bins alone: no schema database. Nothing is said of undeclared fields.
    Install,
}

/// What a check covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "check", rename_all = "kebab-case")]
pub enum Coverage {
    /// Bins written again, read back.
    RoundTrip {
        /// How many bins were read back.
        bins: usize,
        /// How many changed objects were compared.
        objects: usize,
    },
    /// Values typed against the target build.
    Types {
        /// How many bins were read.
        bins: usize,
        /// How many fields were typed.
        fields: usize,
        /// What the types were read from.
        against: Against,
    },
}

/// What a check covered, whatever it found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Done(pub Coverage);

impl Diag for Done {
    const CODE: Code = code("check.done", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        match self.0 {
            Coverage::RoundTrip { bins, objects } => {
                f.count(bins, "bin")
                    .text(" written again read back, ")
                    .count(objects, "changed object")
                    .text(" compared");
            }
            Coverage::Types {
                bins,
                fields,
                against,
            } => {
                f.count(fields, "field")
                    .text(" of ")
                    .count(bins, "bin")
                    .text(" typed against ");
                match against {
                    Against::Schema { build } => {
                        let _ = write!(f, "the schema at build {build}");
                    }
                    Against::Install => {
                        f.text("the install's bins (no schema database)");
                    }
                }
            }
        }
    }
}

/// A package on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Written {
    /// What was written.
    pub kind: WriterKind,
    /// Its name in the output directory.
    pub name: String,
    /// How many chunks it holds.
    pub chunks: usize,
    /// How many of them are named by their hash: a part of their path is longer than a file
    /// name may be.
    pub hash_named: usize,
    /// The bytes of the chunks.
    pub bytes: u64,
}

impl Diag for Written {
    const CODE: Code = code("write.written", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("wrote the ")
            .text(self.kind.as_str())
            .text(" ")
            .text(&self.name)
            .text(" (")
            .count(self.chunks, "chunk");
        if self.hash_named > 0 {
            let _ = write!(f, ", {} named by their hash", self.hash_named);
        }
        f.text(")");
    }
}

/// The caller's policy refused the write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Blocked {
    /// The diagnostics the policy blocks on, in the order raised.
    pub by: Vec<DiagId>,
}

impl Diag for Blocked {
    const CODE: Code = code("write.blocked", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("nothing is written: the caller's policy blocks on ")
            .count(self.by.len(), "diagnostic")
            .text(" (");
        f.list(&self.by, |f, id| {
            let _ = write!(f, "{id}");
        });
        f.text(")");
    }
}
