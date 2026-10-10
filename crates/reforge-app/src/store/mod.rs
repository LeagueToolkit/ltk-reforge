//! The app's store (#ADR-0096): the runs it keeps, their snapshots and the bytes they name, in
//! one folder of the app's data folder.
//!
//! - `ingest/`: the ingest tier, the library's raw cache (#ADR-0095), shared with the command.
//! - `run-tier/`: the run tier, what runs make ([`RunTier`]).
//! - `runs.sqlite3`: the rows, three groups after #ADR-0094's, each naming the one before it
//!   by key.
//!
//! The rows and the run tier are behind one lock, so a sweep never runs while a save flushes.
//! A rows file of another format is moved aside and a new one made, never migrated in place.

mod tier;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use parking_lot::Mutex;
use reforge::snapshot::RawCache;
use rusqlite::{Connection, OptionalExtension};

pub use self::tier::{KEYSPACE, RunTier};
use crate::error::AppError;

/// The format of the rows: one number for the shape of the three groups, raised when it
/// changes (#ADR-0096).
pub const FORMAT: u32 = 1;

/// The rows' file, in the store's folder.
pub const ROWS_FILE: &str = "runs.sqlite3";

/// The ingest tier's folder, in the store's folder.
pub const INGEST_DIR: &str = "ingest";

/// The run tier's folder, in the store's folder.
pub const RUN_TIER_DIR: &str = "run-tier";

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
  report BLOB NOT NULL
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
    ingest: RawCache,
    tier: RunTier,
    #[expect(dead_code, reason = "the save and the reads land next")]
    rows: Mutex<Connection>,
}

impl AppStore {
    /// The store in `dir`, its folder and tables made when absent. A rows file of another
    /// format, or one whose format does not read, is moved aside and a new one made; the
    /// tiers stay, and the first sweep removes the run tier's blobs no row names.
    pub fn open(dir: &Utf8Path) -> Result<Self, AppError> {
        fs::create_dir_all(dir).map_err(failed)?;
        let ingest = RawCache::new(dir.join(INGEST_DIR));
        let tier = RunTier::open(dir.join(RUN_TIER_DIR))?;
        let rows = open_rows(&dir.join(ROWS_FILE))?;
        Ok(Self {
            ingest,
            tier,
            rows: Mutex::new(rows),
        })
    }

    /// The ingest tier: the raw snapshots of archives and their bytes (#ADR-0095).
    pub fn ingest(&self) -> &RawCache {
        &self.ingest
    }

    /// The run tier: what runs make.
    pub fn tier(&self) -> &RunTier {
        &self.tier
    }
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
