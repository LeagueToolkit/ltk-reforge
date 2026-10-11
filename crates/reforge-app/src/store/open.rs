//! The rows' file checked as the store opens, and removed by a reset (#ADR-0096: a store of
//! another format is rebuilt, never migrated). A file is this build's when its format is
//! [`FORMAT`] and every table [`SCHEMA`] declares has the columns it declares, by name and in
//! order: a file an earlier build made under the same number fails on a missing column
//! otherwise. Any other file is left as it is, and the store stays closed until a reset.

use camino::Utf8Path;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

use super::{FORMAT, SCHEMA};
use crate::error::AppError;
use crate::storage::bytes_of;

/// A store whose rows' file another build made, and what a reset of it loses, read from the
/// file with queries both shapes answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Outdated {
    /// The format the file says it is of. 0 when it does not say.
    pub format: u32,
    /// The runs it holds. 0 when they do not count.
    pub runs: u64,
    /// The runs a person marked kept. 0 when they do not count.
    pub kept: u64,
    /// The bytes of the rows' file.
    pub rows_bytes: u64,
    /// The bytes of the run tier, which a reset empties.
    pub tier_bytes: u64,
}

/// Why the store did not open.
#[derive(Debug)]
pub enum StoreOpen {
    /// Its rows' file is another build's.
    Outdated(Outdated),
    /// It did not open, read or write.
    Failed(AppError),
}

impl From<AppError> for StoreOpen {
    fn from(error: AppError) -> Self {
        StoreOpen::Failed(error)
    }
}

/// The store's state, as the pages show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum StoreState {
    /// It is open: runs are kept.
    Open,
    /// Its rows' file is another build's: runs are not kept until a reset.
    Outdated(Outdated),
    /// It did not open: runs are not kept.
    Failed {
        /// Why, as the OS or SQLite says it.
        detail: String,
    },
}

impl StoreState {
    /// The state of a store that did not open.
    pub fn of(error: &StoreOpen) -> Self {
        match error {
            StoreOpen::Outdated(outdated) => StoreState::Outdated(outdated.clone()),
            StoreOpen::Failed(AppError::StoreFailed { detail }) => StoreState::Failed {
                detail: detail.clone(),
            },
            StoreOpen::Failed(error) => StoreState::Failed {
                detail: error.to_string(),
            },
        }
    }
}

/// The rows' database at `path`: the one there when it is this build's, else a new one when
/// none is there. A file of another build's is left as it is, and answered with what a reset
/// of it loses, `tier` the run tier's folder.
pub(super) fn open_rows(path: &Utf8Path, tier: &Utf8Path) -> Result<Connection, StoreOpen> {
    if !path.exists() {
        let rows = Connection::open(path).map_err(AppError::from)?;
        rows.execute_batch(&format!(
            "BEGIN; {SCHEMA} INSERT INTO meta (key, value) VALUES ('format', '{FORMAT}'); COMMIT;"
        ))
        .map_err(AppError::from)?;
        return Ok(with_keys(rows)?);
    }
    let rows = Connection::open(path).map_err(AppError::from)?;
    if format_of(&rows) == Some(FORMAT) && of_schema(&rows) {
        return Ok(with_keys(rows)?);
    }
    drop(rows);
    Err(StoreOpen::Outdated(outdated(path, tier)))
}

/// The rows' format, when their `meta` reads.
fn format_of(rows: &Connection) -> Option<u32> {
    let format = rows.query_row("SELECT value FROM meta WHERE key = 'format'", [], |row| {
        row.get::<_, String>(0)
    });
    format.optional().ok().flatten()?.parse().ok()
}

/// Whether every table [`SCHEMA`] declares is in `rows` with the columns it declares, by name
/// and in order.
fn of_schema(rows: &Connection) -> bool {
    declared()
        .iter()
        .all(|(table, columns)| columns_of(rows, table).is_ok_and(|found| found == *columns))
}

/// Each table [`SCHEMA`] declares and its columns, in order: SQLite's own reading of it, in a
/// database in memory.
fn declared() -> Vec<(String, Vec<String>)> {
    let made = Connection::open_in_memory().and_then(|rows| {
        rows.execute_batch(SCHEMA)?;
        let query = "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name";
        let mut statement = rows.prepare(query)?;
        let tables = statement.query_map([], |row| row.get::<_, String>(0))?;
        let tables: Vec<String> = tables.collect::<Result<_, _>>()?;
        let columns = tables.into_iter().map(|table| {
            let columns = columns_of(&rows, &table)?;
            Ok((table, columns))
        });
        columns.collect()
    });
    made.unwrap_or_else(|error| panic!("the store's schema does not make tables: {error}"))
}

/// A table's columns by name, in order. Empty for a table `rows` lacks.
fn columns_of(rows: &Connection, table: &str) -> rusqlite::Result<Vec<String>> {
    let query = "SELECT name FROM pragma_table_info(?1) ORDER BY cid";
    let mut statement = rows.prepare(query)?;
    let columns = statement.query_map([table], |row| row.get(0))?;
    columns.collect()
}

/// The rows with their references held: a row never names a key no row holds.
pub(super) fn with_keys(rows: Connection) -> Result<Connection, AppError> {
    rows.pragma_update(None, "foreign_keys", true)?;
    Ok(rows)
}

/// What a reset of the rows' file at `path` loses, read only: what does not read counts as
/// nothing.
fn outdated(path: &Utf8Path, tier: &Utf8Path) -> Outdated {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let rows = Connection::open_with_flags(path, flags).ok();
    let count = |query: &str| {
        let rows = rows.as_ref()?;
        let counted = rows.query_row(query, [], |row| row.get::<_, Option<i64>>(0));
        u64::try_from(counted.ok()??).ok()
    };
    Outdated {
        format: rows.as_ref().and_then(format_of).unwrap_or(0),
        runs: count("SELECT COUNT(*) FROM run").unwrap_or(0),
        kept: count("SELECT SUM(kept) FROM run").unwrap_or(0),
        rows_bytes: bytes_of(path),
        tier_bytes: bytes_of(tier),
    }
}
