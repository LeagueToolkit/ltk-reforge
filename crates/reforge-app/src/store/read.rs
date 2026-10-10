//! What the store's rows show: the runs kept, a run's report and drawn view, and the store's
//! size for the settings page. A mod's text is cleaned here, as every view shows it.

use reforge::model::clean;
use rusqlite::{OptionalExtension, Row};
use serde::Serialize;

use super::{AppStore, ROWS_FILE, failed, number, unpack};
use crate::error::AppError;
use crate::run::{RunKind, RunView, StageName};
use crate::storage::bytes_of;
use crate::view::clean_strings;

/// The stages after `load` a run row keeps a snapshot of, in stage order, each with its
/// column.
const REACHED: [(StageName, &str); 6] = [
    (StageName::Fix, "fix"),
    (StageName::Facts, "facts"),
    (StageName::Decide, "decide"),
    (StageName::Plan, "plan"),
    (StageName::Build, "build"),
    (StageName::Check, "\"check\""),
];

/// A run the store keeps, as the Runs page lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct RunRow {
    /// Its number: the order the runs were written in.
    pub id: u64,
    /// When the store wrote it, by the store's clock. Shown, never part of a report.
    pub made_at: String,
    /// A repair or a port.
    pub kind: RunKind,
    /// Whether a person marked it kept, so that the bound does not remove it.
    pub kept: bool,
    /// The archive's SHA-256, as hex.
    pub sha256: String,
    /// The archive's file name.
    pub file_name: String,
    /// The mod's name, as its archive says.
    pub name: Option<String>,
    /// The mod's author, as its archive says.
    pub author: Option<String>,
    /// The mod's version, as its archive says.
    pub version: Option<String>,
    /// The `.modpkg` written.
    pub package: Option<String>,
    /// The stages it keeps a snapshot of, in stage order, `load` first.
    pub reached: Vec<StageName>,
}

/// What the store holds, as the settings page's storage group shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct StoreView {
    /// The runs kept.
    pub runs: u64,
    /// The runs marked kept.
    pub kept: u64,
    /// The mods the runs are of.
    pub mods: u64,
    /// The bytes of the rows' file.
    pub rows_bytes: u64,
    /// The bytes of the run tier.
    pub tier_bytes: u64,
    /// The bytes of the ingest tier.
    pub ingest_bytes: u64,
}

impl AppStore {
    /// Every run kept, newest first.
    pub fn list(&self) -> Result<Vec<RunRow>, AppError> {
        let reached = REACHED.map(|(_, column)| format!("run.{column} IS NOT NULL"));
        let query = format!(
            "SELECT run.id, run.made_at, run.kind, run.kept, raw.sha256, raw.file_name,
               raw.name, raw.author, raw.mod_version, run.package, {}
             FROM run JOIN load ON load.id = run.load_id JOIN raw ON raw.sha256 = load.sha256
             ORDER BY run.id DESC",
            reached.join(", ")
        );
        let rows = self.rows.lock();
        let mut statement = rows.prepare(&query)?;
        let listed = statement.query_map([], |row| Ok(run_row(row)))?;
        listed.map(|row| row?).collect()
    }

    /// The report of a kept run, as JSON. None for a number no run has.
    pub fn report(&self, id: u64) -> Result<Option<String>, AppError> {
        self.packed_column(id, "report")
    }

    /// What the views drew of a kept run as it ended, its number set. None for a number no
    /// run has.
    pub fn run_view(&self, id: u64) -> Result<Option<RunView>, AppError> {
        let Some(json) = self.packed_column(id, "view")? else {
            return Ok(None);
        };
        let mut value: serde_json::Value = serde_json::from_str(&json).map_err(failed)?;
        // Cleaned when it was drawn; cleaned again on the way out, as every view of a mod's
        // text is, since cleaning twice changes nothing.
        clean_strings(&mut value);
        let mut view: RunView = serde_json::from_value(value).map_err(failed)?;
        view.saved = Some(id);
        Ok(Some(view))
    }

    /// A packed column of a kept run's row, as JSON. None for a number no run has.
    pub(super) fn packed_column(&self, id: u64, column: &str) -> Result<Option<String>, AppError> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(None);
        };
        let rows = self.rows.lock();
        let query = format!("SELECT {column} FROM run WHERE id = ?1");
        let packed: Option<Vec<u8>> = rows.query_row(&query, [id], |row| row.get(0)).optional()?;
        packed.map(|packed| unpack(&packed)).transpose()
    }

    /// What the store holds and its bytes on disk.
    pub fn view(&self) -> Result<StoreView, AppError> {
        let rows = self.rows.lock();
        let count = |query: &str| rows.query_row(query, [], |row| row.get(0)).map(number);
        Ok(StoreView {
            runs: count("SELECT count(*) FROM run")?,
            kept: count("SELECT count(*) FROM run WHERE kept")?,
            mods: count("SELECT count(*) FROM raw")?,
            rows_bytes: bytes_of(&self.dir.join(ROWS_FILE)),
            tier_bytes: self.tier.bytes(),
            ingest_bytes: bytes_of(self.ingest.dir()),
        })
    }
}

/// A row of the listing's query.
fn run_row(row: &Row<'_>) -> Result<RunRow, AppError> {
    let shown = |text: Option<String>| text.map(|text| clean(&text).into_owned());
    let kind: String = row.get(2)?;
    let kind = match kind.as_str() {
        "repair" => RunKind::Repair,
        "port" => RunKind::Port,
        other => return Err(failed(format!("a run of the kind {other:?}"))),
    };
    let sha256: Vec<u8> = row.get(4)?;
    let mut reached = vec![StageName::Load];
    for (at, (stage, _)) in REACHED.iter().enumerate() {
        if row.get::<_, bool>(10 + at)? {
            reached.push(*stage);
        }
    }
    Ok(RunRow {
        id: number(row.get(0)?),
        made_at: row.get(1)?,
        kind,
        kept: row.get(3)?,
        sha256: sha256.iter().map(|byte| format!("{byte:02x}")).collect(),
        file_name: clean(&row.get::<_, String>(5)?).into_owned(),
        name: shown(row.get(6)?),
        author: shown(row.get(7)?),
        version: shown(row.get(8)?),
        // The package's file name is made from the mod's.
        package: shown(row.get(9)?),
        reached,
    })
}
