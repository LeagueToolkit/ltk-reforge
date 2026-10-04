//! What the commands that run a mod share: the inputs a run is judged against, the steps of
//! `load`, and the diagnostics as a terminal shows them.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use reforge::archive::Archive;
use reforge::diag::{DiagnosticPolicy, Diagnostics, Render, Sink};
use reforge::env::Env;
use reforge::game::{Game, Install, MimirAbsent, Names, Schema, mimir_cache_dir};
use reforge::history;
use reforge::load::{LoadError, Loader};
use reforge::model::Mod;
use reforge_census::GitRepo;

/// The exit code of a run that a fatal diagnostic ended. A fault ends with 1.
pub const HALTED: u8 = 2;

/// The inputs a run is judged against, as a command line names them.
#[derive(Args)]
pub struct Inputs {
    /// The game install the mod is judged against.
    #[arg(long)]
    game: Utf8PathBuf,
    /// The history index. Without it the mod is judged against the install alone.
    #[arg(long)]
    index: Option<Utf8PathBuf>,
    /// The census history, to compare its head with the index's.
    #[arg(long)]
    repo: Option<Utf8PathBuf>,
    /// The hash cache's directory (default: the one LTK Manager keeps on this machine).
    #[arg(long)]
    mimir: Option<Utf8PathBuf>,
    /// Read no hash cache.
    #[arg(long)]
    no_mimir: bool,
    /// A directory of CommunityDragon text tables. Repeatable, read in the order given.
    #[arg(long = "text-tables")]
    text_tables: Vec<Utf8PathBuf>,
    /// Where the name-hash index of the hash cache's paths is kept. Built there when absent.
    #[arg(long)]
    names_index: Option<Utf8PathBuf>,
}

/// Which meta schema database a run reads.
#[derive(Debug, Clone, Copy)]
pub enum SchemaInput<'a> {
    /// No database: the run reads no value's type.
    Unread,
    /// The database at a path, else the one LTK Manager keeps on this machine.
    Read(Option<&'a Utf8Path>),
}

impl Inputs {
    /// Opens the inputs. Each raises what it found about itself.
    pub fn env(&self, schema: SchemaInput<'_>, sink: &mut Sink<'_>) -> Result<Env> {
        let install = Install::open(&self.game)?;
        install.report(sink);
        let build = install.build().cloned();

        let mut names = Names::builder();
        let mimir = match (&self.mimir, self.no_mimir) {
            (_, true) => None,
            (Some(dir), false) => Some(dir.clone()),
            (None, false) => {
                let usual = mimir_cache_dir();
                if usual.is_none() {
                    sink.raise(MimirAbsent::no_directory());
                }
                usual
            }
        };
        if let Some(dir) = mimir {
            names = names.mimir(dir);
        }
        for dir in &self.text_tables {
            names = names.text_tables(dir);
        }
        if let Some(dir) = &self.names_index {
            names = names.name_hash_index(dir);
        }
        let names = names.build().context("opening the name tables")?;
        names.report(sink);

        let history = history::open(self.index.as_deref(), sink);
        if let Some(stamp) = history.stamp() {
            let head = match &self.repo {
                Some(repo) => Some(GitRepo::open(repo)?.head()),
                None => None,
            };
            stamp.check(head.as_deref(), build.as_ref(), sink);
        }
        let schema = match schema {
            SchemaInput::Unread => None,
            SchemaInput::Read(path) => {
                let path = path.map(Utf8Path::to_path_buf);
                let path = path.or_else(Schema::manager_cache);
                let schema = Schema::open_reported(path.as_deref(), sink);
                if let Some(schema) = &schema {
                    schema.report(build.as_ref(), sink);
                }
                schema
            }
        };
        let env = Env::builder().install(install).history(history);
        Ok(env.names(names).schema(schema).build()?)
    }
}

/// What `load` came to.
pub struct Loaded {
    /// The mod, as far as the steps went.
    pub module: Mod,
    /// Whether a fatal diagnostic ended the load.
    pub halted: bool,
}

/// The steps of `load` in order. A halt keeps the mod as far as it went, for the report.
pub fn load(loader: &mut Loader<'_>, archive: Archive, sink: &mut Sink<'_>) -> Result<Loaded> {
    let mut module = loader.open(archive);
    let fault = "an input of the run did not answer while the mod was loaded";
    loader.fix_files(&mut module, sink).context(fault)?;
    loader.take_game_bins(&mut module, sink).context(fault)?;
    let halted = match loader.survey(&mut module, sink) {
        Ok(()) => false,
        Err(LoadError::Halted(_)) => true,
        Err(error) => return Err(error).context(fault),
    };
    if !halted {
        loader.resolve(&mut module, sink).context(fault)?;
    }
    Ok(Loaded { module, halted })
}

/// Prints every diagnostic a policy does not hide, one per line.
pub fn print(log: &Diagnostics, cx: &Render<'_>, policy: &DiagnosticPolicy) {
    for diagnostic in log.iter().filter(|d| !policy.hides(d)) {
        // The fixups and the checks share their codes: the one that raised a line is named
        // at its end.
        let by = diagnostic.by.as_deref();
        let by = by.map_or(String::new(), |id| format!(" [{id}]"));
        outln!(
            "{}[{}] {}: {}{by}",
            diagnostic.severity(),
            diagnostic.code,
            cx.subject_name(&diagnostic.subject),
            diagnostic.message(cx)
        );
    }
}
