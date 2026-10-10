//! A change of the runs kept, and the sweep after it (#ADR-0096). A run is removed with its
//! `run_blob` rows; a load no run refers to goes, and a raw row no load refers to, in the
//! transaction of the change. After the commit, the run tier's blobs no `run_blob` row lists
//! are removed and the tier collects its space. The ingest tier is not swept: it is cleared by
//! mod.

use std::collections::BTreeSet;

use reforge::model::ContentId;
use rusqlite::{Connection, ToSql, Transaction, params};

use super::{AppStore, RunTier};
use crate::error::AppError;

impl AppStore {
    /// Marks a run kept, which the bound does not remove, or unmarks it.
    pub fn keep(&self, id: u64, kept: bool) -> Result<(), AppError> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(());
        };
        self.change(|_, tx| {
            tx.execute("UPDATE run SET kept = ?1 WHERE id = ?2", params![kept, id])?;
            Ok(((), 0))
        })
    }

    /// Removes a run.
    pub fn remove_run(&self, id: u64) -> Result<(), AppError> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(());
        };
        self.change(|_, tx| Ok(((), remove_runs(tx, "id = ?1", &[&id])?)))
    }

    /// Removes every run of a mod: every run whose load is of the archive with this SHA-256.
    pub fn remove_mod(&self, sha256: ContentId) -> Result<(), AppError> {
        let of_mod = "load_id IN (SELECT id FROM load WHERE sha256 = ?1)";
        let sha256 = sha256.0.as_slice();
        self.change(|_, tx| Ok(((), remove_runs(tx, of_mod, &[&sha256])?)))
    }

    /// Removes every run.
    pub fn remove_all(&self) -> Result<(), AppError> {
        self.change(|_, tx| Ok(((), remove_runs(tx, "1", &[])?)))
    }

    /// Makes a change of the rows under the lock, in one transaction with the bound's cut and
    /// the rows' sweep, and sweeps the run tier after the commit when a run went. The change
    /// answers its value and how many runs it removed.
    pub(super) fn change<T>(
        &self,
        change: impl FnOnce(&RunTier, &Transaction<'_>) -> Result<(T, usize), AppError>,
    ) -> Result<T, AppError> {
        let mut rows = self.rows.lock();
        let tx = rows.transaction()?;
        let (value, removed) = change(&self.tier, &tx)?;
        let cut = cut(&tx, self.bound)?;
        sweep_rows(&tx)?;
        tx.commit()?;
        if removed + cut > 0 {
            sweep_tier(&self.tier, &rows)?;
        }
        Ok(value)
    }
}

/// Removes the runs a condition on `run` selects, with their `run_blob` rows, and answers how
/// many went.
fn remove_runs(
    tx: &Transaction<'_>,
    condition: &str,
    params: &[&dyn ToSql],
) -> Result<usize, AppError> {
    let blobs =
        format!("DELETE FROM run_blob WHERE run_id IN (SELECT id FROM run WHERE {condition})");
    tx.execute(&blobs, params)?;
    Ok(tx.execute(&format!("DELETE FROM run WHERE {condition}"), params)?)
}

/// Removes the runs not marked kept past the newest `bound` of them, lowest numbers first, and
/// answers how many went.
fn cut(tx: &Transaction<'_>, bound: usize) -> Result<usize, AppError> {
    let bound = i64::try_from(bound).unwrap_or(i64::MAX);
    let past = "id IN (SELECT id FROM run WHERE NOT kept ORDER BY id DESC LIMIT -1 OFFSET ?1)";
    remove_runs(tx, past, &[&bound])
}

/// Deletes every load no run refers to, then every raw row no load refers to.
fn sweep_rows(tx: &Transaction<'_>) -> Result<(), AppError> {
    tx.execute_batch(
        "DELETE FROM load WHERE id NOT IN (SELECT load_id FROM run);
         DELETE FROM raw WHERE sha256 NOT IN (SELECT sha256 FROM load);",
    )?;
    Ok(())
}

/// Removes the run tier's blobs no `run_blob` row lists, and collects the space when one went.
pub(super) fn sweep_tier(tier: &RunTier, rows: &Connection) -> Result<(), AppError> {
    let mut statement = rows.prepare("SELECT DISTINCT id FROM run_blob")?;
    let named = statement.query_map([], |row| row.get::<_, Vec<u8>>(0))?;
    let mut marked = BTreeSet::new();
    for id in named {
        // An id of another length names no blob the tier holds.
        if let Ok(id) = <[u8; 32]>::try_from(id?) {
            marked.insert(ContentId(id));
        }
    }
    let unmarked: Vec<ContentId> = tier
        .ids()?
        .into_iter()
        .filter(|id| !marked.contains(id))
        .collect();
    for &id in &unmarked {
        tier.remove(id)?;
    }
    if !unmarked.is_empty() {
        tier.collect()?;
    }
    Ok(())
}
