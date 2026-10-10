//! Facts per blob beside the store (#ADR-0100): a blob's size, and its kind stamped with the
//! generation of the reader that wrote it. A row's kind is text the table reads nothing into,
//! so the table knows ids and rows alone.
//!
//! On disk the table is one SQLite file in the ingest directory, apart from `cacache`'s files.
//! Any number of processes read and write it. In WAL mode a read does not wait on a write, and
//! a read the file does not answer at once answers nothing, so the blob's head is read
//! instead. A run writes the rows it learnt in one transaction at the end of `load`, so runs
//! side by side contend once each; a write that does not get the lock within the busy timeout
//! is tried again, and one that never does is logged and loses its rows, which the next run
//! reads again.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError, RwLock};
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, TransactionBehavior, params};

use crate::model::{ContentId, FactAccess, FactRow};

/// The table's file in the ingest directory.
pub const FACTS_FILE: &str = "facts.sqlite3";

/// How long a write waits for another process's write before it is tried again.
const BUSY: Duration = Duration::from_secs(5);

/// How many times a write is tried. With [`BUSY`] it waits 40 s at most, well past the few
/// milliseconds one run's rows take to write.
const TRIES: usize = 8;

/// A table of facts in memory: the run's, beside a store in memory.
#[derive(Debug, Default)]
pub struct MemoryFacts {
    rows: RwLock<BTreeMap<ContentId, FactRow>>,
}

impl MemoryFacts {
    /// A table with no row.
    pub fn new() -> Self {
        Self::default()
    }
}

impl FactAccess for MemoryFacts {
    fn row(&self, id: ContentId) -> Option<FactRow> {
        let rows = self.rows.read().unwrap_or_else(PoisonError::into_inner);
        rows.get(&id).cloned()
    }

    fn keep_rows(&self, rows: &[(ContentId, FactRow)]) {
        let mut kept = self.rows.write().unwrap_or_else(PoisonError::into_inner);
        kept.extend(rows.iter().cloned());
    }
}

/// A table of facts in a file, which processes share.
#[derive(Debug)]
pub struct FactTable {
    path: Utf8PathBuf,
    read: Mutex<Option<Connection>>,
    write: Mutex<Option<Connection>>,
}

impl FactTable {
    /// The table in the file [`FACTS_FILE`] of a directory, made at the first write when
    /// absent.
    pub fn in_dir(dir: &Utf8Path) -> Self {
        Self {
            path: dir.join(FACTS_FILE),
            read: Mutex::new(None),
            write: Mutex::new(None),
        }
    }

    /// The file it keeps its rows in.
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// How many rows it holds.
    pub fn len(&self) -> usize {
        self.read_row(|db| db.query_row("SELECT count(*) FROM blob_facts", [], |row| row.get(0)))
            .map_or(0, |count: i64| usize::try_from(count).unwrap_or(0))
    }

    /// Whether it holds no row.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Asks the file through the reading connection, opened once the file exists. A file
    /// absent or busy answers nothing.
    fn read_row<T>(&self, ask: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Option<T> {
        let mut read = self.read.lock().unwrap_or_else(PoisonError::into_inner);
        if read.is_none() && self.path.is_file() {
            let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
            *read = Connection::open_with_flags(&self.path, flags).ok();
        }
        ask(read.as_ref()?).ok()
    }

    /// Writes rows in one transaction, through the writing connection, opened and the table
    /// made at the first write.
    fn write_rows(&self, rows: &[(ContentId, FactRow)]) -> rusqlite::Result<()> {
        let mut write = self.write.lock().unwrap_or_else(PoisonError::into_inner);
        let db = match &mut *write {
            Some(db) => db,
            None => write.insert(self.open_for_writes()?),
        };
        let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        {
            let mut insert = transaction.prepare_cached(
                "INSERT OR REPLACE INTO blob_facts (id, size, kind, generation) \
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (id, row) in rows {
                let size = row.size.cast_signed();
                insert.execute(params![&id.0[..], size, row.kind, row.generation])?;
            }
        }
        transaction.commit()
    }

    fn open_for_writes(&self) -> rusqlite::Result<Connection> {
        if let Some(dir) = self.path.parent() {
            let _ = fs_err::create_dir_all(dir);
        }
        let db = Connection::open(&self.path)?;
        db.busy_timeout(BUSY)?;
        db.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS blob_facts (\
                 id BLOB PRIMARY KEY NOT NULL, \
                 size INTEGER NOT NULL, \
                 kind TEXT NOT NULL, \
                 generation INTEGER NOT NULL\
             ) WITHOUT ROWID",
            [],
        )?;
        Ok(db)
    }
}

/// Whether an error is another connection's lock, which a later try may not meet.
fn busy(error: &rusqlite::Error) -> bool {
    let code = error.sqlite_error_code();
    matches!(
        code,
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

impl FactAccess for FactTable {
    fn row(&self, id: ContentId) -> Option<FactRow> {
        self.read_row(|db| {
            let mut select =
                db.prepare_cached("SELECT size, kind, generation FROM blob_facts WHERE id = ?1")?;
            let row = select.query_row(params![&id.0[..]], |row| {
                Ok(FactRow {
                    size: row.get::<_, i64>(0)?.cast_unsigned(),
                    kind: row.get(1)?,
                    generation: row.get(2)?,
                })
            });
            row.optional()
        })
        .flatten()
    }

    fn keep_rows(&self, rows: &[(ContentId, FactRow)]) {
        for _ in 0..TRIES {
            match self.write_rows(rows) {
                Ok(()) => return,
                Err(error) if busy(&error) => {}
                Err(error) => {
                    tracing::warn!(path = %self.path, %error, "the facts of blobs are not kept");
                    return;
                }
            }
        }
        tracing::warn!(path = %self.path, "the facts of blobs are not kept: the file stays locked");
    }
}
