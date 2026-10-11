//! The files of a kept run at a stage (#ADR-0094): what the stage's snapshot names, by WAD,
//! each with its bytes' size and kind, and a mark against the nearest earlier stage the run
//! holds files for, or against another run's files at the same stage. The Runs page renders it
//! and adds no rule of its own (#ADR-0046). The Mod page lists an explored mod's files at
//! `load`, and the run page its last run's at each stage, through the same listing, over that
//! run's own layer above the store's tiers.
//!
//! `load` and `fix` name a file by its WAD and path hash. `plan` and `build` name it by the
//! path it is placed at, whose hash is the one compared, so a file a port moves is removed at
//! its old path and added at its new one.

use std::collections::BTreeMap;

use reforge::model::entity::{FileId, WadId};
use reforge::model::hash::wad_hash_of_hex;
use reforge::model::{ByteView, ChunkPath, ContentId, FactAccess, Head, Layer, clean};
use reforge::snapshot::{self, BuildSnapshot, FixSnapshot, LoadSnapshot, PlanSnapshot, Snapshots};
use reforge::store::BlobStore;
use reforge_magic::{GENERATION, HEAD_LEN, HeadKind, Kind};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use super::{AppStore, failed, unpack};
use crate::error::AppError;
use crate::run::StageName;

/// The stages whose snapshot names files, in stage order.
pub const FILE_STAGES: [StageName; 4] = [
    StageName::Load,
    StageName::Fix,
    StageName::Plan,
    StageName::Build,
];

/// How a file stands against the stage before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum Mark {
    /// The same bytes at the same place.
    Same,
    /// Other bytes at the same place.
    Changed,
    /// At a place the stage before had none at.
    Added,
    /// At a place the stage before had, and this stage has not.
    Removed,
}

/// One file of a stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct FileRow {
    /// Its path hash, as sixteen hex digits.
    pub hash: String,
    /// Its path. None when the stage names the file by its hash alone.
    pub path: Option<String>,
    /// The id of its bytes, as 64 hex digits.
    pub content: String,
    /// Its size in bytes. Zero when no tier holds its bytes.
    pub size: u64,
    /// Its kind, as `reforge-magic` names kinds: `unknown` when none matched, `missing` when
    /// no tier holds its bytes.
    pub kind: String,
    /// How it stands against the stage before.
    pub mark: Mark,
    /// Whose bytes it holds, as the load's file provenance names the fact: `install`,
    /// `history` or `own`. Set for an explored mod's file the provenance covers, None for a
    /// kept run's file.
    #[serde(default)]
    pub source: Option<String>,
}

/// The files of one WAD at a stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct WadFiles {
    /// The WAD's name. A layer other than the base one names its WADs under it:
    /// `hud/Ahri.wad.client`.
    pub name: String,
    /// Its files in path order, the ones named by their hash alone last.
    pub files: Vec<FileRow>,
}

/// The files of a kept run at a stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct StageFiles {
    /// The stage.
    pub stage: StageName,
    /// Its WADs, by name.
    pub wads: Vec<WadFiles>,
}

/// The snapshots of a kept run that name files.
pub struct KeptSnapshots {
    /// The snapshot after `load`. None for a run opened from a package, which has none.
    pub load: Option<LoadSnapshot>,
    /// The snapshot after `fix`.
    pub fix: Option<FixSnapshot>,
    /// The snapshot after `plan`.
    pub plan: Option<PlanSnapshot>,
    /// The snapshot after `build`.
    pub build: Option<BuildSnapshot>,
}

impl KeptSnapshots {
    /// The snapshots that name files of a run's snapshots from `load` on.
    pub fn of(snapshots: &Snapshots) -> Self {
        Self {
            load: Some(snapshots.load.clone()),
            fix: snapshots.fix.clone(),
            plan: snapshots.plan.clone(),
            build: snapshots.build.clone(),
        }
    }

    /// The files at a stage, each marked against the nearest earlier stage the run holds files
    /// for, with their sizes and kinds from `tiers`. Refused for a stage whose snapshot names
    /// no files, or one the run did not reach.
    pub fn files(&self, stage: StageName, tiers: &Tiers<'_>) -> Result<StageFiles, AppError> {
        let current = self.held_listing(stage)?;
        let held = self.stages();
        let before = held.iter().rev().find(|&&earlier| earlier < stage);
        let before = before.and_then(|&earlier| self.listing(earlier));
        Ok(marked(tiers, stage, &current, before.as_ref()))
    }

    /// The stages of [`FILE_STAGES`] the run holds a snapshot of.
    pub fn stages(&self) -> Vec<StageName> {
        let held = [
            self.load.is_some(),
            self.fix.is_some(),
            self.plan.is_some(),
            self.build.is_some(),
        ];
        FILE_STAGES
            .into_iter()
            .zip(held)
            .filter_map(|(stage, held)| held.then_some(stage))
            .collect()
    }

    /// The files a stage's snapshot names. Refused for a stage whose snapshot names no files,
    /// or one the run did not reach.
    fn held_listing(&self, stage: StageName) -> Result<Listing, AppError> {
        let held = self.stages().contains(&stage);
        let listing = held.then(|| self.listing(stage)).flatten();
        listing.ok_or(AppError::NoFilesAtStage { stage })
    }

    /// The files a stage's snapshot names, by WAD and path hash. None for a stage the run
    /// holds no files at.
    fn listing(&self, stage: StageName) -> Option<Listing> {
        let load = self.load.as_ref()?;
        let mut listing = Listing::new();
        match stage {
            StageName::Load => return Some(load_listing(load, &BTreeMap::new())),
            StageName::Fix => {
                let load_wads = load.wads();
                for (&wad, files) in self.fix.as_ref()?.wads() {
                    for (hex, &content) in files {
                        let Some(hash) = wad_hash_of_hex(hex) else {
                            continue;
                        };
                        let loaded = load_wads.get(&wad).and_then(|f| f.get(&hash));
                        let path = loaded.and_then(|f| f.path).map(|p| p.as_str().to_string());
                        listing.add(wad_name(load, wad), hash.0, path, content);
                    }
                }
            }
            StageName::Plan => {
                for (wad, files) in self.plan.as_ref()?.wads() {
                    for (place, &content) in files {
                        listing.add_placed(wad.clone(), place, content);
                    }
                }
            }
            StageName::Build => {
                for (layer, wad, path, content) in self.build.as_ref()?.chunks() {
                    let wad = match layer {
                        Layer::Base => wad.to_string(),
                        layer => format!("{}/{wad}", layer.name()),
                    };
                    listing.add_placed(wad, path.as_str(), content);
                }
            }
            _ => return None,
        }
        Some(listing)
    }
}

/// A WAD's name as the load snapshot's rows give it, else its id.
fn wad_name(load: &LoadSnapshot, id: WadId) -> String {
    let rows = &load.part().wads;
    rows.iter()
        .find(|row| row.id == id)
        .map_or_else(|| format!("{id}"), |row| row.name.to_string())
}

/// The files `load` left, by WAD and path hash, each with the fact `sources` holds for it.
fn load_listing(load: &LoadSnapshot, sources: &BTreeMap<FileId, String>) -> Listing {
    let mut listing = Listing::new();
    let ids = load.part().files.iter().map(|row| {
        let hash = wad_hash_of_hex(&row.hash).map(|hash| hash.0);
        ((row.wad, hash), row.id)
    });
    let ids: BTreeMap<_, _> = ids.collect();
    for (&wad, files) in &load.wads() {
        for (&hash, file) in files {
            let path = file.path.map(|path| path.as_str().to_string());
            let id = ids.get(&(wad, Some(hash.0)));
            let source = id.and_then(|id| sources.get(id)).cloned();
            listing.add_with(wad_name(load, wad), hash.0, path, file.content, source);
        }
    }
    listing
}

/// The fact the load's file provenance gives each file it covers, as `FileFact` serialises its
/// origin. Empty for a load that gave no provenance, such as a repair's.
fn provenance_sources(load: &LoadSnapshot) -> BTreeMap<FileId, String> {
    let rows = load.part().provenance.iter().flatten();
    let facts = rows.filter_map(|row| {
        let fact = serde_json::to_value(&row.fact).ok()?;
        let origin = fact.get("origin")?.as_str()?.to_string();
        Some((row.file, origin))
    });
    facts.collect()
}

/// The files of an explored mod at `load` (#ADR-0091): every file the load snapshot names, each
/// the same, with its size and kind from `tiers` and its provenance fact.
pub fn load_files(load: &LoadSnapshot, tiers: &Tiers<'_>) -> StageFiles {
    let listing = load_listing(load, &provenance_sources(load));
    marked(tiers, StageName::Load, &listing, None)
}

/// A file a stage names.
#[derive(Debug, Clone)]
struct Listed {
    path: Option<String>,
    /// The name its kind is matched with: its path, or for a file placed at its hash, that
    /// place, whose extension says the kind.
    name: Option<String>,
    content: ContentId,
    source: Option<String>,
}

/// The files a stage names, by WAD name and path hash. The first file at a place is kept.
#[derive(Debug, Default)]
struct Listing(BTreeMap<(String, u64), Listed>);

impl Listing {
    fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, wad: String, hash: u64, path: Option<String>, content: ContentId) {
        self.add_with(wad, hash, path, content, None);
    }

    fn add_with(
        &mut self,
        wad: String,
        hash: u64,
        path: Option<String>,
        content: ContentId,
        source: Option<String>,
    ) {
        let name = path.clone();
        let listed = Listed {
            path,
            name,
            content,
            source,
        };
        self.0.entry((wad, hash)).or_insert(listed);
    }

    /// A file at the place a port or a build put it.
    fn add_placed(&mut self, wad: String, place: &str, content: ContentId) {
        match hashed_place(place) {
            Some(hash) => {
                let listed = Listed {
                    path: None,
                    name: Some(place.to_string()),
                    content,
                    source: None,
                };
                self.0.entry((wad, hash)).or_insert(listed);
            }
            None => {
                let hash = ChunkPath::parse(place)
                    .map_or_else(|_| reforge::model::path_hash(place).0, |path| path.hash().0);
                self.add(wad, hash, Some(place.to_string()), content);
            }
        }
    }
}

/// The hash a place names when it is a hash at its WAD's root with an extension, which a
/// packer reads back as the hash (#MOD-ARTIFACT-0007).
fn hashed_place(place: &str) -> Option<u64> {
    let (stem, extension) = place.split_once('.')?;
    let hex = stem.len() == 16 && stem.bytes().all(|b| b.is_ascii_hexdigit());
    (hex && !extension.contains('/'))
        .then(|| u64::from_str_radix(stem, 16).ok())
        .flatten()
}

/// Where a listing reads its files' sizes and kinds and the viewer their bytes, the first that
/// holds an id: a run's own memory layer, then the store's run tier and ingest tier, with the
/// ingest tier's table of facts.
pub struct Tiers<'a> {
    layer: Option<&'a dyn BlobStore>,
    store: Option<&'a AppStore>,
}

impl<'a> Tiers<'a> {
    /// The store's run tier over its ingest tier.
    pub fn of(store: &'a AppStore) -> Self {
        Self {
            layer: None,
            store: Some(store),
        }
    }

    /// A run's own layer over the store's tiers, when there is a store.
    pub fn layered(layer: &'a dyn BlobStore, store: Option<&'a AppStore>) -> Self {
        Self {
            layer: Some(layer),
            store,
        }
    }

    fn stores(&self) -> impl Iterator<Item = &'a dyn BlobStore> {
        let tiers = self.store.into_iter().flat_map(|store| {
            let tier: &dyn BlobStore = store.tier();
            let ingest: &dyn BlobStore = store.ingest().store();
            [tier, ingest]
        });
        self.layer.into_iter().chain(tiers)
    }

    /// The bytes under an id. None when no tier holds them.
    pub(super) fn get(&self, id: ContentId) -> Option<ByteView> {
        self.stores().find_map(|store| store.get(id))
    }

    /// The first `len` bytes under an id, with its size. None when no tier holds them.
    pub(super) fn get_head(&self, id: ContentId, len: usize) -> Option<Head> {
        self.stores().find_map(|store| store.get_head(id, len))
    }

    /// What is known of a blob: its size and what its first bytes say. None for one no tier
    /// holds.
    pub(super) fn blob_facts(&self, id: ContentId) -> Option<(u64, HeadKind)> {
        if !self.stores().any(|store| store.contains(id)) {
            return None;
        }
        let row = self.store.and_then(|store| store.ingest().facts().row(id));
        let kept = row
            .filter(|row| row.generation == GENERATION)
            .and_then(|row| {
                let kind = serde_json::from_str::<HeadKind>(&row.kind).ok()?;
                Some((row.size, kind))
            });
        if kept.is_some() {
            return kept;
        }
        let head = self.get_head(id, HEAD_LEN)?;
        Some((head.size, HeadKind::of(&head.bytes)))
    }
}

/// A kind as `reforge-magic` names it.
pub(crate) fn kind_name(kind: Kind) -> String {
    match serde_json::to_value(kind) {
        Ok(serde_json::Value::String(name)) => name,
        _ => "unknown".to_string(),
    }
}

/// The kind of a blob named `name`, by its first bytes and its name.
pub(super) fn kind_of(head: HeadKind, name: Option<&str>) -> Kind {
    let file_name = name.map(|name| name.rsplit('/').next().unwrap_or(name));
    head.identify(file_name).kind
}

impl AppStore {
    /// The snapshots of a kept run that name files. None for a number no run has.
    pub fn kept_snapshots(&self, id: u64) -> Result<Option<KeptSnapshots>, AppError> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(None);
        };
        let query = "SELECT load.json, run.fix, run.plan, run.build
             FROM run LEFT JOIN load ON load.id = run.load_id WHERE run.id = ?1";
        type Columns = (
            Option<Vec<u8>>,
            Option<Vec<u8>>,
            Option<Vec<u8>>,
            Option<Vec<u8>>,
        );
        let columns: Option<Columns> = {
            let rows = self.rows.lock();
            let row = rows.query_row(query, [id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            });
            row.optional()?
        };
        let Some((load, fix, plan, build)) = columns else {
            return Ok(None);
        };
        Ok(Some(KeptSnapshots {
            load: load.as_deref().map(read_snapshot).transpose()?,
            fix: fix.as_deref().map(read_snapshot).transpose()?,
            plan: plan.as_deref().map(read_snapshot).transpose()?,
            build: build.as_deref().map(read_snapshot).transpose()?,
        }))
    }

    /// The files of a kept run at a stage, each marked against the nearest earlier stage the
    /// run holds files for. None for a number no run has; refused for a stage whose snapshot
    /// names no files, or one the run did not reach.
    pub fn stage_files(&self, id: u64, stage: StageName) -> Result<Option<StageFiles>, AppError> {
        let Some(kept) = self.kept_snapshots(id)? else {
            return Ok(None);
        };
        kept.files(stage, &Tiers::of(self)).map(Some)
    }

    /// The files of kept run `b` at a stage, each marked against kept run `a`'s at the same
    /// stage. None when either number names no run; refused when either run holds no files at
    /// the stage.
    pub fn compare_files(
        &self,
        a: u64,
        b: u64,
        stage: StageName,
    ) -> Result<Option<StageFiles>, AppError> {
        let (Some(a), Some(b)) = (self.kept_snapshots(a)?, self.kept_snapshots(b)?) else {
            return Ok(None);
        };
        let before = a.held_listing(stage)?;
        let current = b.held_listing(stage)?;
        Ok(Some(marked(
            &Tiers::of(self),
            stage,
            &current,
            Some(&before),
        )))
    }
}

/// The files of `current`, each marked against `before`, and those of `before` it lacks
/// marked removed. With nothing before, every file is the same.
fn marked(
    tiers: &Tiers<'_>,
    stage: StageName,
    current: &Listing,
    before: Option<&Listing>,
) -> StageFiles {
    let mut wads: BTreeMap<String, Vec<FileRow>> = BTreeMap::new();
    for ((wad, hash), listed) in &current.0 {
        let mark = match before.map(|b| b.0.get(&(wad.clone(), *hash))) {
            None => Mark::Same,
            Some(None) => Mark::Added,
            Some(Some(earlier)) if earlier.content == listed.content => Mark::Same,
            Some(Some(_)) => Mark::Changed,
        };
        let row = file_row(tiers, *hash, listed, mark);
        wads.entry(wad.clone()).or_default().push(row);
    }
    for ((wad, hash), listed) in before.iter().flat_map(|b| &b.0) {
        if !current.0.contains_key(&(wad.clone(), *hash)) {
            let row = file_row(tiers, *hash, listed, Mark::Removed);
            wads.entry(wad.clone()).or_default().push(row);
        }
    }
    let wads = wads.into_iter().map(|(name, mut files)| {
        files.sort_by(|a, b| {
            let key = |row: &FileRow| (row.path.is_none(), row.path.clone(), row.hash.clone());
            key(a).cmp(&key(b))
        });
        WadFiles {
            name: clean(&name).into_owned(),
            files,
        }
    });
    StageFiles {
        stage,
        wads: wads.collect(),
    }
}

fn file_row(tiers: &Tiers<'_>, hash: u64, listed: &Listed, mark: Mark) -> FileRow {
    let (size, kind) = match tiers.blob_facts(listed.content) {
        Some((size, head)) => (size, kind_name(kind_of(head, listed.name.as_deref()))),
        None => (0, "missing".to_string()),
    };
    FileRow {
        hash: format!("{hash:016x}"),
        path: listed.path.as_deref().map(|path| clean(path).into_owned()),
        content: listed.content.hex(),
        size,
        kind,
        mark,
        source: listed.source.clone(),
    }
}

/// A snapshot from its packed column.
pub(super) fn read_snapshot<S: snapshot::StageSnapshot>(column: &[u8]) -> Result<S, AppError> {
    snapshot::from_json(&unpack(column)?).map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::hashed_place;

    #[test]
    fn a_hash_at_the_wads_root_with_an_extension_is_read_as_the_hash() {
        assert_eq!(
            hashed_place("94553db99d455089.bin"),
            Some(0x9455_3db9_9d45_5089)
        );
        assert_eq!(hashed_place("assets/94553db99d455089.bin"), None);
        assert_eq!(hashed_place("94553db99d45508.bin"), None);
        assert_eq!(hashed_place("skin0.bin"), None);
    }
}
