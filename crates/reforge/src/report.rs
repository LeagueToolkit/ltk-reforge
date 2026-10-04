//! The report: everything a run found, serialised. It is built from the stages' results and
//! the log, part by part, so a run that stops early reports what it has.
//!
//! The report holds rows of its own and no reference into the run: it outlives the mod. Ids
//! join its tables, in creation order. Nothing in it is a clock or an address. The install's
//! place is the one text that differs between machines.
//!
//! `docs/design/DIAGNOSTICS.md`, "Serialised".

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;
use serde::Serialize;

use crate::diag::{Diagnostic, Diagnostics, Render};
use crate::env::Env;
use crate::fixups::{After, Change, Changes, Counts, ObjectSet, Snapshot};
use crate::game::{Build, Game};
use crate::load::dating::{Bound, Piece};
use crate::load::inventory::Inventory;
use crate::load::provenance::FileFact;
use crate::load::{Dating, FileProvenance};
use crate::model::entity::{
    BinId, CharId, CharKind, FileId, FileKind, FileSource, FileUse, ModMeta, ObjectId, UnitId,
    WadId, WadLayout, WadRole,
};
use crate::model::hash::hex;
use crate::model::{
    ChunkPath, ContentId, FixupId, Mod, Slot, Spell, WadName, bin, clean_json, value, walk,
};
use crate::package::{Origin, Package};

/// The schema a report states. A field is added under the same schema; a field renamed or
/// retyped takes the next number.
pub const SCHEMA: &str = "reforge.report/1";

/// The history a run was judged against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryInput {
    /// The commit of the build the history took in last.
    pub head: String,
    /// The highest version among its builds.
    pub newest_build: Build,
    /// How many builds it holds.
    pub builds: usize,
}

/// What a run was judged against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inputs {
    /// Where the game was read from. A path of the machine for an install.
    pub install: String,
    /// The install's build, when it says.
    pub build: Option<Build>,
    /// The history. None when the run was judged against the install alone.
    pub history: Option<HistoryInput>,
}

impl Inputs {
    /// The inputs an environment holds.
    pub fn of(env: &Env) -> Self {
        Self {
            install: env.game().describe(),
            build: env.game().build().cloned(),
            history: env.history().stamp().map(|stamp| HistoryInput {
                head: stamp.head.clone(),
                newest_build: stamp.newest_build.clone(),
                builds: stamp.builds,
            }),
        }
    }
}

/// A WAD of the mod.
#[derive(Debug, Clone, Serialize)]
pub struct WadRow {
    /// Its id.
    pub id: WadId,
    /// Its name as the archive spells it.
    pub name: WadName,
    /// Whose it is.
    #[serde(flatten)]
    pub role: WadRole,
    /// How the archive holds it.
    pub layout: WadLayout,
    /// How many files it holds, the ones a fixup made and the bins taken from the game
    /// included.
    pub files: usize,
    /// How many of them have a path the archive or the names spell.
    pub named: usize,
    /// The locale of a locale WAD taken as its champion's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packed_locale: Option<String>,
    /// Whether it restores a champion's model from before its rework.
    pub restoration: bool,
}

/// A file of the mod.
#[derive(Debug, Clone, Serialize)]
pub struct FileRow {
    /// Its id.
    pub id: FileId,
    /// The WAD it is in.
    pub wad: WadId,
    /// The hash the client finds it by, as sixteen hex digits.
    pub hash: String,
    /// Its path, where the archive or the names spell it.
    pub path: Option<ChunkPath>,
    /// What it is, by its first bytes.
    pub kind: FileKind,
    /// Its size in bytes.
    pub size: u64,
    /// The SHA-256 of its bytes.
    pub content: ContentId,
    /// How the client comes to use it.
    #[serde(rename = "use")]
    pub used: FileUse,
    /// Where it comes from.
    #[serde(flatten)]
    pub source: FileSource,
}

/// A character.
#[derive(Debug, Clone, Serialize)]
pub struct CharacterRow {
    /// Its id.
    pub id: CharId,
    /// Its directory name, lowercase.
    pub name: String,
    /// The spelling entry names use.
    pub spelled: String,
    /// A champion or a sub-character.
    #[serde(flatten)]
    pub kind: CharKind,
    /// The WAD its bins are in.
    pub wad: WadId,
    /// Its units, by slot.
    pub units: Vec<UnitId>,
    /// Its root bin, when the mod ships one.
    pub root_bin: Option<FileId>,
    /// Whether every skin bin of it is the game's byte for byte.
    pub equal_to_game: bool,
}

/// A skin unit.
#[derive(Debug, Clone, Serialize)]
pub struct UnitRow {
    /// Its id.
    pub id: UnitId,
    /// Whose skin it is.
    pub character: CharId,
    /// The character's name.
    pub character_name: String,
    /// The slot its bin sits in.
    pub slot: Slot,
    /// The bin's file.
    pub file: FileId,
    /// The bin, when it reads.
    pub bin: Option<BinId>,
    /// The files of the bins the client loads for it, the skin bin first.
    pub loads: Vec<FileId>,
    /// Its skin entry.
    pub entry: Option<ObjectId>,
    /// The slot the entry's name spells.
    pub entry_name_slot: Option<Slot>,
}

/// The piece that sets a mod's dating, with the hash a reader finds it by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceRow {
    /// The piece.
    #[serde(flatten)]
    pub piece: Piece,
    /// A file's path hash as sixteen hex digits, an object's entry hash as eight.
    pub hash: String,
}

/// The dating of the mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatingRow {
    /// The build the mod was made at or after.
    pub not_before: Option<Bound>,
    /// The first piece that first shipped at the bound.
    pub evidence: Option<EvidenceRow>,
    /// How many pieces the history dates.
    pub dated: usize,
    /// How many of them first shipped at the bound.
    pub at_bound: usize,
}

/// What is known of one file's bytes.
#[derive(Debug, Clone, Serialize)]
pub struct ProvenanceRow {
    /// The file.
    pub file: FileId,
    /// The fact.
    #[serde(flatten)]
    pub fact: FileFact,
}

/// What a fixup left where a value was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "as", rename_all = "kebab-case")]
pub enum AfterRow {
    /// The value in the form the target build reads, at the same place.
    Value(Snapshot),
    /// The same value, under another field or in another struct.
    Moved {
        /// Where the value is, its fields spelled.
        to: String,
    },
    /// The value is removed, and nothing is lost by it.
    Removed,
    /// The value is removed, and the mod's own build read it.
    Lost,
    /// The value as it was: it has no conversion to the kind the target build reads.
    Unconvertible {
        /// The kind the target build reads there.
        wants: &'static str,
    },
}

/// What a fixup did at one place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeRow {
    /// The object the value is in.
    pub object: ObjectId,
    /// The object's entry hash, as eight hex digits.
    pub entry: String,
    /// Whether a unit's skin entry reaches the object: what a port can carry (#ADR-0002).
    pub reached: bool,
    /// Where the value was, its fields spelled.
    pub path: String,
    /// The value before. None for a value the fixup added.
    pub before: Option<Snapshot>,
    /// What is there after.
    pub after: AfterRow,
}

impl ChangeRow {
    fn of(change: &Change, m: &Mod, reached: &ObjectSet, names: &dyn Spell) -> Self {
        let after = match &change.after {
            After::Value(snapshot) => AfterRow::Value(snapshot.clone()),
            After::Moved { to } => AfterRow::Moved {
                to: to.spelled(names),
            },
            After::Removed => AfterRow::Removed,
            After::Lost => AfterRow::Lost,
            After::Unconvertible { wants } => AfterRow::Unconvertible { wants },
        };
        Self {
            object: change.object,
            entry: format!("{:08x}", m.objects[change.object].entry.0),
            reached: reached.contains(change.object),
            path: change.path.spelled(names),
            before: change.before.clone(),
            after,
        }
    }
}

/// What one fixup did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixupRow {
    /// The fixup.
    pub fixup: FixupId,
    /// How many of its changes are of each outcome.
    #[serde(flatten)]
    pub counts: Counts,
    /// Its changes, object by object, in the order made.
    pub changes: Vec<ChangeRow>,
}

/// A chunk of the package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChunkRow {
    /// The layer it is in.
    pub layer: String,
    /// The WAD it goes in.
    pub wad: WadName,
    /// Its path in the WAD.
    pub path: ChunkPath,
    /// The hash the client finds it by, as sixteen hex digits.
    pub hash: String,
    /// The file of the mod it was built from.
    pub file: FileId,
    /// Where its bytes come from: `kept` for the file's own, `delta` or `whole` for a bin
    /// written again, `contained` for a DDS put in TEX's container.
    pub origin: &'static str,
    /// Its size in bytes.
    pub size: u64,
    /// The SHA-256 of its bytes as written.
    pub content: ContentId,
    /// For a bin that reads: the SHA-256 of each object as `ltk_meta` writes it, by the
    /// object's entry hash as eight hex digits, in file order. Two programs that write the
    /// same values give the same digests, whatever bytes each copied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objects: Option<IndexMap<String, String>>,
    /// For a bin written again: the entry hashes of the objects a fixup changed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub edited: Vec<String>,
    /// For each of those objects: its digest with the fields of every struct in the order of
    /// their hashes. The client finds a field by its hash, so two repairs that leave the same
    /// values in another order of fields differ in `objects` and are equal here.
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub values: IndexMap<String, String>,
}

/// The package a run built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackageRow {
    /// The project's name.
    pub name: String,
    /// The project's version.
    pub version: String,
    /// Its chunks, layer by layer, by WAD and path.
    pub chunks: Vec<ChunkRow>,
    /// The chunk paths its hash table names.
    pub paths: Vec<String>,
    /// The entry names its hash table names.
    pub entries: Vec<String>,
}

/// How many values a fixup changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixupCount {
    /// The fixup.
    pub fixup: FixupId,
    /// How many values differ by it.
    pub values: usize,
}

/// A chunk that two packages of one mod do not hold alike.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChunkPair {
    /// The WAD.
    pub wad: WadName,
    /// The hash the client finds the chunk by, as sixteen hex digits.
    pub hash: String,
    /// The SHA-256 of its bytes in the first package. None where that package lacks it.
    pub first: Option<ContentId>,
    /// The SHA-256 of its bytes in the second package. None where that package lacks it.
    pub second: Option<ContentId>,
}

/// What repairing a repaired mod again came to (#ADR-0011): the package taken through the run
/// a second time. A repair that is whole changes nothing the second time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgainRow {
    /// Whether the second run built a package.
    pub built: bool,
    /// The build the repaired mod dates at by itself, when the caller gave it. The second
    /// run is judged with what the first established of the mod, its dating among it: an
    /// object a fixup brought to the target build is the game's own of that build, and a
    /// repaired mod dates by it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redated: Option<Bound>,
    /// The values the fixups changed the second time, by fixup. Empty when none did.
    pub fixups: Vec<FixupCount>,
    /// How many chunks the two packages do not hold alike.
    pub chunks_differing: usize,
    /// The first of those chunks, up to [`AgainRow::SHOWN`].
    pub chunks: Vec<ChunkPair>,
}

impl AgainRow {
    /// How many differing chunks a row lists.
    pub const SHOWN: usize = 16;

    /// The second run of a repair against the first: the package the first built, the one the
    /// second built from it, and what the fixups changed the second time.
    pub fn of(first: &Package, second: Option<&Package>, changes: Option<&Changes>) -> Self {
        let keyed = |package: &Package| -> BTreeMap<(WadName, String), ContentId> {
            let chunks = package.chunks().map(|(_, key, chunk)| {
                let hash = format!("{:016x}", key.hash().0);
                ((key.wad.clone(), hash), ContentId::of(&chunk.bytes))
            });
            chunks.collect()
        };
        let before = keyed(first);
        let after = second.map(keyed).unwrap_or_default();
        let keys: BTreeSet<_> = before.keys().chain(after.keys()).collect();
        let differing = keys
            .into_iter()
            .filter(|key| before.get(key) != after.get(key));
        let differing: Vec<ChunkPair> = differing
            .map(|key| ChunkPair {
                wad: key.0.clone(),
                hash: key.1.clone(),
                first: before.get(key).copied(),
                second: after.get(key).copied(),
            })
            .collect();
        let fixups = changes.into_iter().flat_map(Changes::by_fixup);
        let fixups = fixups.map(|(fixup, made)| FixupCount {
            fixup: fixup.clone(),
            values: Counts::of(made).edits(),
        });
        Self {
            built: second.is_some(),
            redated: None,
            fixups: fixups.filter(|row| row.values > 0).collect(),
            chunks_differing: differing.len(),
            chunks: differing.into_iter().take(Self::SHOWN).collect(),
        }
    }

    /// The build the repaired mod dates at by itself.
    #[must_use]
    pub fn redated(mut self, dating: &Dating) -> Self {
        self.redated = dating.not_before.clone();
        self
    }

    /// Whether the second run changed nothing: no value, and no chunk.
    pub fn same(&self) -> bool {
        self.built && self.fixups.is_empty() && self.chunks_differing == 0
    }
}

/// What a diagnostic is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubjectRow {
    /// The kind of thing.
    pub kind: &'static str,
    /// The id the report's tables are joined on, or the name for what has no id.
    pub id: String,
    /// The name for a person.
    pub name: String,
}

/// One diagnostic.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticRow {
    /// Its place in the run's log.
    pub id: u32,
    /// Its code.
    pub code: &'static str,
    /// What the output loses.
    pub severity: &'static str,
    /// What it speaks to.
    pub channel: &'static str,
    /// The stage that raised it.
    pub stage: &'static str,
    /// What it is about.
    pub subject: SubjectRow,
    /// The fixup or check that raised it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
    /// The record it rests on, by its code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// One line for a person.
    pub message: String,
    /// The finding's fields.
    pub data: serde_json::Value,
}

impl DiagnosticRow {
    /// A diagnostic as a report holds it, its message and its subject's name written with `cx`.
    pub fn of(diagnostic: &Diagnostic, cx: &Render<'_>) -> Self {
        Self {
            id: diagnostic.id.0,
            code: diagnostic.code.name,
            severity: diagnostic.severity().as_str(),
            channel: diagnostic.channel().as_str(),
            stage: diagnostic.stage.as_str(),
            subject: SubjectRow {
                kind: diagnostic.subject.kind(),
                id: diagnostic.subject.id(),
                name: cx.subject_name(&diagnostic.subject),
            },
            by: diagnostic.by.as_deref().map(str::to_string),
            record: diagnostic.record.map(|record| record.to_string()),
            message: diagnostic.message(cx),
            data: diagnostic.data(),
        }
    }
}

/// Everything a run found.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// The report's schema.
    pub schema: &'static str,
    /// What the archive says of itself. None when the archive did not read.
    #[serde(rename = "mod")]
    pub meta: Option<ModMeta>,
    /// What the run was judged against. None when no environment was given.
    pub inputs: Option<Inputs>,
    /// The id of the diagnostic that ended the run, when one did.
    pub halted: Option<u32>,
    /// The mod's WADs.
    pub wads: Vec<WadRow>,
    /// The mod's files.
    pub files: Vec<FileRow>,
    /// The characters.
    pub characters: Vec<CharacterRow>,
    /// The skin units.
    pub units: Vec<UnitRow>,
    /// The dating, when it was found.
    pub dating: Option<DatingRow>,
    /// What the archive carries, when it was taken.
    pub inventory: Option<Inventory>,
    /// Whose bytes each used file holds, in file order, when it was found.
    pub provenance: Option<Vec<ProvenanceRow>>,
    /// What each fixup changed, in the order the fixups ran, when they ran. A fixup that
    /// changed nothing has no row.
    pub changes: Option<Vec<FixupRow>>,
    /// The package, when one was built.
    pub package: Option<PackageRow>,
    /// What repairing the repaired mod again came to, when a caller asked for it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub again: Option<AgainRow>,
    /// Every diagnostic, in the order raised.
    pub diagnostics: Vec<DiagnosticRow>,
}

impl Report {
    /// A builder with no part given.
    pub fn builder() -> ReportBuilder {
        ReportBuilder {
            report: Report {
                schema: SCHEMA,
                meta: None,
                inputs: None,
                halted: None,
                wads: Vec::new(),
                files: Vec::new(),
                characters: Vec::new(),
                units: Vec::new(),
                dating: None,
                inventory: None,
                provenance: None,
                changes: None,
                package: None,
                again: None,
                diagnostics: Vec::new(),
            },
        }
    }

    /// The report as JSON for a file or a terminal: every character a terminal acts on is
    /// written as a JSON escape, so a parser reads back the same text.
    ///
    /// # Panics
    ///
    /// When serde fails to write a report into memory, which no value of one makes it do.
    pub fn to_json(&self) -> String {
        let json = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| panic!("a report of schema {SCHEMA} does not write: {error}"));
        clean_json(&json).into_owned()
    }
}

/// Builds a [`Report`] from the parts a run has.
#[derive(Debug, Clone)]
pub struct ReportBuilder {
    report: Report,
}

impl ReportBuilder {
    /// What the run was judged against.
    #[must_use]
    pub fn env(mut self, env: &Env) -> Self {
        self.report.inputs = Some(Inputs::of(env));
        self
    }

    /// The mod as `load` left it: its metadata, WADs, files, characters and units.
    #[must_use]
    pub fn module(mut self, m: &Mod) -> Self {
        self.report.meta = Some(m.meta.clone());
        self.report.wads = m.wads.iter().map(|(id, wad)| wad_row(m, id, wad)).collect();
        self.report.files = m
            .files
            .iter()
            .map(|(id, file)| FileRow {
                id,
                wad: file.wad,
                hash: format!("{:016x}", file.hash.0),
                path: file.path.clone(),
                kind: file.kind,
                size: file.size,
                content: file.content,
                used: file.used,
                source: file.source,
            })
            .collect();
        self.report.characters = m
            .characters
            .iter()
            .map(|(id, character)| CharacterRow {
                id,
                name: character.name.clone(),
                spelled: character.spelled.clone(),
                kind: character.kind.clone(),
                wad: character.wad,
                units: character.units.clone(),
                root_bin: character.root_bin,
                equal_to_game: m.equal_to_game.contains(&id),
            })
            .collect();
        self.report.units = m
            .units
            .iter()
            .map(|(id, unit)| UnitRow {
                id,
                character: unit.character,
                character_name: m.characters[unit.character].name.clone(),
                slot: unit.source_slot,
                file: unit.file,
                bin: unit.bin,
                loads: unit.loads.iter().map(|&bin| m.bins[bin].file).collect(),
                entry: unit.entry,
                entry_name_slot: unit.entry_name_slot,
            })
            .collect();
        self
    }

    /// The dating of the mod. The mod gives the hash of the piece that sets it.
    #[must_use]
    pub fn dating(mut self, dating: &Dating, m: &Mod) -> Self {
        let evidence = dating.evidence.map(|piece| EvidenceRow {
            piece,
            hash: match piece {
                Piece::File(file) => format!("{:016x}", m.files[file].hash.0),
                Piece::Object(object) => format!("{:08x}", m.objects[object].entry.0),
            },
        });
        self.report.dating = Some(DatingRow {
            not_before: dating.not_before.clone(),
            evidence,
            dated: dating.dated,
            at_bound: dating.at_bound,
        });
        self
    }

    /// What the archive carries.
    #[must_use]
    pub fn inventory(mut self, inventory: &Inventory) -> Self {
        self.report.inventory = Some(inventory.clone());
        self
    }

    /// Whose bytes each used file holds.
    #[must_use]
    pub fn provenance(mut self, provenance: &FileProvenance) -> Self {
        let rows = provenance.iter().map(|(file, fact)| ProvenanceRow {
            file,
            fact: fact.clone(),
        });
        self.report.provenance = Some(rows.collect());
        self
    }

    /// What the fixups changed in the mod they were applied to, every path spelled with
    /// `names`. Every change is listed: a run that stops before the plan has no list of what
    /// ships, so each says whether a unit's skin entry reaches its object (#ADR-0002).
    #[must_use]
    pub fn changes(mut self, changes: &Changes, m: &Mod, names: &dyn Spell) -> Self {
        let units: Vec<UnitId> = m.units.ids().collect();
        let reached = ObjectSet::reached(m, &units);
        let rows = changes.by_fixup().map(|(fixup, made)| FixupRow {
            fixup: fixup.clone(),
            counts: Counts::of(made),
            changes: made
                .iter()
                .map(|change| ChangeRow::of(change, m, &reached, names))
                .collect(),
        });
        self.report.changes = Some(rows.collect());
        self
    }

    /// The package a run built: every chunk with the digest of its bytes, and for a bin the
    /// digest of each object it holds.
    #[must_use]
    pub fn package(mut self, package: &Package) -> Self {
        let chunks = package.chunks().map(|(layer, key, chunk)| {
            let is_bin = key.path.extension() == Some("bin");
            let parsed = is_bin.then(|| bin::parse(&chunk.bytes).ok()).flatten();
            let objects = parsed.map(|parsed| {
                let digests = parsed.objects.iter().map(|object| {
                    let entry = format!("{:08x}", object.value.path_hash.0);
                    (entry, hex(&value::digest(&object.value)))
                });
                digests.collect()
            });
            let edited = match &chunk.origin {
                Origin::Rewritten { written, .. } => written.edited.as_slice(),
                Origin::Kept(_) | Origin::Contained(_) => &[],
            };
            let values = edited.iter().map(|object| {
                let mut sorted = object.clone();
                walk::structs_mut(&mut sorted, |mut s| s.fields_mut().sort_keys());
                let entry = format!("{:08x}", object.path_hash.0);
                (entry, hex(&value::digest(&sorted)))
            });
            let values = values.collect();
            let edited = edited
                .iter()
                .map(|object| format!("{:08x}", object.path_hash.0));
            ChunkRow {
                layer: layer.name(),
                wad: key.wad.clone(),
                path: key.path.clone(),
                hash: format!("{:016x}", key.hash().0),
                file: chunk.origin.file(),
                origin: chunk.origin.name(),
                size: chunk.bytes.len() as u64,
                content: ContentId::of(&chunk.bytes),
                objects,
                edited: edited.collect(),
                values,
            }
        });
        self.report.package = Some(PackageRow {
            name: package.config.name.clone(),
            version: package.config.version.clone(),
            chunks: chunks.collect(),
            paths: package.tables.paths().map(str::to_string).collect(),
            entries: package.tables.entries().map(str::to_string).collect(),
        });
        self
    }

    /// What repairing the repaired mod again came to.
    #[must_use]
    pub fn again(mut self, again: AgainRow) -> Self {
        self.report.again = Some(again);
        self
    }

    /// Every diagnostic of the log, each written with `cx`, and the one that ended the run.
    #[must_use]
    pub fn diagnostics(mut self, log: &Diagnostics, cx: &Render<'_>) -> Self {
        self.report.diagnostics = log.iter().map(|d| DiagnosticRow::of(d, cx)).collect();
        self.report.halted = log.halted().map(|halted| halted.0.0);
        self
    }

    /// The report.
    pub fn build(self) -> Report {
        self.report
    }
}

fn wad_row(m: &Mod, id: WadId, wad: &crate::model::entity::ModWad) -> WadRow {
    let named = wad
        .files
        .iter()
        .filter(|&&file| m.files[file].path.is_some());
    WadRow {
        id,
        name: wad.name.clone(),
        role: wad.role.clone(),
        layout: wad.layout,
        files: wad.files.len(),
        named: named.count(),
        packed_locale: wad.packed_locale.clone(),
        restoration: m.restorations.contains(&id),
    }
}
