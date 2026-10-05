//! The inputs a run is judged against: the game install, the hash tables, the schema database
//! and the history index (`docs/design/ROADMAP-UX.md`, "The inputs"). They are opened once and
//! shared by every run, since opening the install and the name tables takes seconds.
//!
//! The command opens them for each run, and what each one found is in the run's log, so in its
//! report and in the evidence a package carries. [`Inputs::report`] raises the same findings
//! into each run's log, in the command's order, so a run of the app reports as one of the
//! command does.

use camino::{Utf8Path, Utf8PathBuf};
use reforge::diag::{Diagnostics, Render, Sink, Stage};
use reforge::env::Env;
use reforge::game::{
    Game, Install, MimirAbsent, NameBook, NameSource, Names, NoBuild, Schema, SchemaAbsent,
    SchemaUnreadable, Speller, UnreadableWad, VersionUnknown, WadsUnreadable, mimir_cache_dir,
};
use reforge::history::{HistoryAbsent, IndexMissing, SqliteHistory};
use serde::Serialize;

use crate::error::{AppError, chain};
use crate::settings::Settings;
use crate::view::Finding;

/// The inputs as the settings choose them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Choice {
    /// The game install.
    pub game: Option<Utf8PathBuf>,
    /// The hash cache's folder, else the one the League Toolkit programs share.
    pub hashes: Option<Utf8PathBuf>,
    /// The schema database, else the one the League Toolkit programs share.
    pub schema: Option<Utf8PathBuf>,
    /// The history index's folder, else none.
    pub history: Option<Utf8PathBuf>,
}

impl From<&Settings> for Choice {
    fn from(settings: &Settings) -> Self {
        let path = |text: &Option<String>| text.as_deref().map(Utf8PathBuf::from);
        Self {
            game: path(&settings.game),
            hashes: path(&settings.hashes),
            schema: path(&settings.schema),
            history: path(&settings.history),
        }
    }
}

/// What opening one input found, kept to be raised again in each run.
enum Opened {
    /// None was chosen or found.
    Absent,
    /// The one at this path did not open, for this reason.
    Unreadable { path: Utf8PathBuf, reason: String },
    /// The one at this path opened.
    Read(Utf8PathBuf),
}

impl Opened {
    fn path(&self) -> Option<String> {
        match self {
            Opened::Read(path) => Some(path.to_string()),
            _ => None,
        }
    }
}

/// The inputs, opened.
pub struct Inputs {
    env: Env,
    root: Utf8PathBuf,
    unreadable: Vec<UnreadableWad>,
    no_build: Option<NoBuild>,
    hashes: Option<Utf8PathBuf>,
    schema: Opened,
    history: Opened,
}

impl Inputs {
    /// Opens the inputs a choice names. `names_index` is the folder the name-hash index of the
    /// hash cache is kept in, built there when absent.
    ///
    /// Only the game is required. Every other input that is absent or does not read is a
    /// finding of each run, and the run goes on with less.
    pub fn open(choice: &Choice, names_index: &Utf8Path) -> Result<Self, AppError> {
        let root = choice.game.clone().ok_or(AppError::NoGame)?;
        let install = Install::open(&root).map_err(|error| AppError::GameUnreadable {
            path: root.to_string(),
            detail: chain(&error),
        })?;
        let unreadable = install.unreadable().to_vec();
        let no_build = install.no_build().cloned();

        let hashes = choice.hashes.clone().or_else(mimir_cache_dir);
        let mut names = Names::builder().name_hash_index(names_index);
        if let Some(dir) = &hashes {
            names = names.mimir(dir);
        }
        let names = names.build().map_err(|error| AppError::NamesUnreadable {
            detail: chain(&error),
        })?;

        let mut env = Env::builder().install(install).names(names);

        let history = match &choice.history {
            None => Opened::Absent,
            Some(dir) => match SqliteHistory::open(dir) {
                Ok(history) => {
                    env = env.history(Box::new(history));
                    Opened::Read(dir.clone())
                }
                Err(error) => Opened::Unreadable {
                    path: dir.clone(),
                    reason: chain(&error),
                },
            },
        };

        let schema = match choice.schema.clone().or_else(Schema::shared_copy) {
            None => Opened::Absent,
            Some(path) => match Schema::open(&path) {
                Ok(schema) => {
                    env = env.schema(schema);
                    Opened::Read(path)
                }
                Err(error) => Opened::Unreadable {
                    reason: chain(&error),
                    path,
                },
            },
        };

        let env = env.build().map_err(|error| AppError::Internal {
            detail: error.to_string(),
        })?;
        Ok(Self {
            env,
            root,
            unreadable,
            no_build,
            hashes,
            schema,
            history,
        })
    }

    /// Inputs over an environment a caller built: a game that is no install, as a test's is.
    /// What each part lacks is raised as [`Inputs::open`] raises it.
    pub fn of_env(env: Env) -> Self {
        let root = Utf8PathBuf::from(env.game().describe());
        let schema = match env.schema() {
            Some(_) => Opened::Read(Utf8PathBuf::new()),
            None => Opened::Absent,
        };
        let history = match env.history().stamp() {
            Some(_) => Opened::Read(Utf8PathBuf::new()),
            None => Opened::Absent,
        };
        Self {
            env,
            root,
            unreadable: Vec::new(),
            no_build: None,
            hashes: None,
            schema,
            history,
        }
    }

    /// The environment every run is judged against.
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// Raises what the inputs found, in the order the command raises it when it opens them.
    pub fn report(&self, sink: &mut Sink<'_>) {
        if !self.unreadable.is_empty() {
            sink.raise(WadsUnreadable {
                wads: self.unreadable.clone(),
            });
        }
        if let Some(why) = &self.no_build {
            sink.raise(VersionUnknown { why: why.clone() });
        }
        let build = self.env.game().build().cloned();

        if self.hashes.is_none() {
            sink.raise(MimirAbsent::no_directory());
        }
        self.env.names().report(sink);

        match &self.history {
            Opened::Absent => {
                sink.raise(HistoryAbsent);
            }
            Opened::Unreadable { path, reason } => {
                sink.raise(IndexMissing {
                    index: path.to_string(),
                    reason: reason.clone(),
                });
            }
            Opened::Read(_) => {}
        }
        if let Some(stamp) = self.env.history().stamp() {
            stamp.check(None, build.as_ref(), sink);
        }

        match (&self.schema, self.env.schema()) {
            (Opened::Absent, _) => {
                sink.raise(SchemaAbsent);
            }
            (Opened::Unreadable { reason, .. }, _) => {
                sink.raise(SchemaUnreadable {
                    reason: reason.clone(),
                });
            }
            (Opened::Read(_), Some(schema)) => schema.report(build.as_ref(), sink),
            (Opened::Read(_), None) => {}
        }
    }

    /// A new log holding what the inputs found, for a run to start from.
    pub fn log(&self) -> Diagnostics {
        let mut log = Diagnostics::new();
        self.report(&mut log.at(Stage::Read));
        log
    }

    /// The inputs as the front end shows them.
    pub fn view(&self) -> InputsView {
        let log = self.log();
        let book = NameBook::new();
        let speller = Speller::new(&book, self.env.names());
        let cx = Render {
            names: &speller,
            module: None,
        };
        let stamp = self.env.history().stamp();
        let tables = self
            .env
            .names()
            .sources()
            .iter()
            .flat_map(|source| match source {
                NameSource::Mimir { tables, .. } => {
                    tables.iter().map(ToString::to_string).collect()
                }
                NameSource::TextTables { .. } => Vec::new(),
            });
        InputsView {
            game: GameView {
                path: self.root.to_string(),
                build: self.env.game().build().map(ToString::to_string),
                unreadable_wads: self.unreadable.len(),
            },
            hashes: HashesView {
                dir: self.hashes.as_ref().map(ToString::to_string),
                tables: tables.collect(),
            },
            schema: SchemaView {
                path: self.schema.path(),
                latest_build: self.env.schema().map(|schema| schema.latest().to_string()),
            },
            history: HistoryView {
                dir: self.history.path(),
                newest_build: stamp.map(|stamp| stamp.newest_build.to_string()),
                builds: stamp.map(|stamp| stamp.builds),
            },
            findings: log.iter().map(|d| Finding::of(d, &cx)).collect(),
        }
    }
}

/// The game install as read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct GameView {
    /// The folder.
    pub path: String,
    /// The build it says it is.
    pub build: Option<String>,
    /// How many of its WADs did not read.
    pub unreadable_wads: usize,
}

/// The hash tables as read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct HashesView {
    /// The hash cache's folder. None when the platform names none.
    pub dir: Option<String>,
    /// The tables of it that opened.
    pub tables: Vec<String>,
}

/// The schema database as read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct SchemaView {
    /// The file that was read. None when none was found or it did not read.
    pub path: Option<String>,
    /// The newest build number it describes.
    pub latest_build: Option<String>,
}

/// The history index as read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct HistoryView {
    /// Its folder. None when none was chosen or it did not open.
    pub dir: Option<String>,
    /// The newest build it holds.
    pub newest_build: Option<String>,
    /// How many builds it holds.
    pub builds: Option<usize>,
}

/// The inputs as the front end shows them: each one, and what opening them found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct InputsView {
    /// The game install.
    pub game: GameView,
    /// The hash tables.
    pub hashes: HashesView,
    /// The schema database.
    pub schema: SchemaView,
    /// The history index.
    pub history: HistoryView,
    /// What opening them found, as each run reports it.
    pub findings: Vec<Finding>,
}

/// The game install on this machine, found as LTK Manager finds it: the Riot Client's list of
/// installs, a running client, then the usual folders on every drive.
pub fn find_game() -> Option<String> {
    let exe = ltk_mod_core::auto_detect_league_path()?;
    // The detection names `<install>/Game/League of Legends.exe`.
    exe.parent()?.parent().map(ToString::to_string)
}
