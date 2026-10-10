//! The app's store (#ADR-0096): the runs it keeps, their snapshots and the bytes they name, in
//! one folder of the app's data folder.
//!
//! - `ingest/`: the ingest tier, the library's raw cache (#ADR-0095).
//! - `run-tier/`: the run tier, what runs make ([`RunTier`]).
//! - `runs.sqlite3`: the rows, three groups after #ADR-0094's, each naming the one before it
//!   by key.
//!
//! The rows and the run tier are behind one lock, so a sweep never runs while a save flushes.
//! A rows file of another format is moved aside and a new one made, never migrated in place.
//! A save, a removal and a change of a run's mark each cut the bound and sweep what no run
//! names, in the transaction of the change.

mod files;
mod read;
mod sweep;
mod tier;
mod viewer;

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use parking_lot::Mutex;
use reforge::archive::RawSnapshot;
use reforge::model::{ByteView, ContentId, Spelled};
use reforge::report::Report;
use reforge::snapshot::{self, LoadSnapshot, RawCache, Snapshots, StageSnapshot};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

pub use self::files::{FILE_STAGES, FileRow, KeptSnapshots, Mark, StageFiles, WadFiles};
pub use self::read::{RunRow, StoreView};
pub use self::tier::{KEYSPACE, RunTier};
pub use self::viewer::{FileView, HEX_BYTES, MAX_BIN_TEXT, MAX_DECODED, MAX_TEXT};
use crate::error::AppError;
use crate::run::{RunKind, RunSettings, RunView};

/// The format of the rows: one number for the shape of the three groups, raised when it
/// changes (#ADR-0096).
pub const FORMAT: u32 = 1;

/// The rows' file, in the store's folder.
pub const ROWS_FILE: &str = "runs.sqlite3";

/// The ingest tier's folder, in the store's folder.
pub const INGEST_DIR: &str = "ingest";

/// The run tier's folder, in the store's folder.
pub const RUN_TIER_DIR: &str = "run-tier";

/// How many runs not marked kept the store keeps: past it, the lowest numbers are removed
/// (#ADR-0096). Generous and a power of two: a run is a few MB of the run tier, mostly shared
/// between reruns of one mod. A setting of the person's takes its place later.
pub const RUN_BOUND: usize = 256;

/// The zstd level of the rows' JSON: the library's default, fast to write on every save.
const ZSTD_LEVEL: i32 = 3;

/// The files SQLite keeps beside a database, which go where it goes.
const SIDE_FILES: [&str; 3] = ["-journal", "-wal", "-shm"];

/// The tables of format 1. `check` is a word of SQL's, so its column is always quoted.
const SCHEMA: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE raw (
  sha256 BLOB PRIMARY KEY,
  version INTEGER NOT NULL,
  caps TEXT NOT NULL,
  path TEXT NOT NULL,
  file_name TEXT NOT NULL,
  name TEXT,
  author TEXT,
  mod_version TEXT
);
CREATE TABLE load (
  id INTEGER PRIMARY KEY,
  key TEXT NOT NULL UNIQUE,
  sha256 BLOB NOT NULL REFERENCES raw(sha256),
  json BLOB NOT NULL
);
CREATE INDEX load_raw ON load(sha256);
CREATE TABLE run (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  load_id INTEGER NOT NULL REFERENCES load(id),
  kind TEXT NOT NULL,
  settings TEXT NOT NULL,
  schema_build TEXT,
  program_version TEXT NOT NULL,
  program_commit TEXT,
  package TEXT,
  kept INTEGER NOT NULL DEFAULT 0,
  made_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  fix BLOB, facts BLOB, decide BLOB, plan BLOB, build BLOB, \"check\" BLOB,
  report BLOB NOT NULL,
  view BLOB NOT NULL
);
CREATE INDEX run_load ON run(load_id);
CREATE TABLE run_blob (
  run_id INTEGER NOT NULL REFERENCES run(id),
  id BLOB NOT NULL,
  PRIMARY KEY (run_id, id)
);
CREATE INDEX run_blob_id ON run_blob(id);
";

/// The runs the app keeps, their snapshots and the bytes they name.
pub struct AppStore {
    dir: Utf8PathBuf,
    ingest: RawCache,
    tier: RunTier,
    rows: Mutex<Connection>,
    bound: usize,
}

/// What a run that wrote its package leaves to keep (#ADR-0096).
pub struct Saved {
    /// The raw snapshot the run started from.
    pub raw: RawSnapshot,
    /// The path the archive was read from.
    pub path: Utf8PathBuf,
    /// The snapshots from `load` on.
    pub snapshots: Snapshots,
    /// The run's report.
    pub report: Report,
    /// What the views drew of the run as it ended, its number not yet set.
    pub view: RunView,
    /// A repair or a port.
    pub kind: RunKind,
    /// The settings the request gave.
    pub settings: RunSettings,
    /// The schema database's build, as the inputs report it.
    pub schema_build: Option<String>,
    /// The `.modpkg` written.
    pub package: Option<String>,
    /// The run's memory layer: every byte its stages made, by id.
    pub layer: BTreeMap<ContentId, ByteView>,
}

impl AppStore {
    /// The store in `dir`, its folder and tables made when absent. A rows file of another
    /// format, or one whose format does not read, is moved aside and a new one made; the
    /// tiers stay. The run tier's blobs no row names are swept: a moved file's, and those of a
    /// save a crash ended between its flush and its rows.
    pub fn open(dir: &Utf8Path) -> Result<Self, AppError> {
        fs::create_dir_all(dir).map_err(failed)?;
        let ingest = RawCache::new(dir.join(INGEST_DIR));
        let tier = RunTier::open(dir.join(RUN_TIER_DIR))?;
        let rows = open_rows(&dir.join(ROWS_FILE))?;
        sweep::sweep_tier(&tier, &rows)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            ingest,
            tier,
            rows: Mutex::new(rows),
            bound: RUN_BOUND,
        })
    }

    /// The store keeping `bound` runs not marked kept in place of [`RUN_BOUND`]: what a
    /// setting of the person's gives.
    #[must_use]
    pub fn with_bound(mut self, bound: usize) -> Self {
        self.bound = bound;
        self
    }

    /// The ingest tier: the raw snapshots of archives and their bytes (#ADR-0095).
    pub fn ingest(&self) -> &RawCache {
        &self.ingest
    }

    /// The run tier: what runs make.
    pub fn tier(&self) -> &RunTier {
        &self.tier
    }

    /// Keeps a run that wrote its package, and answers its number. The layer is flushed into
    /// the run tier first and the rows written after it in one transaction, so a row never
    /// names a byte the tier lacks: the raw row (its path the last one read), the load row,
    /// shared by every run of its key, the run row and the ids of its layer. The bound is
    /// cut after it.
    pub fn save(&self, saved: Saved) -> Result<u64, AppError> {
        self.change(|tier, tx| {
            tier.flush(&saved.layer)?;
            Ok((insert_run(tx, &saved)?, 0))
        })
    }
}

/// Writes a saved run's rows, and answers its number.
fn insert_run(tx: &Transaction<'_>, saved: &Saved) -> Result<u64, AppError> {
    let (raw, load) = (&saved.raw, &saved.snapshots.load);
    let meta = &raw.meta;
    // A mod's own text is kept raw, and cleaned where a view shows it.
    let text = |spelled: &Option<Spelled>| spelled.as_ref().map(|s| s.raw().to_string());
    tx.execute(
        "INSERT INTO raw (sha256, version, caps, path, file_name, name, author, mod_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (sha256) DO UPDATE SET version = excluded.version, caps = excluded.caps,
           path = excluded.path, file_name = excluded.file_name",
        params![
            meta.sha256.0.as_slice(),
            raw.version,
            json_of(&raw.caps),
            saved.path.as_str(),
            meta.file_name.raw(),
            text(&meta.name),
            text(&meta.author),
            text(&meta.version),
        ],
    )?;
    let key = load_key(raw, load);
    tx.execute(
        "INSERT INTO load (key, sha256, json) VALUES (?1, ?2, ?3) ON CONFLICT (key) DO NOTHING",
        params![key, meta.sha256.0.as_slice(), pack_snapshot(load)?],
    )?;
    let load_id: i64 = tx.query_row("SELECT id FROM load WHERE key = ?1", [&key], |row| {
        row.get(0)
    })?;
    let s = &saved.snapshots;
    let program = load.program();
    tx.execute(
        "INSERT INTO run (load_id, kind, settings, schema_build, program_version,
           program_commit, package, fix, facts, decide, plan, build, \"check\", report, view)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            load_id,
            kind_text(saved.kind),
            json_of(&saved.settings),
            saved.schema_build,
            program.version,
            program.commit,
            saved.package,
            s.fix.as_ref().map(pack_snapshot).transpose()?,
            s.facts.as_ref().map(pack_snapshot).transpose()?,
            s.decide.as_ref().map(pack_snapshot).transpose()?,
            s.plan.as_ref().map(pack_snapshot).transpose()?,
            s.build.as_ref().map(pack_snapshot).transpose()?,
            s.check.as_ref().map(pack_snapshot).transpose()?,
            pack(&saved.report.to_json())?,
            pack(&json_of(&saved.view))?,
        ],
    )?;
    let run_id = tx.last_insert_rowid();
    let mut blob = tx.prepare("INSERT INTO run_blob (run_id, id) VALUES (?1, ?2)")?;
    for id in saved.layer.keys() {
        blob.execute(params![run_id, id.0.as_slice()])?;
    }
    Ok(number(run_id))
}

/// A run's number or a count, as SQLite gives it: never negative, since SQLite numbers rows
/// from 1.
fn number(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_else(|_| panic!("SQLite gave {value} for a number or a count"))
}

/// The key of a load (#ADR-0096): the hex of the SHA-256 over the JSON of what the snapshot
/// after `load` was made under. The raw snapshot by its archive, reading and caps; the
/// program by its version and commit; the inputs by their identity (#ADR-0094); and the
/// settings `load` declares.
fn load_key(raw: &RawSnapshot, load: &LoadSnapshot) -> String {
    let made_under = (
        load.meta().sha256.hex(),
        raw.version,
        &raw.caps,
        load.program(),
        load.inputs(),
        load.settings(),
    );
    ContentId::of(json_of(&made_under).as_bytes()).hex()
}

/// The text a run's kind is kept as.
fn kind_text(kind: RunKind) -> &'static str {
    match kind {
        RunKind::Repair => "repair",
        RunKind::Port => "port",
    }
}

/// A value of the app's or the library's as JSON. None of the values kept fails to write.
fn json_of<T: serde::Serialize + std::fmt::Debug>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|error| panic!("{value:?} does not write as JSON: {error}"))
}

/// A snapshot as a column: its JSON, as the library writes it, packed.
fn pack_snapshot<S: StageSnapshot>(snapshot: &S) -> Result<Vec<u8>, AppError> {
    pack(&snapshot::to_json(snapshot))
}

/// JSON as a column: one zstd frame.
fn pack(json: &str) -> Result<Vec<u8>, AppError> {
    zstd::encode_all(json.as_bytes(), ZSTD_LEVEL).map_err(failed)
}

/// A column as JSON.
fn unpack(column: &[u8]) -> Result<String, AppError> {
    let bytes = zstd::decode_all(column).map_err(failed)?;
    String::from_utf8(bytes).map_err(failed)
}

/// The rows' database at `path`: the one there when it is of this format, else a new one.
fn open_rows(path: &Utf8Path) -> Result<Connection, AppError> {
    if path.exists() {
        let found = Connection::open(path).ok().map(|rows| {
            let format = format_of(&rows);
            (rows, format)
        });
        match found {
            Some((rows, Some(FORMAT))) => return with_keys(rows),
            found => {
                // The file is renamed once no connection holds it.
                let format = found.and_then(|(_, format)| format);
                let suffix = format.map_or("unread".to_string(), |n| format!("format-{n}"));
                move_aside(path, &suffix)?;
            }
        }
    }
    let rows = Connection::open(path)?;
    rows.execute_batch(&format!(
        "BEGIN; {SCHEMA} INSERT INTO meta (key, value) VALUES ('format', '{FORMAT}'); COMMIT;"
    ))?;
    with_keys(rows)
}

/// The rows' format, when their `meta` reads.
fn format_of(rows: &Connection) -> Option<u32> {
    let format = rows.query_row("SELECT value FROM meta WHERE key = 'format'", [], |row| {
        row.get::<_, String>(0)
    });
    format.optional().ok().flatten()?.parse().ok()
}

/// The rows with their references held: a row never names a key no row holds.
fn with_keys(rows: Connection) -> Result<Connection, AppError> {
    rows.pragma_update(None, "foreign_keys", true)?;
    Ok(rows)
}

/// Moves a rows file, and the files SQLite keeps beside it, to `<file>.<suffix>`.
fn move_aside(path: &Utf8Path, suffix: &str) -> Result<(), AppError> {
    let aside = Utf8PathBuf::from(format!("{path}.{suffix}"));
    tracing::warn!(%path, %aside, "the store's rows are not of this format, moved aside");
    fs::rename(path, &aside).map_err(failed)?;
    for side in SIDE_FILES {
        let file = Utf8PathBuf::from(format!("{path}{side}"));
        if file.exists() {
            fs::rename(&file, format!("{aside}{side}")).map_err(failed)?;
        }
    }
    Ok(())
}

/// A failure of the store, as the front end shows it.
fn failed(error: impl std::fmt::Display) -> AppError {
    AppError::StoreFailed {
        detail: error.to_string(),
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        failed(error)
    }
}

impl From<fjall::Error> for AppError {
    fn from(error: fjall::Error) -> Self {
        failed(error)
    }
}
