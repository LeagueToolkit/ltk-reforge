//! The kept run a rerun resumes from (#ADR-0094, #ADR-0096): the newest run of one load and
//! kind, with the snapshots its row holds; and a kept run by its number, which a read of its
//! package resumes (#ADR-0103).

use reforge::snapshot::Snapshots;
use rusqlite::OptionalExtension;

use super::files::read_snapshot;
use super::read::kind_of;
use super::{AppStore, kind_text, number};
use crate::error::AppError;
use crate::run::RunKind;

/// A run the store keeps, with its snapshots.
pub struct KeptRun {
    /// Its number.
    pub id: u64,
    /// A repair or a port.
    pub kind: RunKind,
    /// Its snapshots from `load` on.
    pub snapshots: Snapshots,
}

/// The packed columns of a run's row: its number, its kind, its load's snapshot and each
/// later stage's.
type Columns = (i64, String, Vec<u8>, [Option<Vec<u8>>; 6]);

impl AppStore {
    /// The newest run of `kind` of the load with this key ([`super::load_key`]), its
    /// snapshots unpacked. None when the store keeps none: a repair's snapshots do not resume
    /// a port.
    pub fn newest_kept_run_of(
        &self,
        key: &str,
        kind: RunKind,
    ) -> Result<Option<KeptRun>, AppError> {
        let query = "SELECT run.id, run.kind, load.json, run.fix, run.facts, run.decide,
               run.plan, run.build, run.\"check\"
             FROM run JOIN load ON load.id = run.load_id WHERE load.key = ?1 AND run.kind = ?2
             ORDER BY run.id DESC LIMIT 1";
        let columns: Option<Columns> = {
            let rows = self.rows.lock();
            let mut statement = rows.prepare(query)?;
            statement
                .query_row([key, kind_text(kind)], columns_of)
                .optional()?
        };
        columns.map(kept_run).transpose()
    }

    /// A kept run by its number, its snapshots unpacked. None for a number no run has, and
    /// for a run opened from a package, which has no snapshots.
    pub fn kept_run(&self, id: u64) -> Result<Option<KeptRun>, AppError> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(None);
        };
        let query = "SELECT run.id, run.kind, load.json, run.fix, run.facts, run.decide,
               run.plan, run.build, run.\"check\"
             FROM run JOIN load ON load.id = run.load_id WHERE run.id = ?1";
        let columns: Option<Columns> = {
            let rows = self.rows.lock();
            let mut statement = rows.prepare(query)?;
            statement.query_row([id], columns_of).optional()?
        };
        columns.map(kept_run).transpose()
    }
}

/// A run's row as [`Columns`].
fn columns_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<Columns> {
    let later = [3, 4, 5, 6, 7, 8].map(|at| row.get(at));
    let [fix, facts, decide, plan, build, check] = later;
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        [fix?, facts?, decide?, plan?, build?, check?],
    ))
}

/// A kept run from its row's columns.
fn kept_run((id, kind, load, later): Columns) -> Result<KeptRun, AppError> {
    let [fix, facts, decide, plan, build, check] = later;
    let mut snapshots = Snapshots::new(read_snapshot(&load)?);
    snapshots.fix = fix.as_deref().map(read_snapshot).transpose()?;
    snapshots.facts = facts.as_deref().map(read_snapshot).transpose()?;
    snapshots.decide = decide.as_deref().map(read_snapshot).transpose()?;
    snapshots.plan = plan.as_deref().map(read_snapshot).transpose()?;
    snapshots.build = build.as_deref().map(read_snapshot).transpose()?;
    snapshots.check = check.as_deref().map(read_snapshot).transpose()?;
    Ok(KeptRun {
        id: number(id),
        kind: kind_of(&kind)?,
        snapshots,
    })
}
