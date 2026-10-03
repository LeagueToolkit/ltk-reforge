//! The report: everything a run found, serialised. It is built from the stages' results and
//! the log, part by part, so a run that stops early reports what it has.
//!
//! The report holds rows of its own and no reference into the run: it outlives the mod. Ids
//! join its tables, in creation order. Nothing in it is a clock or an address. The install's
//! place is the one text that differs between machines.
//!
//! `docs/design/DIAGNOSTICS.md`, "Serialised".

use serde::Serialize;

use crate::diag::{Diagnostic, Diagnostics, Render};
use crate::env::Env;
use crate::game::{Build, Game};
use crate::load::dating::{Bound, Piece};
use crate::load::inventory::Inventory;
use crate::load::provenance::FileFact;
use crate::load::{Dating, FileProvenance};
use crate::model::entity::{
    BinId, CharId, CharKind, FileId, FileKind, FileSource, FileUse, ModMeta, ObjectId, UnitId,
    WadId, WadLayout, WadRole,
};
use crate::model::{ChunkPath, ContentId, Mod, Slot, WadName, clean_json};

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
