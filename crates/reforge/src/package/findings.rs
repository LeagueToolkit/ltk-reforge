//! What the build, the checks and the write raise: one type per code of
//! `docs/design/CODES.md`, "package, check, write".

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ltk_meta::PropertyKind as Kind;
use serde::{Serialize, Serializer};

use super::texture::TexRefusal;
use super::thumbnail::ThumbnailRefusal;
use super::wired::LeftBehind;
use super::write::WriterKind;
use crate::diag::{Channel, Code, Diag, DiagId, Message, Record, Severity, Subject};
use crate::game::FieldType;
use crate::model::entity::UnitId;
use crate::model::value::kind_name;
use crate::model::{BinHash, ChunkPath, WadHash, WadName};

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

fn hexes<S: Serializer>(hashes: &[BinHash], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(hashes.iter().map(|hash| format!("{:08x}", hash.0)))
}

fn path_hexes<S: Serializer>(hashes: &[WadHash], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(hashes.iter().map(|hash| format!("{:016x}", hash.0)))
}

/// A unit that is not written: the game has no bin to patch at its target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotWritten {
    /// The unit.
    pub unit: UnitId,
    /// The bin the unit's patch is for.
    pub target: ChunkPath,
}

impl Diag for NotWritten {
    const CODE: Code = code("unit.not-written", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "the game has no {} that reads as a bin; the unit is not written",
            self.target
        );
    }
}

/// A skin bin written for a sub-character (#ADR-0017) to which records of the unit's patch
/// did not apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewBinFailed {
    /// The unit.
    pub unit: UnitId,
    /// The bin written.
    pub target: ChunkPath,
    /// How many records applied.
    pub applied: usize,
    /// How many did not.
    pub skipped: usize,
}

impl Diag for NewBinFailed {
    const CODE: Code = code("unit.new-bin-failed", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "{} written from the game's skin0: {} records applied, {} did not",
            self.target, self.applied, self.skipped
        );
    }
}

/// A sub-character the game ships no skin bin of at its target: the build writes one whole,
/// from the game's skin0 (#ADR-0017).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewBin {
    /// The unit.
    pub unit: UnitId,
    /// The bin written.
    pub target: ChunkPath,
}

impl Diag for NewBin {
    const CODE: Code = code("unit.new-bin", Severity::Note);
    const RECORD: Option<Record> = Some(Record::game_behavior(1));

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "the game has no {} for the character; the port writes one from its skin0",
            self.target
        );
    }
}

/// What one unit's port holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnitPorted {
    /// The unit.
    pub unit: UnitId,
    /// The game bin patched, or the bin written where the game has none.
    pub target: ChunkPath,
    /// How many records and whole objects the patch holds.
    pub records: usize,
    /// How many objects the unit's own bin carries.
    pub carried: usize,
    /// Whether the graph ships.
    pub graph: bool,
    /// Whether the target is a bin the port wrote whole (#ADR-0017).
    pub new_bin: bool,
}

impl Diag for UnitPorted {
    const CODE: Code = code("unit.ported", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.records, "record");
        let _ = write!(f, " on {}, ", self.target);
        f.count(self.carried, "object").text(" carried");
        if self.graph {
            f.text(", the graph shipped");
        }
        if self.new_bin {
            f.text("; the bin is written whole, from the game's skin0");
        }
    }
}

/// Carried fields the game's entry has and the mod's lacks. The game's stay: a patch cannot
/// remove a field (#ADR-0026).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldKept {
    /// The unit.
    pub unit: UnitId,
    /// The fields.
    #[serde(serialize_with = "hexes")]
    pub fields: Vec<BinHash>,
}

impl Diag for FieldKept {
    const CODE: Code = code("package.field-kept", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the mod's entry lacks ");
        f.list(&self.fields, |f, field| {
            f.field(*field);
        });
        f.text("; the game's stays (a patch cannot remove a field)");
    }
}

/// The game's bin holds no entry under the target's name: the mod's entry goes in whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntryWhole {
    /// The unit.
    pub unit: UnitId,
}

impl Diag for EntryWhole {
    const CODE: Code = code("package.entry-whole", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text(
            "the game's bin holds no entry under the target's name; the mod's entry goes in whole",
        );
    }
}

/// A resolver's map written whole (#ADR-0006): the game's keys at the build of the run, with
/// the mod's laid over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolverFrozen {
    /// The unit.
    pub unit: UnitId,
    /// How many keys the map written holds.
    pub keys: usize,
}

impl Diag for ResolverFrozen {
    const CODE: Code = code("package.resolver-frozen", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "the resolver's map is written whole ({} keys): keys the game adds later are not seen until the port is made again",
            self.keys
        );
    }
}

/// A port's output: what the package holds, and the files of the mod it leaves out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ported {
    /// Files of the mod shipped under the namespace.
    pub files: usize,
    /// Bins the port made.
    pub bins: usize,
    /// Banks the port made of the mod's own clips.
    pub banks: usize,
    /// Patches of game bins.
    pub patches: usize,
    /// Files left out, which the client does not reach through the port (#ADR-0039), by the
    /// reason.
    #[serde(flatten)]
    pub left: LeftBehind,
}

impl Diag for Ported {
    const CODE: Code = code("package.ported", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files, "file")
            .text(" shipped under the namespace, ")
            .count(self.bins, "bin")
            .text(" and ")
            .count(self.banks, "bank")
            .text(" made, ")
            .count(self.patches, "PTCH file")
            .text(" on game bins");
        if self.left.total() > 0 {
            f.text("; ")
                .count(self.left.total(), "file")
                .text(" left out: ");
            f.list(&self.left.reasons(), |f, (n, reason)| {
                let _ = write!(f, "{n} {reason}");
            });
        }
    }
}

/// A layer's manifest that does not load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManifestUnreadable {
    /// The layer.
    pub layer: String,
    /// Why, as the reader says it.
    pub reason: String,
}

impl Diag for ManifestUnreadable {
    const CODE: Code = code("check.manifest-unreadable", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the manifest of the layer ")
            .text(&self.layer)
            .text(" does not load: ")
            .spelled(&self.reason);
    }
}

/// A patch that does not apply to the bin it is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplyFailed {
    /// The layer of the patch.
    pub layer: String,
    /// The bin it is for.
    pub target: ChunkPath,
    /// Why, as the overlay says it. None when the game has no bin there.
    pub reason: Option<String>,
}

impl Diag for ApplyFailed {
    const CODE: Code = code("check.apply-failed", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Chunk(self.target.clone())
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the patch of the layer ").text(&self.layer);
        match &self.reason {
            Some(reason) => f.text(" does not apply: ").spelled(reason),
            None => f.text(" has no game bin to apply to"),
        };
    }
}

/// Edits of a patch that the overlay skips.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordSkipped {
    /// The layer of the patch.
    pub layer: String,
    /// The bin it is for.
    pub target: ChunkPath,
    /// Each edit skipped, as the overlay says it.
    pub records: Vec<String>,
}

impl Diag for RecordSkipped {
    const CODE: Code = code("check.record-skipped", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Chunk(self.target.clone())
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.records.len(), "edit")
            .text(" of the layer ")
            .text(&self.layer)
            .text(" do not apply: ");
        f.list(&self.records, |f, record| {
            f.spelled(record);
        });
    }
}

/// Links a patch adds that resolve to no bin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinkUnresolved {
    /// The bin patched.
    pub target: ChunkPath,
    /// The links.
    pub links: Vec<String>,
}

impl Diag for LinkUnresolved {
    const CODE: Code = code("check.link-unresolved", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Chunk(self.target.clone())
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("links the port adds resolve to nothing: ");
        f.list(&self.links, |f, link| {
            f.spelled(link);
        });
    }
}

/// A patched bin whose load, as the overlay follows it, reached the cap on bins: names in the
/// bins past it are not found by the check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverlayCapped {
    /// The bin patched.
    pub target: ChunkPath,
    /// The cap on the bins the load of one patched bin follows.
    pub cap: usize,
}

impl Diag for OverlayCapped {
    const CODE: Code = code("check.load-capped", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Chunk(self.target.clone())
    }

    fn message(&self, f: &mut Message<'_>) {
        let _ = write!(
            f,
            "its load stops at {} bins; names in the bins past it are not checked",
            self.cap
        );
    }
}

/// Names the patched entry of a unit reaches that resolve to nothing, and did resolve in the
/// mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NameUnresolved {
    /// The unit.
    pub unit: UnitId,
    /// The names, by their hashes.
    #[serde(serialize_with = "hexes")]
    pub names: Vec<BinHash>,
}

impl Diag for NameUnresolved {
    const CODE: Code = code("check.name-unresolved", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.names.len(), "name")
            .text(" the patched entry reaches resolve to nothing: ");
        f.list(&self.names, |f, name| {
            f.entry(*name);
        });
    }
}

/// Files a unit's port names that are neither shipped nor the game's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileMissing {
    /// The unit.
    pub unit: UnitId,
    /// The files, by their path hashes.
    #[serde(serialize_with = "path_hexes")]
    pub files: Vec<WadHash>,
}

impl Diag for FileMissing {
    const CODE: Code = code("check.file-missing", Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file")
            .text(" the port names are neither shipped nor the game's: ");
        f.list(&self.files, |f, file| {
            f.path(*file);
        });
    }
}

/// A value of a skin entry that a check of the base skin reads for another skin's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkinValue {
    /// `championSkinName`.
    SkinName,
    /// A tag of `skinAudioProperties.tagEventList`.
    AudioTag,
    /// The name of a unit of `skinAudioProperties.bankUnits`.
    BankUnit,
    /// `skinMeshProperties.skeleton`.
    Skeleton,
    /// `skinMeshProperties.simpleSkin`.
    SimpleSkin,
    /// `skinMeshProperties.texture`.
    Texture,
}

impl SkinValue {
    /// The value as the entry spells its place.
    pub fn as_str(self) -> &'static str {
        match self {
            SkinValue::SkinName => "championSkinName",
            SkinValue::AudioTag => "tagEventList",
            SkinValue::BankUnit => "bankUnits",
            SkinValue::Skeleton => "skeleton",
            SkinValue::SimpleSkin => "simpleSkin",
            SkinValue::Texture => "texture",
        }
    }
}

/// One value of a patched base skin that names another skin of the champion.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct OtherSkinName {
    /// Which value.
    pub value: SkinValue,
    /// What it holds.
    pub names: String,
}

/// A champion's base skin that, patched, names another skin of the champion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OtherSkin {
    /// The unit.
    pub unit: UnitId,
    /// The values, in the order a check of the base skin reads them.
    pub values: Vec<OtherSkinName>,
}

impl Diag for OtherSkin {
    const CODE: Code = code("check.other-skin", Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Unit(self.unit)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the patched base skin names another skin of the champion: ");
        f.list(&self.values, |f, value| {
            f.text(value.value.as_str()).text(" ").spelled(&value.names);
        });
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

/// A material's pass and the shader it links.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ShaderLink {
    /// The material's entry hash.
    #[serde(serialize_with = "hex")]
    pub material: BinHash,
    /// The shader's entry hash.
    #[serde(serialize_with = "hex")]
    pub shader: BinHash,
}

/// Materials of the package that link a shader the target's shader bin lacks. The client
/// crashes while it loads them (#GAME-BEHAVIOR-0023): there is nothing to write (#ADR-0069).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShaderMissing {
    /// Each material and the shader it links, once, in hash order.
    pub links: Vec<ShaderLink>,
}

impl Diag for ShaderMissing {
    const CODE: Code = code("package.shader-missing", Severity::Fatal);
    const RECORD: Option<Record> = Some(Record::game_behavior(23));

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.links.len(), "material")
            .text(" linking a shader the target's shader bin lacks; the client crashes at load: ");
        f.list(&self.links, |f, link| {
            f.entry(link.material)
                .text(" (")
                .entry(link.shader)
                .text(")");
        });
    }
}

/// The mod's image is not the package's thumbnail (#ADR-0051). The package is whole without
/// one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThumbnailDropped {
    /// Why.
    #[serde(flatten)]
    pub why: ThumbnailRefusal,
}

impl Diag for ThumbnailDropped {
    const CODE: Code = code("package.thumbnail-dropped", Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the mod's image is not carried as the thumbnail: ");
        match self.why {
            ThumbnailRefusal::EntryTooLarge { bytes } => {
                let _ = write!(f, "its entry is {bytes} bytes");
            }
            ThumbnailRefusal::EntryUnreadable => {
                f.text("its entry does not read");
            }
            ThumbnailRefusal::NotAnImage => {
                f.text("it decodes as no PNG, JPEG or WebP");
            }
            ThumbnailRefusal::NotEncoded => {
                f.text("it does not encode as WebP");
            }
            ThumbnailRefusal::TooLarge { bytes } => {
                let _ = write!(f, "it is {bytes} bytes as WebP");
            }
        }
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
        /// How many banks the run made were read back.
        banks: usize,
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
    /// Patches applied to the game's bins as the overlay applies them.
    Apply {
        /// How many patches applied.
        patches: usize,
        /// How many names the patched entries reach.
        reached: usize,
        /// Names unresolved as they were in the mod.
        names_known: usize,
        /// Files unresolved as they were in the mod.
        files_known: usize,
        /// Names the game's own entry and resolver leave unresolved.
        names_game: usize,
        /// Files the game's own entry and resolver leave unresolved.
        files_game: usize,
    },
    /// Champions' patched base skins read for another skin's name.
    BaseSkin {
        /// How many patched base skin entries were read.
        entries: usize,
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
            Coverage::RoundTrip {
                bins,
                objects,
                banks,
            } => {
                f.count(bins, "bin")
                    .text(" written again read back, ")
                    .count(objects, "changed object")
                    .text(" compared");
                if banks > 0 {
                    f.text(", ").count(banks, "bank").text(" made read back");
                }
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
            Coverage::Apply {
                patches,
                reached,
                names_known,
                files_known,
                names_game,
                files_game,
            } => {
                f.count(patches, "game bin")
                    .text(" patched as the overlay patches, ")
                    .count(reached, "name")
                    .text(" reached");
                let _ = write!(
                    f,
                    "; {names_known} names and {files_known} files unresolved as they were in the mod, {names_game} names and {files_game} files the game's own entry and resolver leave unresolved"
                );
            }
            Coverage::BaseSkin { entries } => {
                f.count(entries, "patched base skin")
                    .text(" read for another skin's name");
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
