//! A batch (#ADR-0091): the archives of a folder, or files dropped, run as one job with one set
//! of settings, each through `write` as its own run. The library runs one archive per `Run`
//! and the app holds one environment across runs, so a batch is a loop here: it adds no rule
//! about a mod (#ADR-0046), and each row is one run's answer.

use std::sync::Arc;
use std::time::Instant;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use parking_lot::Mutex;
use reforge::archive::file_name_of;
use reforge::diag::Cancel;
use reforge::model::clean;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::inputs::{Choice, Inputs};
use crate::run::{
    self, OriginView, RunKind, RunRequest, RunSettings, RunView, StageName, StageState, Watch,
};
use crate::settings::PackageFormat;
use crate::state::OpenInputs;
use crate::store::AppStore;
use crate::view::{Finding, Severity};

/// The extensions a mod archive is listed by, compared without case.
const ARCHIVE_EXTENSIONS: [&str; 2] = ["fantome", "zip"];

/// How many archives run against one opening of the inputs. The caches of an environment grow
/// with every mod, and this is the plain bound on that growth until it is measured.
pub const REOPEN_EVERY: usize = 16;

/// Whether a path is a mod archive by its extension.
fn is_archive(path: &Utf8Path) -> bool {
    path.extension().is_some_and(|extension| {
        ARCHIVE_EXTENSIONS
            .iter()
            .any(|archive| extension.eq_ignore_ascii_case(archive))
    })
}

/// The mod archives of a folder, by extension, sorted by name. Its subfolders are not read, and
/// a name that is not UTF-8 is left out: a run takes its archive by a path as text.
pub fn list_archives(folder: &Utf8Path) -> Result<Vec<String>, AppError> {
    let unreadable = |error: std::io::Error| AppError::FolderUnreadable {
        path: folder.to_string(),
        detail: error.to_string(),
    };
    let mut archives = Vec::new();
    for entry in fs::read_dir(folder).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if is_archive(&path) && path.is_file() {
            archives.push(path.into_string());
        }
    }
    archives.sort();
    Ok(archives)
}

/// What the front end asks a batch to do. Every archive runs through the stage asked,
/// `write` unless another is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct BatchRequest {
    /// A repair or a port, of every archive.
    #[serde(default)]
    pub kind: RunKind,
    /// The last stage every archive runs through. `write` writes a package per archive; an
    /// earlier stage, `decide` for a scan of the mods' origins (#ADR-0107), writes none and
    /// keeps no run.
    #[serde(default = "write_stage")]
    pub through: StageName,
    /// The archives, in the order they run.
    pub archives: Vec<String>,
    /// The settings of every run.
    pub settings: RunSettings,
    /// The folder every package is written in. None writes each beside its archive.
    pub out: Option<String>,
    /// The form every package is written in (#ADR-0105).
    #[serde(default)]
    pub format: PackageFormat,
}

/// The stage a batch runs through unless asked otherwise.
fn write_stage() -> StageName {
    StageName::Write
}

/// How one archive's run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Outcome {
    /// The package was written.
    Written,
    /// The run ran through the stage asked, before `write`, with nothing written.
    Scanned,
    /// A finding the settings block refused the write.
    Refused,
    /// A fatal finding ended the run after `read`.
    Halted {
        /// The finding's code.
        code: String,
    },
    /// The batch's cancel ended the run.
    Cancelled {
        /// The stage it ended.
        stage: StageName,
    },
    /// An input did not answer, or the run did not start.
    Fault {
        /// Why, as text from outside the app.
        detail: String,
    },
    /// The archive does not read: a fatal finding ended `read`.
    Unreadable,
}

/// The findings of one run by severity: its own, not the inputs'.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct SeverityCounts {
    /// Fatal findings.
    pub fatal: u32,
    /// Errors.
    pub error: u32,
    /// Warnings.
    pub warning: u32,
    /// Notes.
    pub note: u32,
}

impl SeverityCounts {
    fn add(&mut self, severity: Severity) {
        let count = match severity {
            Severity::Fatal => &mut self.fatal,
            Severity::Error => &mut self.error,
            Severity::Warning => &mut self.warning,
            Severity::Note => &mut self.note,
        };
        *count = count.saturating_add(1);
    }
}

/// One unit of a port and the slot it went to, for the batch's summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BatchUnit {
    /// The character's name.
    pub character: String,
    /// The slot the mod ships the unit at.
    pub source: u16,
    /// The slot it went to. None: left to the game.
    pub chosen: Option<u16>,
    /// Where its work comes from.
    pub origin: OriginView,
}

/// One archive of a batch and how its run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BatchRow {
    /// Its place in the request, from 0.
    pub index: usize,
    /// The archive's file name.
    pub archive: String,
    /// The mod's name, as its archive says, once it was read.
    pub name: Option<String>,
    /// The mod's author, as its archive says.
    pub author: Option<String>,
    /// The mod's version, as its archive says.
    pub version: Option<String>,
    /// A port's units and where each went. Empty for a repair, and for a run that decided
    /// nothing.
    pub units: Vec<BatchUnit>,
    /// How the run ended.
    pub outcome: Outcome,
    /// The run's number in the app's store, once a write kept it there (#ADR-0096).
    pub saved: Option<u64>,
    /// The package written.
    pub package: Option<String>,
    /// The run's findings by severity.
    pub counts: SeverityCounts,
    /// How long the run took, in milliseconds. Shown, never written to a report.
    pub millis: u64,
}

/// A batch as it ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BatchView {
    /// A row per archive that ran, in order.
    pub rows: Vec<BatchRow>,
    /// How long the batch took, in milliseconds.
    pub millis: u64,
    /// Whether a cancel stopped the batch before its last archive ran to its end.
    pub stopped: bool,
}

/// An archive of the batch under way starting, or ending with its row.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct BatchProgress {
    /// The archive's place in the request, from 0.
    pub index: usize,
    /// How many archives the request holds.
    pub total: usize,
    /// The archive's file name.
    pub archive: String,
    /// Its row, once its run ended.
    pub row: Option<BatchRow>,
}

/// The inputs a batch runs against: opened once, and let go of and opened again after every
/// [`REOPEN_EVERY`] archives.
pub trait BatchInputs {
    /// The inputs, opened when none are open.
    fn open(&self) -> Result<Arc<Inputs>, AppError>;

    /// Lets go of the inputs open, so the next [`BatchInputs::open`] opens them again.
    fn forget(&self);
}

/// The inputs the settings choose, as the app holds them open.
pub struct Chosen<'a> {
    /// What holds them open.
    pub open: &'a OpenInputs,
    /// The settings' choice.
    pub choice: &'a Choice,
}

impl BatchInputs for Chosen<'_> {
    fn open(&self) -> Result<Arc<Inputs>, AppError> {
        self.open.get(self.choice)
    }

    fn forget(&self) {
        self.open.forget();
    }
}

/// A watch that counts the findings of a run by severity and forwards nothing: the window
/// hears a batch through [`BatchProgress`].
#[derive(Default)]
struct Counting(Arc<Mutex<SeverityCounts>>);

impl Watch for Counting {
    fn findings(&mut self) -> Option<Box<dyn FnMut(Finding) + Send>> {
        let counts = Arc::clone(&self.0);
        Some(Box::new(move |finding| counts.lock().add(finding.severity)))
    }
}

/// The archive's file name, cleaned: a mod's file is named by whoever made it.
fn file_name(archive: &str) -> String {
    clean(file_name_of(Utf8Path::new(archive))).into_owned()
}

/// The folder a package is written in: the request's, else the archive's own.
fn out_of(request: &BatchRequest, archive: &str) -> String {
    if let Some(out) = &request.out {
        return out.clone();
    }
    let parent = Utf8Path::new(archive).parent().map(Utf8Path::as_str);
    match parent {
        Some(parent) if !parent.is_empty() => parent.to_string(),
        _ => ".".to_string(),
    }
}

/// How a run ended, from its view.
fn outcome_of(view: &RunView, through: StageName) -> Outcome {
    if view.written.is_some() {
        return Outcome::Written;
    }
    if through != StageName::Write && run::reached(view, through) {
        return Outcome::Scanned;
    }
    if let Some(stage) = view.cancelled {
        return Outcome::Cancelled { stage };
    }
    if let Some(detail) = &view.fault {
        return Outcome::Fault {
            detail: detail.clone(),
        };
    }
    if view.refused {
        return Outcome::Refused;
    }
    if let Some(halted) = view.halted {
        let read = view
            .stages
            .iter()
            .find(|stage| stage.stage == StageName::Read);
        if read.is_some_and(|read| read.state == StageState::Halted) {
            return Outcome::Unreadable;
        }
        let finding = view.findings.iter().find(|finding| finding.id == halted);
        let code = finding.map(|finding| finding.code.clone());
        return Outcome::Halted {
            code: code.unwrap_or_default(),
        };
    }
    Outcome::Fault {
        detail: "the run wrote no package".to_string(),
    }
}

fn millis_since(at: Instant) -> u64 {
    u64::try_from(at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// What one archive of a batch runs with, beside the archive.
struct Batch<'a> {
    request: &'a BatchRequest,
    store: Option<&'a AppStore>,
    cancel: &'a Cancel,
}

impl Batch<'_> {
    /// Runs one archive through the stage asked, and answers its row. A run that does not
    /// start is the row's fault.
    fn run_one(&self, inputs: &Inputs, index: usize, archive: &str) -> BatchRow {
        let through = self.request.through;
        let writes = through == StageName::Write;
        let request = RunRequest {
            kind: self.request.kind,
            archive: archive.to_string(),
            through,
            settings: self.request.settings.clone(),
            out: writes.then(|| out_of(self.request, archive)),
            format: self.request.format,
            keep: false,
        };
        let mut watch = Counting::default();
        let at = Instant::now();
        let ran = run::run(inputs, &request, self.store, &mut watch, Some(self.cancel));
        let millis = millis_since(at);
        let counts = *watch.0.lock();
        match ran {
            Ok(view) => {
                let units = view.targets.as_ref().map_or_else(Vec::new, |targets| {
                    targets
                        .decisions
                        .iter()
                        .map(|decision| BatchUnit {
                            character: decision.character_name.clone(),
                            source: decision.source,
                            chosen: decision.chosen,
                            origin: decision.origin.clone(),
                        })
                        .collect()
                });
                let meta = view.meta.as_ref();
                BatchRow {
                    index,
                    archive: file_name(archive),
                    outcome: outcome_of(&view, through),
                    name: meta.and_then(|meta| meta.name.clone()),
                    author: meta.and_then(|meta| meta.author.clone()),
                    version: meta.and_then(|meta| meta.version.clone()),
                    units,
                    saved: view.saved,
                    package: view.written,
                    counts,
                    millis,
                }
            }
            Err(error) => BatchRow {
                counts,
                millis,
                ..fault_row(index, archive, &error)
            },
        }
    }
}

/// The row of an archive whose run did not start.
fn fault_row(index: usize, archive: &str, error: &AppError) -> BatchRow {
    BatchRow {
        index,
        archive: file_name(archive),
        name: None,
        author: None,
        version: None,
        units: Vec::new(),
        outcome: Outcome::Fault {
            detail: error.to_string(),
        },
        saved: None,
        package: None,
        counts: SeverityCounts::default(),
        millis: 0,
    }
}

/// Runs every archive of a request in order, each through `write` as its own run, and tells
/// `progress` as each starts and ends. With a store, a run that wrote its package is kept.
///
/// Settings the library does not take refuse the batch before it starts. A fault in one
/// archive is that row's outcome, and the batch goes on. A `cancel` set ends the archive under
/// way after its step (#ADR-0097) and the batch after that archive, with the rows so far.
pub fn run_batch(
    inputs: &dyn BatchInputs,
    request: &BatchRequest,
    store: Option<&AppStore>,
    cancel: &Cancel,
    progress: &mut dyn FnMut(BatchProgress),
) -> Result<BatchView, AppError> {
    request.settings.validate()?;
    let started = Instant::now();
    let batch = Batch {
        request,
        store,
        cancel,
    };
    let total = request.archives.len();
    let mut held = Some(inputs.open()?);
    let mut rows = Vec::new();
    let mut stopped = false;
    for (index, archive) in request.archives.iter().enumerate() {
        if index > 0 && index % REOPEN_EVERY == 0 {
            held = None;
            inputs.forget();
        }
        progress(BatchProgress {
            index,
            total,
            archive: file_name(archive),
            row: None,
        });
        let opened = match held.take() {
            Some(open) => Ok(open),
            None => inputs.open(),
        };
        // Inputs that do not open again fault this archive alone; the next tries again.
        let row = match &opened {
            Ok(open) => batch.run_one(open, index, archive),
            Err(error) => fault_row(index, archive, error),
        };
        held = opened.ok();
        progress(BatchProgress {
            index,
            total,
            archive: row.archive.clone(),
            row: Some(row.clone()),
        });
        let cancelled = matches!(row.outcome, Outcome::Cancelled { .. });
        rows.push(row);
        if cancel.is_set() {
            stopped = cancelled || index + 1 < total;
            break;
        }
    }
    Ok(BatchView {
        rows,
        millis: millis_since(started),
        stopped,
    })
}
