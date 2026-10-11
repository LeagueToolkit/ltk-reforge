//! The project file (#ADR-0096): a kept run with its load, its raw snapshot and the bytes they
//! name, as one zip.
//!
//! - `project.json`, deflated: the rows of the run and its mod, each snapshot, the report and
//!   the view as their JSON, and the list of blobs.
//! - `blobs/<64 hex>`, stored: one entry per blob, the bytes under the content id the name
//!   spells. A mod's bytes are compressed textures and zstd chunks already; the JSON is where
//!   deflate pays.
//!
//! The file names bytes by content id only, never by a mod's names.

use std::collections::BTreeSet;
use std::io::{BufWriter, Write};

use camino::Utf8Path;
use fs_err as fs;
use reforge::archive::{Caps, RawSnapshot};
use reforge::model::ContentId;
use reforge::snapshot;
use reforge::store::BlobStore;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

mod import;

use super::read::Origin;
use super::{AppStore, failed, unpack};
use crate::error::AppError;

/// The extension of a project file.
pub const PROJECT_EXTENSION: &str = "reforge-project";

/// The format of a project file: one number for its shape, raised when it changes.
pub const PROJECT_FORMAT: u32 = 1;

/// The entry that holds the rows and the snapshots.
const PROJECT_JSON: &str = "project.json";

/// What the name of a blob's entry starts with.
const BLOBS: &str = "blobs/";

/// The mod's row, as the project holds it.
#[derive(Debug, Serialize, Deserialize)]
struct RawRow {
    /// The archive's SHA-256, as hex.
    sha256: String,
    /// The raw reading's version.
    version: u32,
    /// The caps the archive was read under.
    caps: Caps,
    /// The path the archive was last read from, on the machine that exported the project.
    path: String,
    file_name: String,
    name: Option<String>,
    author: Option<String>,
    mod_version: Option<String>,
}

/// The run's row, its snapshots, report and view as their JSON.
#[derive(Debug, Serialize, Deserialize)]
struct RunColumns {
    kind: String,
    settings: Value,
    schema_build: Option<String>,
    program_version: String,
    program_commit: Option<String>,
    package: Option<String>,
    origin: Origin,
    made_at: String,
    fix: Option<Value>,
    facts: Option<Value>,
    decide: Option<Value>,
    plan: Option<Value>,
    build: Option<Value>,
    check: Option<Value>,
    report: Value,
    view: Value,
}

/// A blob the project holds.
#[derive(Debug, Serialize, Deserialize)]
struct BlobRow {
    /// Its content id, as 64 lowercase hex digits.
    id: String,
    /// Its size.
    bytes: u64,
}

/// `project.json`.
#[derive(Debug, Serialize, Deserialize)]
struct ProjectJson {
    format: u32,
    raw: RawRow,
    raw_snapshot: Value,
    load: Option<Value>,
    run: RunColumns,
    blobs: Vec<BlobRow>,
}

/// The packed columns of a run's row and its mod's, as the export reads them.
struct Exported {
    raw: RawRow,
    raw_sha256: ContentId,
    load: Option<Vec<u8>>,
    run: RunColumns,
    ids: Vec<ContentId>,
}

/// A packed snapshot, report or view column as a JSON value.
fn value_of(column: &[u8]) -> Result<Value, AppError> {
    serde_json::from_str(&unpack(column)?).map_err(failed)
}

/// A packed column that may be NULL as a JSON value.
fn value_of_some(column: Option<Vec<u8>>) -> Result<Option<Value>, AppError> {
    column.as_deref().map(value_of).transpose()
}

impl AppStore {
    /// Writes a kept run as a project file at `path`, and answers the file's size. None for
    /// a number no run has.
    ///
    /// The blobs are the ids the run's `run_blob` rows list and the ids its raw snapshot
    /// names, each once, from the run tier, else the ingest tier. One neither holds refuses
    /// the export before anything is written. The file is written beside `path` under a
    /// temporary name and renamed into place when whole. A run opened from a package has no
    /// snapshots and no bytes, and is refused.
    pub fn export_project(&self, id: u64, path: &Utf8Path) -> Result<Option<u64>, AppError> {
        let Some(exported) = self.exported(id)? else {
            return Ok(None);
        };
        let raw = self.raw_snapshot_of(&exported)?;
        let mut ids: BTreeSet<ContentId> = exported.ids.iter().copied().collect();
        ids.extend(raw.ids());
        let ingest = self.ingest.store();
        let mut blobs = Vec::new();
        for &id in &ids {
            let size = self.tier.get_head(id, 0).or_else(|| ingest.get_head(id, 0));
            let size = size.ok_or(AppError::BlobMissing { id: id.hex() })?.size;
            blobs.push(BlobRow {
                id: id.hex(),
                bytes: size,
            });
        }
        let project = ProjectJson {
            format: PROJECT_FORMAT,
            raw: exported.raw,
            raw_snapshot: serde_json::from_str(&snapshot::to_json(&raw)).map_err(failed)?,
            load: value_of_some(exported.load)?,
            run: exported.run,
            blobs,
        };
        let json = serde_json::to_vec(&project).map_err(failed)?;
        self.write_project(path, &json, &ids)
    }

    /// The zip at `path`: `project.json`, then each blob in id order.
    fn write_project(
        &self,
        path: &Utf8Path,
        json: &[u8],
        ids: &BTreeSet<ContentId>,
    ) -> Result<Option<u64>, AppError> {
        let folder = path.parent().filter(|dir| !dir.as_str().is_empty());
        let folder = folder.unwrap_or(Utf8Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(folder).map_err(failed)?;
        {
            let mut zip = ZipWriter::new(BufWriter::new(temporary.as_file_mut()));
            // No time is written: an entry's time is the zip format's earliest.
            let deflated = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .large_file(true);
            let stored = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Stored)
                .large_file(true);
            zip.start_file(PROJECT_JSON, deflated).map_err(failed)?;
            zip.write_all(json).map_err(failed)?;
            for &id in ids {
                let bytes = self.tier.get(id).or_else(|| self.ingest.store().get(id));
                let bytes = bytes.ok_or(AppError::BlobMissing { id: id.hex() })?;
                zip.start_file(format!("{BLOBS}{}", id.hex()), stored)
                    .map_err(failed)?;
                zip.write_all(&bytes).map_err(failed)?;
            }
            let mut out = zip.finish().map_err(failed)?;
            out.flush().map_err(failed)?;
        }
        temporary.as_file().sync_all().map_err(failed)?;
        let written = temporary.as_file().metadata().map_err(failed)?.len();
        temporary
            .persist(path)
            .map_err(|error| failed(error.error))?;
        Ok(Some(written))
    }

    /// The raw snapshot the ingest tier holds of an exported run's archive, at the reading and
    /// caps its row names. Refused as the archive's bytes missing when the mod was cleared
    /// from the ingest tier.
    fn raw_snapshot_of(&self, exported: &Exported) -> Result<RawSnapshot, AppError> {
        let (sha256, raw) = (exported.raw_sha256, &exported.raw);
        let entry = self.ingest.entry(sha256, raw.version, &raw.caps);
        let missing = || AppError::BlobMissing { id: sha256.hex() };
        let json = fs::read_to_string(entry).map_err(|_| missing())?;
        snapshot::from_json(&json).map_err(|_| missing())
    }

    /// The columns of a kept run's row and its mod's, and the ids its `run_blob` rows list.
    /// None for a number no run has; refused for a run opened from a package.
    fn exported(&self, id: u64) -> Result<Option<Exported>, AppError> {
        let Ok(key) = i64::try_from(id) else {
            return Ok(None);
        };
        let query = "SELECT run.origin, run.kind, run.settings, run.schema_build,
               run.program_version, run.program_commit, run.package, run.made_at,
               load.json AS load, run.fix, run.facts, run.decide, run.plan, run.build,
               run.\"check\", run.report, run.view, raw.sha256, raw.version, raw.caps, raw.path,
               raw.file_name, raw.name, raw.author, raw.mod_version
             FROM run LEFT JOIN load ON load.id = run.load_id
               JOIN raw ON raw.sha256 = run.sha256
             WHERE run.id = ?1";
        let (columns, listed) = {
            let rows = self.rows.lock();
            let columns = rows.query_row(query, [key], |row| {
                let packed = |name: &str| row.get::<_, Option<Vec<u8>>>(name);
                Ok(Columns {
                    origin: row.get("origin")?,
                    kind: row.get("kind")?,
                    settings: row.get("settings")?,
                    schema_build: row.get("schema_build")?,
                    program_version: row.get("program_version")?,
                    program_commit: row.get("program_commit")?,
                    package: row.get("package")?,
                    made_at: row.get("made_at")?,
                    load: packed("load")?,
                    later: [
                        packed("fix")?,
                        packed("facts")?,
                        packed("decide")?,
                        packed("plan")?,
                        packed("build")?,
                        packed("check")?,
                    ],
                    report: row.get("report")?,
                    view: row.get("view")?,
                    sha256: row.get("sha256")?,
                    version: row.get("version")?,
                    caps: row.get("caps")?,
                    path: row.get("path")?,
                    file_name: row.get("file_name")?,
                    name: row.get("name")?,
                    author: row.get("author")?,
                    mod_version: row.get("mod_version")?,
                })
            });
            let columns = columns.optional()?;
            let mut statement = rows.prepare("SELECT id FROM run_blob WHERE run_id = ?1")?;
            let listed = statement.query_map([key], |row| row.get::<_, Vec<u8>>(0))?;
            (columns, listed.collect::<Result<Vec<_>, _>>()?)
        };
        let Some(c) = columns else {
            return Ok(None);
        };
        let origin = Origin::of(&c.origin)?;
        if origin == Origin::Package {
            return Err(AppError::NoProjectOf { id });
        }
        let raw_sha256 = <[u8; 32]>::try_from(c.sha256.as_slice())
            .map(ContentId)
            .map_err(|_| failed("a raw row whose key is no SHA-256"))?;
        // An id of another length names no blob.
        let ids = listed
            .iter()
            .filter_map(|id| <[u8; 32]>::try_from(id.as_slice()).ok());
        let [fix, facts, decide, plan, build, check] = c.later;
        Ok(Some(Exported {
            raw: RawRow {
                sha256: raw_sha256.hex(),
                version: c.version,
                caps: serde_json::from_str(&c.caps).map_err(failed)?,
                path: c.path,
                file_name: c.file_name,
                name: c.name,
                author: c.author,
                mod_version: c.mod_version,
            },
            raw_sha256,
            load: c.load,
            run: RunColumns {
                kind: c.kind,
                settings: serde_json::from_str(&c.settings).map_err(failed)?,
                schema_build: c.schema_build,
                program_version: c.program_version,
                program_commit: c.program_commit,
                package: c.package,
                origin,
                made_at: c.made_at,
                fix: value_of_some(fix)?,
                facts: value_of_some(facts)?,
                decide: value_of_some(decide)?,
                plan: value_of_some(plan)?,
                build: value_of_some(build)?,
                check: value_of_some(check)?,
                report: value_of(&c.report)?,
                view: value_of(&c.view)?,
            },
            ids: ids.map(ContentId).collect(),
        }))
    }
}

/// The columns of a run's row and its mod's, as SQLite gives them.
struct Columns {
    origin: String,
    kind: String,
    settings: String,
    schema_build: Option<String>,
    program_version: String,
    program_commit: Option<String>,
    package: Option<String>,
    made_at: String,
    load: Option<Vec<u8>>,
    later: [Option<Vec<u8>>; 6],
    report: Vec<u8>,
    view: Vec<u8>,
    sha256: Vec<u8>,
    version: u32,
    caps: String,
    path: String,
    file_name: String,
    name: Option<String>,
    author: Option<String>,
    mod_version: Option<String>,
}
