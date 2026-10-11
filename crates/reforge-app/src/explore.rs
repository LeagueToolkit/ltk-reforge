//! A mod explored before any run (#ADR-0091): a port's run through `load`, over the store's
//! tiers and its raw cache as a run reads them, and the load report rendered. Nothing goes to
//! the store's rows. The app holds the last mod explored, whose files and file viewer the Mod
//! page reads (`docs/design/ROADMAP-UX.md`, "The Mod page").
//!
//! The view is the report's rows read through `clean`, as the views of a run are, and adds no
//! rule about a mod (#ADR-0046).

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use reforge::archive::RawSnapshot;
use reforge::game::{Names, Speller};
use reforge::model::{BinHash, ContentId, Spell, clean};
use reforge::package::Thumbnail;
use reforge::snapshot::LoadSnapshot;
use reforge::store::{BlobStore, MemoryStore};
use reforge::{Porter, RunError};
use reforge_magic::Kind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, chain};
use crate::inputs::Inputs;
use crate::run::{
    Maker, ModView, ReportParts, RunKind, RunSettings, Unwatched, data_url, ingested,
};
use crate::store::{self, AppStore, FileView, RunAgain, StageFiles, Tiers};
use crate::view::{Finding, read_clean, through_clean};

/// What dated the mod: the first piece of it that first shipped at the bound.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct EvidenceView {
    /// `file` or `object`, as the report names the piece.
    pub kind: String,
    /// The file's path, else its hash; the object's entry name, else its hash.
    pub name: String,
    /// A file's path hash as sixteen hex digits, an object's entry hash as eight.
    pub hash: String,
}

/// The build the mod was made at or after, and what dated it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct DatingView {
    /// The build. None when the history dates no piece of the mod.
    pub not_before: Option<String>,
    /// The piece that sets the bound.
    pub evidence: Option<EvidenceView>,
    /// How many pieces the history dates.
    pub dated: usize,
    /// How many of them first shipped at the bound.
    pub at_bound: usize,
}

/// A WAD of the mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct WadView {
    /// Its id in the report: `%w0`.
    pub id: String,
    /// Its name as the archive spells it.
    pub name: String,
    /// Whose it is: `champion`, `locale` or `other`.
    pub role: String,
    /// How the archive holds it: `loose` or `packed`.
    pub layout: String,
    /// How many files it holds.
    pub files: usize,
    /// How many of them have a path the archive or the names spell.
    pub named: usize,
    /// The locale of a locale WAD taken as its champion's.
    pub packed_locale: Option<String>,
    /// Whether it restores a champion's model from before its rework.
    pub restoration: bool,
}

/// A character of the mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CharacterView {
    /// Its id in the report: `%c0`.
    pub id: String,
    /// Its directory name, lowercase.
    pub name: String,
    /// The spelling entry names use.
    pub spelled: String,
    /// `champion` or `sub`.
    pub kind: String,
    /// The name of the WAD its bins are in.
    pub wad: String,
    /// How many units it has.
    pub units: usize,
    /// Whether every skin bin of it is the game's byte for byte.
    pub equal_to_game: bool,
    /// Its root bin's path, else its hash, when the mod ships one.
    pub root_bin: Option<String>,
}

/// A skin unit of the mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct UnitView {
    /// Its id in the report: `%u0`.
    pub id: String,
    /// Its character's name.
    pub character_name: String,
    /// The slot its bin sits in.
    pub slot: u16,
    /// Its skin bin's path, else its hash.
    pub file: String,
    /// The slot its skin entry's name spells.
    pub entry_name_slot: Option<u16>,
    /// How many bins the client loads for it, the skin bin among them.
    pub loads: usize,
}

/// The files and bytes of one kind, as `reforge-magic` names kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct KindCount {
    /// The kind.
    pub kind: String,
    /// How many files and entries are of it.
    pub files: usize,
    /// Their bytes.
    pub bytes: u64,
}

/// The files and bytes of one provenance fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SourceCount {
    /// The fact, as the load's file provenance names it: `install`, `history` or `own`.
    pub fact: String,
    /// How many files it covers.
    pub files: usize,
    /// Their bytes.
    pub bytes: u64,
}

/// A mod explored: the load report, rendered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ExploreView {
    /// The archive's path.
    pub archive: String,
    /// The mod, as its archive names it.
    pub meta: Option<ModView>,
    /// Its thumbnail as a `data:` URL, as the mod card's: the WebP the library encoded from
    /// the mod's image (#ADR-0051), never the archive's own bytes.
    pub thumbnail: Option<String>,
    /// The fatal finding that ended the run, by its id.
    pub halted: Option<u32>,
    /// An input that did not answer, which ended the run.
    pub fault: Option<String>,
    /// Every finding of `read` and `load`, in the order raised: the inputs' first.
    pub findings: Vec<Finding>,
    /// A name for each id of the report's tables, as `RunView::names` holds them.
    pub names: BTreeMap<String, String>,
    /// The dating. None when `load` did not end.
    pub dating: Option<DatingView>,
    /// The WADs.
    pub wads: Vec<WadView>,
    /// The characters.
    pub characters: Vec<CharacterView>,
    /// The skin units.
    pub units: Vec<UnitView>,
    /// The files and entries per kind, in the kind's order.
    pub kinds: Vec<KindCount>,
    /// The files per provenance fact, in the fact's order. Empty when `load` gave no
    /// provenance.
    pub sources: Vec<SourceCount>,
}

/// A mod explored, as the app holds it until the next.
pub struct ExploredMod {
    archive: Utf8PathBuf,
    raw: Option<RawSnapshot>,
    load: Option<LoadSnapshot>,
    layer: MemoryStore,
    view: ExploreView,
}

impl ExploredMod {
    /// The view.
    pub fn view(&self) -> &ExploreView {
        &self.view
    }

    /// The raw snapshot, when `read` ended.
    pub fn raw(&self) -> Option<&RawSnapshot> {
        self.raw.as_ref()
    }

    /// The load snapshot, when `load` ended.
    pub fn load(&self) -> Option<&LoadSnapshot> {
        self.load.as_ref()
    }

    /// The files at `load`, every mark `same`. None when `load` did not end.
    pub fn files(&self, store: Option<&AppStore>) -> Option<StageFiles> {
        let tiers = Tiers::layered(&self.layer, store);
        self.load
            .as_ref()
            .map(|load| store::load_files(load, &tiers))
    }

    /// One file by the id of its bytes, from what `load` made over the store's tiers, its
    /// names spelled through the load's names and `names`.
    pub fn file(
        &self,
        store: Option<&AppStore>,
        content: ContentId,
        names: Option<&Names>,
    ) -> Result<FileView, AppError> {
        let tiers = Tiers::layered(&self.layer, store);
        let book = || {
            Ok(self
                .load
                .as_ref()
                .map(|load| load.names().clone())
                .unwrap_or_default())
        };
        store::file_view(&tiers, content, book, names)
    }

    /// The archive and the standard settings, for the run page to run it.
    pub fn run_again(&self) -> RunAgain {
        RunAgain {
            kind: RunKind::Port,
            archive: self.archive.to_string(),
            settings: RunSettings::default(),
            archive_present: self.archive.is_file(),
        }
    }
}

/// One file by the id of its bytes, from the explored mod when there is one, else from the
/// store's tiers alone. Refused when none holds the bytes.
pub fn explored_file(
    explored: Option<&ExploredMod>,
    store: Option<&AppStore>,
    content: ContentId,
    names: Option<&Names>,
) -> Result<FileView, AppError> {
    match (explored, store) {
        (Some(explored), store) => explored.file(store, content, names),
        (None, Some(store)) => {
            store::file_view(&Tiers::of(store), content, || Ok(Default::default()), names)
        }
        (None, None) => Err(AppError::BlobMissing { id: content.hex() }),
    }
}

/// Explores the archive at `path`: a port's run with the standard settings through `load`,
/// reading through the store's tiers and starting from the raw snapshot its ingest tier holds
/// of the archive, else from the archive. A run halted in `read` or `load` keeps what it has.
pub fn explore(inputs: &Inputs, path: &Utf8Path, store: Option<&AppStore>) -> ExploredMod {
    let maker = Maker::Port(Porter::new(inputs.env()));
    let caps = maker.settings().caps();
    let cached = store.and_then(|store| ingested(store, path, caps));
    let mut run = maker.start(inputs, store, &mut Unwatched);
    if let Some(raw) = &cached {
        run = run.resume(raw.clone()).unwrap_or_else(|error| {
            tracing::warn!(%error, "an explore does not start from the ingest tier's raw snapshot");
            maker.start(inputs, store, &mut Unwatched)
        });
    }
    let read = if run.ran(reforge::diag::Stage::Read) {
        Ok(())
    } else {
        run.read_file(path).map(drop)
    };
    let fault = match read.and_then(|()| run.load().map(drop)) {
        Ok(()) | Err(RunError::Halted(_)) => None,
        Err(error) => Some(chain(&error)),
    };

    let raw = run.archive().cloned().or(cached);
    let thumbnail = raw
        .as_ref()
        .and_then(|raw| raw.image)
        .and_then(|image| image.open(&run.store()).ok())
        .and_then(|image| Thumbnail::of(&image).ok())
        .map(|thumbnail| data_url(&thumbnail));
    let load = run.load_snapshot().cloned();
    let report: Value = read_clean(&run.report().to_json());
    let meta = raw.as_ref().map(|raw| through_clean(&raw.meta));
    let speller = load
        .as_ref()
        .map(|load| Speller::new(load.names(), inputs.env().names()));
    let view = view_of(path, report, meta, thumbnail, fault, speller.as_ref());
    let layer = MemoryStore::new();
    for bytes in run.into_layer().into_values() {
        layer.put(bytes);
    }
    ExploredMod {
        archive: path.to_path_buf(),
        raw,
        load,
        layer,
        view,
    }
}

#[derive(Deserialize)]
struct WadPart {
    id: String,
    name: String,
    role: String,
    layout: String,
    files: usize,
    named: usize,
    #[serde(default)]
    packed_locale: Option<String>,
    restoration: bool,
}

#[derive(Deserialize)]
struct FilePart {
    id: String,
    #[serde(default)]
    path: Option<String>,
    hash: String,
    size: u64,
}

#[derive(Deserialize)]
struct CharacterPart {
    id: String,
    name: String,
    spelled: String,
    kind: String,
    wad: String,
    units: Vec<String>,
    #[serde(default)]
    root_bin: Option<String>,
    equal_to_game: bool,
}

#[derive(Deserialize)]
struct UnitPart {
    id: String,
    character_name: String,
    slot: u16,
    file: String,
    #[serde(default)]
    entry_name_slot: Option<u16>,
    loads: Vec<String>,
}

#[derive(Deserialize)]
struct BoundPart {
    build: String,
}

#[derive(Deserialize)]
struct EvidencePart {
    piece: String,
    id: String,
    hash: String,
}

#[derive(Deserialize)]
struct DatingPart {
    #[serde(default)]
    not_before: Option<BoundPart>,
    #[serde(default)]
    evidence: Option<EvidencePart>,
    dated: usize,
    at_bound: usize,
}

#[derive(Deserialize)]
struct TallyPart {
    files: usize,
    bytes: u64,
}

#[derive(Deserialize)]
struct InventoryPart {
    by_kind: BTreeMap<Kind, TallyPart>,
}

/// Whose bytes a file's are, as the library's `BytesOrigin` declares the facts, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Origin {
    Install,
    History,
    Own,
}

#[derive(Deserialize)]
struct ProvenancePart {
    file: String,
    origin: Origin,
}

/// The parts of the report the Mod page draws beside [`ReportParts`].
#[derive(Deserialize)]
struct LoadParts {
    #[serde(default)]
    wads: Vec<WadPart>,
    #[serde(default)]
    files: Vec<FilePart>,
    #[serde(default)]
    characters: Vec<CharacterPart>,
    #[serde(default)]
    units: Vec<UnitPart>,
    #[serde(default)]
    dating: Option<DatingPart>,
    #[serde(default)]
    inventory: Option<InventoryPart>,
    #[serde(default)]
    provenance: Option<Vec<ProvenancePart>>,
}

/// A value of the report, already cleaned, as a part.
///
/// # Panics
///
/// When the part does not read what the report wrote: a view out of step with the report's
/// schema, which the crate's tests hold.
fn part_of<T: serde::de::DeserializeOwned>(report: &Value) -> T {
    serde_json::from_value(report.clone()).unwrap_or_else(|error| {
        panic!(
            "a view of {} does not read the report: {error}",
            std::any::type_name::<T>()
        )
    })
}

/// The view of a run's report, read through `clean`. `speller` spells an object that dated
/// the mod; the run's load names it.
fn view_of(
    path: &Utf8Path,
    report: Value,
    meta: Option<ModView>,
    thumbnail: Option<String>,
    fault: Option<String>,
    speller: Option<&Speller<'_>>,
) -> ExploreView {
    let mut parts: ReportParts = part_of(&report);
    let load: LoadParts = part_of(&report);
    let names = parts.names();
    let files: BTreeMap<&str, &FilePart> = load.files.iter().map(|f| (f.id.as_str(), f)).collect();
    let file_name = |id: &str| {
        files.get(id).map_or_else(
            || id.to_string(),
            |file| file.path.clone().unwrap_or_else(|| file.hash.clone()),
        )
    };
    let wad_name = |id: &str| names.get(id).cloned().unwrap_or_else(|| id.to_string());

    let dating = load.dating.map(|dating| DatingView {
        not_before: dating.not_before.map(|bound| bound.build),
        evidence: dating.evidence.map(|evidence| {
            let name = match evidence.piece.as_str() {
                "file" => file_name(&evidence.id),
                _ => object_name(&evidence.hash, speller),
            };
            EvidenceView {
                kind: evidence.piece,
                name,
                hash: evidence.hash,
            }
        }),
        dated: dating.dated,
        at_bound: dating.at_bound,
    });
    let wads = load.wads.into_iter().map(|wad| WadView {
        id: wad.id,
        name: wad.name,
        role: wad.role,
        layout: wad.layout,
        files: wad.files,
        named: wad.named,
        packed_locale: wad.packed_locale,
        restoration: wad.restoration,
    });
    let characters = load.characters.iter().map(|character| CharacterView {
        id: character.id.clone(),
        name: character.name.clone(),
        spelled: character.spelled.clone(),
        kind: character.kind.clone(),
        wad: wad_name(&character.wad),
        units: character.units.len(),
        equal_to_game: character.equal_to_game,
        root_bin: character.root_bin.as_deref().map(file_name),
    });
    let units = load.units.iter().map(|unit| UnitView {
        id: unit.id.clone(),
        character_name: unit.character_name.clone(),
        slot: unit.slot,
        file: file_name(&unit.file),
        entry_name_slot: unit.entry_name_slot,
        loads: unit.loads.len(),
    });
    let kinds = load.inventory.into_iter().flat_map(|inventory| {
        let by_kind = inventory.by_kind.into_iter();
        by_kind.map(|(kind, tally)| KindCount {
            kind: store::kind_name(kind),
            files: tally.files,
            bytes: tally.bytes,
        })
    });
    let mut sources: BTreeMap<Origin, (usize, u64)> = BTreeMap::new();
    for row in load.provenance.iter().flatten() {
        let count = sources.entry(row.origin).or_default();
        count.0 += 1;
        count.1 += files.get(row.file.as_str()).map_or(0, |file| file.size);
    }
    let sources = sources
        .into_iter()
        .map(|(origin, (files, bytes))| SourceCount {
            fact: origin_key(origin),
            files,
            bytes,
        });

    ExploreView {
        archive: clean(path.as_str()).into_owned(),
        meta: parts.meta.take().or(meta),
        thumbnail,
        halted: parts.halted,
        fault,
        findings: std::mem::take(&mut parts.diagnostics),
        dating,
        wads: wads.collect(),
        characters: characters.collect(),
        units: units.collect(),
        kinds: kinds.collect(),
        sources: sources.collect(),
        names,
    }
}

/// An object by its entry name, else its hash as the report writes it.
fn object_name(hash: &str, speller: Option<&Speller<'_>>) -> String {
    let entry = u32::from_str_radix(hash, 16).ok().map(BinHash);
    let spelled = entry
        .zip(speller)
        .map(|(entry, speller)| speller.entry_or_hex(entry));
    // A name the run learnt can be the mod's own text.
    clean(&spelled.unwrap_or_else(|| hash.to_string())).into_owned()
}

/// A fact as `FileFact` serialises its origin.
fn origin_key(origin: Origin) -> String {
    match serde_json::to_value(origin) {
        Ok(Value::String(key)) => key,
        _ => format!("{origin:?}"),
    }
}
