//! Diagnostics: what a run found about a mod or about its own inputs. One ordered log runs
//! through every stage, and a stage returns its result whatever it found.
//!
//! `docs/design/DIAGNOSTICS.md`.

mod filter;
mod log;
mod message;

use std::fmt;

use serde::{Deserialize, Serialize};

pub use self::filter::{Action, DiagnosticPolicy, Filter, FilterError, PolicyRule};
pub use self::log::{DiagId, Diagnostic, Diagnostics, Halted, Sink};
pub use self::message::{Message, Render};
use crate::model::entity::{BinId, CharId, FileId, ObjectId, UnitId, WadId};
use crate::model::{ChunkPath, Spelled};

/// What the output loses against the mod as its author saw it. It does not say what the run did
/// next (#ADR-0003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Nothing is lost: what was found or done.
    Note,
    /// The output may differ from what the author saw, or an input limits how far the mod was
    /// judged.
    Warning,
    /// Something of the mod is not in the output, or the output is known to be wrong somewhere.
    Error,
    /// There is no output.
    Fatal,
}

impl Severity {
    /// The severity's name in a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Note => "note",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Fatal => "fatal",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a diagnostic speaks to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// The run's inputs besides the archive: the install, the names, the history, the schema.
    Run,
    /// The archive as shipped: its layout, what the client loads of it, what is left behind.
    Input,
    /// The facts and the choice of target.
    Decision,
    /// The output: what the package holds, changes or loses.
    Output,
}

impl Channel {
    /// The channel's name in a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Run => "run",
            Channel::Input => "input",
            Channel::Decision => "decision",
            Channel::Output => "output",
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One step of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// The archive's bytes into memory.
    Read,
    /// The mod from the archive: WADs, files, bins, characters, units.
    Load,
    /// Values brought to what the target build reads.
    Fix,
    /// What is true of the mod.
    Facts,
    /// The choice of target.
    Decide,
    /// The units that ship, renamed and placed.
    Plan,
    /// The package.
    Build,
    /// The package checked.
    Check,
    /// The package on disk.
    Write,
}

impl Stage {
    /// The stage's name in a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Read => "read",
            Stage::Load => "load",
            Stage::Fix => "fix",
            Stage::Facts => "facts",
            Stage::Decide => "decide",
            Stage::Plan => "plan",
            Stage::Build => "build",
            Stage::Check => "check",
            Stage::Write => "write",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A diagnostic's code, `area.finding`, with what the code fixes: its channel and its severity
/// (#ADR-0008). `docs/design/CODES.md` lists every code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code {
    /// The stable name, `area.finding`.
    pub name: &'static str,
    /// What the finding speaks to.
    pub channel: Channel,
    /// What the output loses.
    pub severity: Severity,
}

impl Code {
    /// Declares a code.
    pub const fn new(name: &'static str, channel: Channel, severity: Severity) -> Self {
        Self {
            name,
            channel,
            severity,
        }
    }

    /// The thing the finding is about: the part of the name before the dot.
    pub fn area(&self) -> &'static str {
        self.name
            .split_once('.')
            .map_or(self.name, |(area, _)| area)
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

/// A series of numbered records under `docs/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Series {
    /// A change Riot made to the game.
    GameChange,
    /// How the game client behaves.
    GameBehavior,
    /// What tools, mod managers and authors' practices put in mods.
    ModArtifact,
}

/// A numbered record, as a comment or a doc cites it: `#GAME-CHANGE-0001`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Record {
    /// The series the record is in.
    pub series: Series,
    /// Its number in the series.
    pub number: u16,
}

impl Record {
    /// A game change, by its number.
    pub const fn game_change(number: u16) -> Self {
        Self {
            series: Series::GameChange,
            number,
        }
    }

    /// A game behavior, by its number.
    pub const fn game_behavior(number: u16) -> Self {
        Self {
            series: Series::GameBehavior,
            number,
        }
    }

    /// A mod artifact, by its number.
    pub const fn mod_artifact(number: u16) -> Self {
        Self {
            series: Series::ModArtifact,
            number,
        }
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let series = match self.series {
            Series::GameChange => "GAME-CHANGE",
            Series::GameBehavior => "GAME-BEHAVIOR",
            Series::ModArtifact => "MOD-ARTIFACT",
        };
        write!(f, "#{series}-{:04}", self.number)
    }
}

/// One of the run's inputs besides the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Input {
    /// The game's files on disk.
    Install,
    /// The hash tables.
    Names,
    /// The history index.
    History,
    /// The meta schema database.
    Schema,
}

/// What a diagnostic is about. A reference, not a string: a report joins it with its tables.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subject {
    /// The run as a whole, or the whole mod.
    Run,
    /// One of the run's inputs besides the archive.
    Input(Input),
    /// An entry of the archive, by the name the archive spells.
    Entry(Spelled),
    /// A WAD of the mod.
    Wad(WadId),
    /// A file of the mod.
    File(FileId),
    /// A loaded bin.
    Bin(BinId),
    /// An object of a loaded bin.
    Object(ObjectId),
    /// A character.
    Character(CharId),
    /// A skin unit.
    Unit(UnitId),
    /// A path of the output.
    Chunk(ChunkPath),
}

impl Subject {
    /// The kind's name in a report.
    pub fn kind(&self) -> &'static str {
        match self {
            Subject::Run => "run",
            Subject::Input(_) => "input",
            Subject::Entry(_) => "entry",
            Subject::Wad(_) => "wad",
            Subject::File(_) => "file",
            Subject::Bin(_) => "bin",
            Subject::Object(_) => "object",
            Subject::Character(_) => "character",
            Subject::Unit(_) => "unit",
            Subject::Chunk(_) => "chunk",
        }
    }

    /// The printed id a report's tables are joined on: `%u0` for a unit, the name for the rest.
    pub fn id(&self) -> String {
        match self {
            Subject::Run => "run".to_string(),
            Subject::Input(input) => format!("{input:?}").to_ascii_lowercase(),
            Subject::Entry(name) => name.raw().to_string(),
            Subject::Wad(id) => id.to_string(),
            Subject::File(id) => id.to_string(),
            Subject::Bin(id) => id.to_string(),
            Subject::Object(id) => id.to_string(),
            Subject::Character(id) => id.to_string(),
            Subject::Unit(id) => id.to_string(),
            Subject::Chunk(path) => path.to_string(),
        }
    }
}

/// A finding: a typed value declared by the module that raises it. Its type fixes its code, and
/// its fields are the report's data.
pub trait Diag: Serialize + Send + Sync + 'static {
    /// The code, with its channel and severity.
    const CODE: Code;

    /// The record the finding rests on, when every finding of the type rests on one. A fixup's
    /// own record, given to its sink, comes first.
    const RECORD: Option<Record> = None;

    /// What the finding is about.
    fn subject(&self) -> Subject;

    /// One line for a person, written from the fields.
    fn message(&self, f: &mut Message<'_>);
}
