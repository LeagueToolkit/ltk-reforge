//! A project file imported (#ADR-0096). The file is untrusted input and is read as a mod
//! archive is (`docs/design/ARCHITECTURE.md`, "Untrusted input"): its entries counted under
//! the caps, `project.json` read up to a cap, every other entry a blob named by its content id,
//! its size bounded, read whole and refused when its bytes are not the content its name
//! spells. Nothing is extracted to disk under a name of the file's: a blob goes into a tier
//! under its content id.
//!
//! The entries are read twice: once to check every blob, then once to put them, so a file
//! with one bad blob leaves nothing in either tier.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufReader, Read, Seek};

use camino::Utf8Path;
use fs_err as fs;
use reforge::archive::{Caps, RAW_VERSION, RawSnapshot};
use reforge::model::{ByteView, ContentId, clean_json};
use reforge::snapshot::{
    self, BuildSnapshot, CheckSnapshot, DecideSnapshot, FactsSnapshot, FixSnapshot, LoadSnapshot,
    PlanSnapshot, SNAPSHOT_FORMAT, Snapshots, StageSnapshot,
};
use reforge::store::BlobStore;
use rusqlite::{Transaction, params};
use serde_json::Value;
use zip::ZipArchive;

use super::{BLOBS, PROJECT_FORMAT, PROJECT_JSON, ProjectJson};
use crate::error::AppError;
use crate::run::{RunSettings, RunView};
use crate::store::read::{Origin, kind_of};
use crate::store::{AppStore, failed, json_of, kind_text, load_key, number, pack, pack_snapshot};

/// A refusal of the file, with what is wrong with it.
fn malformed(detail: impl std::fmt::Display) -> AppError {
    AppError::ProjectMalformed {
        detail: detail.to_string(),
    }
}

/// The project's run, its snapshots read and every value held to its shape.
struct Imported {
    raw: RawSnapshot,
    /// The mod's row: its path, file name and text, as the project gives them.
    path: String,
    file_name: String,
    name: Option<String>,
    author: Option<String>,
    mod_version: Option<String>,
    snapshots: Option<Snapshots>,
    kind: &'static str,
    settings: RunSettings,
    schema_build: Option<String>,
    program_version: String,
    program_commit: Option<String>,
    package: Option<String>,
    made_at: String,
    report: String,
    view: RunView,
}

impl Imported {
    /// The ids the run names apart from its raw snapshot's: what a save lists in `run_blob`.
    fn run_ids(&self, listed: &BTreeMap<ContentId, u64>) -> BTreeSet<ContentId> {
        let raw: BTreeSet<ContentId> = self.raw.ids().collect();
        let mut ids: BTreeSet<ContentId> = listed
            .keys()
            .filter(|id| !raw.contains(id))
            .copied()
            .collect();
        ids.extend(self.snapshots.iter().flat_map(Snapshots::ids));
        ids
    }
}

/// A stage's snapshot from its JSON, refused when it is of another format than this program
/// reads or does not read as its shape.
fn stage_snapshot<S: StageSnapshot>(value: Value, what: &str) -> Result<S, AppError> {
    let format = value.get("format").and_then(Value::as_u64);
    if format != Some(u64::from(SNAPSHOT_FORMAT)) {
        return Err(malformed(format!(
            "the {what} snapshot is of format {format:?}"
        )));
    }
    let json = value.to_string();
    snapshot::from_json(&json).map_err(|error| malformed(format!("the {what} snapshot: {error}")))
}

/// The snapshot of a stage after `load` the run may lack.
fn later_snapshot<S: StageSnapshot>(
    value: Option<Value>,
    what: &str,
) -> Result<Option<S>, AppError> {
    value.map(|value| stage_snapshot(value, what)).transpose()
}

/// Whether a text is the time the store writes: `YYYY-MM-DD HH:MM:SS`, as SQLite's
/// `CURRENT_TIMESTAMP` gives it.
fn is_store_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 19
        && bytes.iter().enumerate().all(|(at, &byte)| match at {
            4 | 7 => byte == b'-',
            10 => byte == b' ',
            13 | 16 => byte == b':',
            _ => byte.is_ascii_digit(),
        })
}

/// A content id as an entry's name or the list spells it: 64 lowercase hex digits.
fn lowercase_id(text: &str) -> Option<ContentId> {
    let lowercase = text
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    lowercase.then(|| ContentId::parse(text)).flatten()
}

impl Imported {
    /// The project's run from its JSON.
    fn of(project: ProjectJson) -> Result<Self, AppError> {
        let raw_row = project.raw;
        let sha256 = lowercase_id(&raw_row.sha256)
            .ok_or_else(|| malformed("the mod's SHA-256 is not 64 lowercase hex digits"))?;
        let raw: RawSnapshot = snapshot::from_json(&project.raw_snapshot.to_string())
            .map_err(|error| malformed(format!("the raw snapshot: {error}")))?;
        // As the ingest tier takes a cache entry (#ADR-0095).
        if raw.version != RAW_VERSION {
            return Err(malformed(format!(
                "the raw snapshot is of reading {}, and this program reads {RAW_VERSION}",
                raw.version
            )));
        }
        if raw.meta.sha256 != sha256 || raw.caps != raw_row.caps || raw_row.version != raw.version {
            return Err(malformed("the raw snapshot is not the mod's row's"));
        }
        let run = project.run;
        let later = [
            run.fix.is_some(),
            run.facts.is_some(),
            run.decide.is_some(),
            run.plan.is_some(),
            run.build.is_some(),
            run.check.is_some(),
        ];
        let snapshots = match project.load {
            Some(load) => {
                let load: LoadSnapshot = stage_snapshot(load, "load")?;
                if load.meta().sha256 != sha256 {
                    return Err(malformed("the load snapshot is of another archive"));
                }
                let mut snapshots = Snapshots::new(load);
                snapshots.fix = later_snapshot(run.fix, "fix")?;
                snapshots.facts = later_snapshot(run.facts, "facts")?;
                snapshots.decide = later_snapshot(run.decide, "decide")?;
                snapshots.plan = later_snapshot(run.plan, "plan")?;
                snapshots.build = later_snapshot(run.build, "build")?;
                snapshots.check = later_snapshot(run.check, "check")?;
                Some(snapshots)
            }
            None if later.contains(&true) => {
                return Err(malformed("the run holds later snapshots and no load"));
            }
            None => None,
        };
        let kind = match kind_of(&run.kind) {
            Ok(kind) => kind_text(kind),
            Err(_) => return Err(malformed("the run's kind is no repair or port")),
        };
        if !is_store_time(&run.made_at) {
            return Err(malformed("the run's time is not the store's"));
        }
        let settings = serde_json::from_value(run.settings)
            .map_err(|error| malformed(format!("the run's settings: {error}")))?;
        let mut view: RunView = serde_json::from_value(run.view)
            .map_err(|error| malformed(format!("the run's view: {error}")))?;
        view.saved = None;
        if !run.report.is_object() {
            return Err(malformed("the run's report is no JSON object"));
        }
        Ok(Self {
            raw,
            path: raw_row.path,
            file_name: raw_row.file_name,
            name: raw_row.name,
            author: raw_row.author,
            mod_version: raw_row.mod_version,
            snapshots,
            kind,
            settings,
            schema_build: run.schema_build,
            program_version: run.program_version,
            program_commit: run.program_commit,
            package: run.package,
            // The time the exporting store made the run, kept as it is: the store's clock says
            // when a run was made, and an import makes none.
            made_at: run.made_at,
            // As a report is kept: every character a terminal acts on written as an escape.
            report: clean_json(&run.report.to_string()).into_owned(),
            view,
        })
    }
}

/// The blobs a project lists, by id, each with its size. Refused for an id that is not 64
/// lowercase hex digits, or one listed twice.
fn listed_blobs(project: &ProjectJson) -> Result<BTreeMap<ContentId, u64>, AppError> {
    let mut listed = BTreeMap::new();
    for blob in &project.blobs {
        let id = lowercase_id(&blob.id)
            .ok_or_else(|| malformed("a listed blob's id is not 64 lowercase hex digits"))?;
        if listed.insert(id, blob.bytes).is_some() {
            return Err(malformed(format!("the blob {} is listed twice", id.hex())));
        }
    }
    Ok(listed)
}

/// The bytes of an entry, read whole up to `cap`. Refused past it.
fn read_capped(entry: &mut impl Read, cap: u64, name: &str) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    entry
        .take(cap.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| malformed(format!("{name} does not read: {error}")))?;
    if bytes.len() as u64 > cap {
        return Err(malformed(format!("{name} is over {cap} bytes")));
    }
    Ok(bytes)
}

/// The project's zip, its `project.json` checked and its blob entries matched to the list.
struct Archive<R> {
    zip: ZipArchive<R>,
    /// Each blob entry's place in the zip, by id.
    blobs: BTreeMap<ContentId, usize>,
}

impl<R: Read + Seek> Archive<R> {
    /// Matches every entry other than `project.json` to a listed blob by its name, and
    /// bounds each one's size as its header says. Refused for an entry of any other name, a
    /// name twice, a blob the list does not name or names otherwise, and a listed blob no
    /// entry holds.
    fn matched(
        mut zip: ZipArchive<R>,
        listed: &BTreeMap<ContentId, u64>,
        caps: &Caps,
    ) -> Result<Self, AppError> {
        let mut blobs = BTreeMap::new();
        let mut total = 0u64;
        let mut json_entries = 0;
        for at in 0..zip.len() {
            let entry = zip.by_index_raw(at).map_err(malformed)?;
            let name = entry.name();
            if name == PROJECT_JSON {
                json_entries += 1;
                continue;
            }
            let id = name.strip_prefix(BLOBS).and_then(lowercase_id);
            let Some(id) = id else {
                return Err(malformed("an entry is neither project.json nor a blob"));
            };
            let Some(&bytes) = listed.get(&id) else {
                return Err(malformed(format!("the blob {} is not listed", id.hex())));
            };
            let size = entry.size();
            if size > caps.chunk_bytes {
                return Err(malformed(format!("the blob {} is over the cap", id.hex())));
            }
            if size != bytes {
                return Err(malformed(format!(
                    "the blob {} is not its listed size",
                    id.hex()
                )));
            }
            total = total.saturating_add(size);
            if total > caps.total_bytes {
                return Err(malformed("the blobs are over the cap on all bytes"));
            }
            if blobs.insert(id, at).is_some() {
                return Err(malformed(format!(
                    "the blob {} is in two entries",
                    id.hex()
                )));
            }
        }
        if json_entries > 1 {
            return Err(malformed("project.json is in two entries"));
        }
        if let Some(id) = listed.keys().find(|id| !blobs.contains_key(id)) {
            return Err(malformed(format!(
                "the blob {} is listed and absent",
                id.hex()
            )));
        }
        Ok(Self { zip, blobs })
    }

    /// The bytes of a blob's entry, refused when they are not the content its name spells.
    fn blob(&mut self, id: ContentId, caps: &Caps) -> Result<ByteView, AppError> {
        let at = self.blobs[&id];
        let mut entry = self.zip.by_index(at).map_err(malformed)?;
        let name = format!("{BLOBS}{}", id.hex());
        let bytes = read_capped(&mut entry, caps.chunk_bytes, &name)?;
        if ContentId::of(&bytes) != id {
            return Err(malformed(format!(
                "the bytes of {name} are not its content"
            )));
        }
        Ok(ByteView::from(bytes))
    }

    /// Checks every blob's bytes against its name.
    fn check_all(&mut self, caps: &Caps) -> Result<(), AppError> {
        let ids: Vec<ContentId> = self.blobs.keys().copied().collect();
        for id in ids {
            self.blob(id, caps)?;
        }
        Ok(())
    }
}

/// `project.json`, read up to the cap on one entry, its format checked first.
fn project_json<R: Read + Seek>(
    zip: &mut ZipArchive<R>,
    caps: &Caps,
) -> Result<ProjectJson, AppError> {
    let mut entry = zip
        .by_name(PROJECT_JSON)
        .map_err(|_| malformed("the file holds no project.json"))?;
    let bytes = read_capped(&mut entry, caps.entry_bytes, PROJECT_JSON)?;
    let value: Value = serde_json::from_slice(&bytes).map_err(malformed)?;
    match value.get("format").and_then(Value::as_u64) {
        Some(found) if found == u64::from(PROJECT_FORMAT) => {}
        Some(found) => return Err(AppError::ProjectFormat { found }),
        None => return Err(malformed("project.json states no format")),
    }
    serde_json::from_value(value).map_err(malformed)
}

impl AppStore {
    /// Imports a project file as a kept run, and answers its number (#ADR-0096).
    ///
    /// The blobs the raw snapshot names go into the ingest tier and the raw snapshot into its
    /// cache; the rest go into the run tier. The rows are written as a save writes them,
    /// under the store's lock with the run tier's bytes flushed first: the mod's row unless
    /// one is there, the load unless one of its key is there, the run with the origin
    /// `project`, the file's path and the mark kept, and its `run_blob` rows. The mod's path
    /// is the project's own: the archive may be elsewhere on this machine.
    pub fn import_project(&self, path: &Utf8Path) -> Result<u64, AppError> {
        let caps = Caps::default();
        let file = fs::File::open(path).map_err(malformed)?;
        let mut zip = ZipArchive::new(BufReader::new(file)).map_err(malformed)?;
        if zip.len() > caps.entries {
            return Err(malformed(format!("{} entries, over the cap", zip.len())));
        }
        let project = project_json(&mut zip, &caps)?;
        let listed = listed_blobs(&project)?;
        let imported = Imported::of(project)?;
        let named = imported
            .raw
            .ids()
            .chain(imported.snapshots.iter().flat_map(Snapshots::ids));
        if let Some(id) = named.into_iter().find(|id| !listed.contains_key(id)) {
            return Err(malformed(format!(
                "the blob {} is named and not listed",
                id.hex()
            )));
        }
        let mut archive = Archive::matched(zip, &listed, &caps)?;
        archive.check_all(&caps)?;

        let raw_ids: BTreeSet<ContentId> = imported.raw.ids().collect();
        for &id in &raw_ids {
            let put = self.ingest.store().put(archive.blob(id, &caps)?);
            if put != id {
                return Err(malformed(format!("the bytes of {} changed", id.hex())));
            }
        }
        self.ingest.put(&imported.raw).map_err(failed)?;
        let run_ids = imported.run_ids(&listed);
        self.change(|tier, tx| {
            for &id in listed.keys().filter(|id| !raw_ids.contains(id)) {
                if BlobStore::put(tier, archive.blob(id, &caps)?) != id {
                    return Err(malformed(format!("the bytes of {} changed", id.hex())));
                }
            }
            // The puts reach the disk before a row names them.
            tier.flush(&BTreeMap::new())?;
            let id = insert_imported(tx, &imported, path, &run_ids)?;
            Ok((id, 0))
        })
    }
}

/// Writes an imported run's rows, and answers its number.
fn insert_imported(
    tx: &Transaction<'_>,
    imported: &Imported,
    path: &Utf8Path,
    run_ids: &BTreeSet<ContentId>,
) -> Result<u64, AppError> {
    let raw = &imported.raw;
    let sha256 = raw.meta.sha256.0;
    tx.execute(
        "INSERT INTO raw (sha256, version, caps, path, file_name, name, author, mod_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT (sha256) DO NOTHING",
        params![
            sha256.as_slice(),
            raw.version,
            json_of(&raw.caps),
            imported.path,
            imported.file_name,
            imported.name,
            imported.author,
            imported.mod_version,
        ],
    )?;
    let load_id: Option<i64> = match &imported.snapshots {
        Some(snapshots) => {
            let load = &snapshots.load;
            let key = load_key(raw, load.program(), load.inputs(), load.settings());
            tx.execute(
                "INSERT INTO load (key, sha256, json) VALUES (?1, ?2, ?3)
                 ON CONFLICT (key) DO NOTHING",
                params![key, sha256.as_slice(), pack_snapshot(load)?],
            )?;
            let query = "SELECT id FROM load WHERE key = ?1";
            Some(tx.query_row(query, [&key], |row| row.get(0))?)
        }
        None => None,
    };
    let s = imported.snapshots.as_ref();
    let packed = |snapshot: Option<Result<Vec<u8>, AppError>>| snapshot.transpose();
    tx.execute(
        "INSERT INTO run (load_id, sha256, origin, opened_from, kind, settings, schema_build,
           program_version, program_commit, package, kept, made_at, fix, facts, decide, plan,
           build, \"check\", report, view)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
           ?18, ?19)",
        params![
            load_id,
            sha256.as_slice(),
            Origin::Project.as_str(),
            path.as_str(),
            imported.kind,
            json_of(&imported.settings),
            imported.schema_build,
            imported.program_version,
            imported.program_commit,
            imported.package,
            imported.made_at,
            packed(
                s.and_then(|s| s.fix.as_ref())
                    .map(pack_snapshot::<FixSnapshot>)
            )?,
            packed(
                s.and_then(|s| s.facts.as_ref())
                    .map(pack_snapshot::<FactsSnapshot>)
            )?,
            packed(
                s.and_then(|s| s.decide.as_ref())
                    .map(pack_snapshot::<DecideSnapshot>)
            )?,
            packed(
                s.and_then(|s| s.plan.as_ref())
                    .map(pack_snapshot::<PlanSnapshot>)
            )?,
            packed(
                s.and_then(|s| s.build.as_ref())
                    .map(pack_snapshot::<BuildSnapshot>)
            )?,
            packed(
                s.and_then(|s| s.check.as_ref())
                    .map(pack_snapshot::<CheckSnapshot>)
            )?,
            pack(&imported.report)?,
            pack(&json_of(&imported.view))?,
        ],
    )?;
    let run_id = tx.last_insert_rowid();
    let mut blob = tx.prepare("INSERT INTO run_blob (run_id, id) VALUES (?1, ?2)")?;
    for id in run_ids {
        blob.execute(params![run_id, id.0.as_slice()])?;
    }
    Ok(number(run_id))
}
