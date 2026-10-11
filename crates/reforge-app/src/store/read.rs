//! What the store's rows show: the runs kept, a run's report and drawn view, and the store's
//! size for the settings page. A mod's text is cleaned here, as every view shows it.

use camino::Utf8Path;
use reforge::archive::Caps;
use reforge::model::{ContentId, clean};
use reforge::snapshot::RawCache;
use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{AppStore, ROWS_FILE, failed, number, unpack};
use crate::error::AppError;
use crate::run::{RunKind, RunSettings, RunView, StageName};
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

/// Where a kept run came from (#ADR-0096).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// A run of the app's that wrote its package, or one kept on request before the write.
    Run,
    /// A project file imported: a run with its load, its raw snapshot and their bytes.
    Project,
    /// A package's embedded evidence (#ADR-0041), opened as a run with no snapshots.
    Package,
}

impl Origin {
    /// The text the origin is kept as.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Origin::Run => "run",
            Origin::Project => "project",
            Origin::Package => "package",
        }
    }

    /// The origin from the text it is kept as.
    pub(super) fn of(text: &str) -> Result<Self, AppError> {
        match text {
            "run" => Ok(Origin::Run),
            "project" => Ok(Origin::Project),
            "package" => Ok(Origin::Package),
            other => Err(failed(format!("a run of the origin {other:?}"))),
        }
    }
}

/// A run the store keeps, as the Runs page lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct RunRow {
    /// Its number: the order the runs were written in.
    pub id: u64,
    /// Where it came from.
    pub origin: Origin,
    /// The path the project or the package was opened from. None for a run of the app's.
    pub opened_from: Option<String>,
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
    /// The stages it keeps a snapshot of, in stage order, `load` first. Empty for a run
    /// opened from a package.
    pub reached: Vec<StageName>,
    /// Whether the ingest tier holds the archive's raw snapshot. A run whose mod was cleared
    /// from it is read-only until the archive is read again (#ADR-0096). False for a run
    /// opened from a package, which never had one.
    pub archive_ingested: bool,
}

/// What a kept run was run on and with: what the run page takes to run it again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct RunAgain {
    /// A repair or a port.
    pub kind: RunKind,
    /// The path the archive was last read from, as the person picked it: not cleaned, since
    /// the run reads the file at it.
    pub archive: String,
    /// The settings the request gave.
    pub settings: RunSettings,
    /// Whether a file is at the path.
    pub archive_present: bool,
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
    /// The bytes of the blobs the ingest tier's entries name whose archive no run is of: what
    /// clearing the unused archive data frees at most, since a blob two entries share is
    /// counted in each.
    pub ingest_unused_bytes: u64,
}

impl AppStore {
    /// Every run kept, newest first.
    pub fn list(&self) -> Result<Vec<RunRow>, AppError> {
        let query = listing("ORDER BY run.id DESC");
        let rows = self.rows.lock();
        let mut statement = rows.prepare(&query)?;
        let listed = statement.query_map([], |row| Ok(run_row(row, &self.ingest)))?;
        listed.map(|row| row?).collect()
    }

    /// One kept run as the list shows it. None for a number no run has.
    pub fn row(&self, id: u64) -> Result<Option<RunRow>, AppError> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(None);
        };
        let query = listing("WHERE run.id = ?1");
        let rows = self.rows.lock();
        let found = rows.query_row(&query, [id], |row| Ok(run_row(row, &self.ingest)));
        found.optional()?.transpose()
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

    /// The `.modpkg` a kept run wrote. None for a number no run has, or a run that wrote none.
    pub fn package_of(&self, id: u64) -> Result<Option<String>, AppError> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(None);
        };
        let rows = self.rows.lock();
        let query = "SELECT package FROM run WHERE id = ?1";
        let package = rows.query_row(query, [id], |row| row.get(0)).optional()?;
        Ok(package.flatten())
    }

    /// What a kept run was run on and with. None for a number no run has.
    pub fn run_again(&self, id: u64) -> Result<Option<RunAgain>, AppError> {
        let Ok(id) = i64::try_from(id) else {
            return Ok(None);
        };
        let query = "SELECT run.kind, raw.path, run.settings
             FROM run JOIN raw ON raw.sha256 = run.sha256 WHERE run.id = ?1";
        let row: Option<(String, String, String)> = {
            let rows = self.rows.lock();
            let row = rows.query_row(query, [id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            });
            row.optional()?
        };
        let Some((kind, archive, settings)) = row else {
            return Ok(None);
        };
        let settings: RunSettings = serde_json::from_str(&settings).map_err(failed)?;
        Ok(Some(RunAgain {
            kind: kind_of(&kind)?,
            archive_present: Utf8Path::new(&archive).is_file(),
            archive,
            settings,
        }))
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
        let used = self.mods()?;
        let unused = self.ingest.entries().into_iter();
        let unused = unused.filter(|entry| !used.contains(&entry.sha256));
        let unused_bytes = unused.map(|entry| self.ingest.entry_bytes(&entry)).sum();
        let rows = self.rows.lock();
        let count = |query: &str| rows.query_row(query, [], |row| row.get(0)).map(number);
        Ok(StoreView {
            runs: count("SELECT count(*) FROM run")?,
            kept: count("SELECT count(*) FROM run WHERE kept")?,
            mods: count("SELECT count(*) FROM raw")?,
            rows_bytes: bytes_of(&self.dir.join(ROWS_FILE)),
            tier_bytes: self.tier.bytes(),
            ingest_bytes: bytes_of(self.ingest.dir()),
            ingest_unused_bytes: unused_bytes,
        })
    }

    /// The archives the runs kept are of, by SHA-256: every `raw` row's.
    pub(super) fn mods(&self) -> Result<Vec<ContentId>, AppError> {
        let rows = self.rows.lock();
        let mut statement = rows.prepare("SELECT sha256 FROM raw ORDER BY sha256")?;
        let found = statement.query_map([], |row| row.get::<_, Vec<u8>>(0))?;
        let mut mods = Vec::new();
        for sha256 in found {
            // A key of another length names no archive.
            if let Ok(id) = <[u8; 32]>::try_from(sha256?) {
                mods.push(ContentId(id));
            }
        }
        Ok(mods)
    }

    /// The bytes of the blobs the ingest tier's entries of an archive name: what clearing
    /// the mod's archive data frees at most, since a blob another entry names stays.
    pub fn mod_ingest_bytes(&self, sha256: ContentId) -> u64 {
        let entries = self.ingest.entries().into_iter();
        let of_mod = entries.filter(|entry| entry.sha256 == sha256);
        of_mod.map(|entry| self.ingest.entry_bytes(&entry)).sum()
    }
}

/// A run's kind from the text it is kept as.
pub(super) fn kind_of(text: &str) -> Result<RunKind, AppError> {
    match text {
        "repair" => Ok(RunKind::Repair),
        "port" => Ok(RunKind::Port),
        other => Err(failed(format!("a run of the kind {other:?}"))),
    }
}

/// The listing's query, ending in `tail`: its filter or its order.
fn listing(tail: &str) -> String {
    let reached = REACHED.map(|(_, column)| format!("run.{column} IS NOT NULL"));
    format!(
        "SELECT run.id, run.made_at, run.kind, run.kept, raw.sha256, raw.file_name,
           raw.name, raw.author, raw.mod_version, run.package, raw.version, raw.caps,
           run.origin, run.opened_from, run.load_id IS NOT NULL, {}
         FROM run JOIN raw ON raw.sha256 = run.sha256
         {tail}",
        reached.join(", ")
    )
}

/// A row of the listing's query.
fn run_row(row: &Row<'_>, ingest: &RawCache) -> Result<RunRow, AppError> {
    let shown = |text: Option<String>| text.map(|text| clean(&text).into_owned());
    let kind = kind_of(&row.get::<_, String>(2)?)?;
    let origin = Origin::of(&row.get::<_, String>(12)?)?;
    let sha256: Vec<u8> = row.get(4)?;
    let mut reached = Vec::new();
    if row.get::<_, bool>(14)? {
        reached.push(StageName::Load);
    }
    for (at, (stage, _)) in REACHED.iter().enumerate() {
        if row.get::<_, bool>(15 + at)? {
            reached.push(*stage);
        }
    }
    let version: u32 = row.get(10)?;
    let caps = match origin {
        // A package's raw row names no reading: its archive was never read here.
        Origin::Package => None,
        Origin::Run | Origin::Project => {
            let caps = serde_json::from_str::<Caps>(&row.get::<_, String>(11)?);
            Some(caps.map_err(failed)?)
        }
    };
    // The entry's file, not its snapshot: a listing reads no snapshot.
    let archive_ingested = caps.is_some_and(|caps| {
        <[u8; 32]>::try_from(sha256.as_slice())
            .is_ok_and(|id| ingest.entry(ContentId(id), version, &caps).is_file())
    });
    Ok(RunRow {
        archive_ingested,
        id: number(row.get(0)?),
        origin,
        opened_from: shown(row.get(13)?),
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
