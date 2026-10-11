//! The kept runs a rerun resumes from (#ADR-0094, #ADR-0096): the runs of one load, by its
//! key, each with the snapshots its row holds.

use reforge::snapshot::Snapshots;

use super::files::read_snapshot;
use super::read::kind_of;
use super::{AppStore, number};
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
    /// The runs of the load with this key ([`super::load_key`]), newest first, each with its
    /// snapshots unpacked.
    pub fn kept_runs_of(&self, key: &str) -> Result<Vec<KeptRun>, AppError> {
        let query = "SELECT run.id, run.kind, load.json, run.fix, run.facts, run.decide,
               run.plan, run.build, run.\"check\"
             FROM run JOIN load ON load.id = run.load_id WHERE load.key = ?1
             ORDER BY run.id DESC";
        let columns: Vec<Columns> = {
            let rows = self.rows.lock();
            let mut statement = rows.prepare(query)?;
            let found = statement.query_map([key], |row| {
                let later = [3, 4, 5, 6, 7, 8].map(|at| row.get(at));
                let [fix, facts, decide, plan, build, check] = later;
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    [fix?, facts?, decide?, plan?, build?, check?],
                ))
            })?;
            found.collect::<Result<_, _>>()?
        };
        columns.into_iter().map(kept_run).collect()
    }
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
