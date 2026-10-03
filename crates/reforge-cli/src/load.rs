//! `reforge load`: read one archive, load it, gather what is known of its files, and report.

use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use clap::Args;
use fs_err as fs;
use reforge::archive::{Archive, Caps};
use reforge::diag::{Diagnostics, Render, Sink, Stage};
use reforge::env::Env;
use reforge::game::{Game, Install, MimirAbsent, Names, Speller, mimir_cache_dir};
use reforge::history;
use reforge::load::{Dating, FileProvenance, Inventory, LoadError, LoadOptions, Loader, against};
use reforge::model::{Dump, DumpCx, Mod};
use reforge::report::Report;
use reforge_census::GitRepo;

/// The exit code of a load that a fatal diagnostic ended. A fault ends with 1.
const HALTED: u8 = 2;

#[derive(Args)]
pub struct Load {
    /// The mod archive: a `.fantome`, or a zip of the same layout.
    archive: Utf8PathBuf,
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
    /// Write the report, as JSON, to this file.
    #[arg(long)]
    report: Option<Utf8PathBuf>,
    /// Print the mod as `load` left it: bins, meshes, skeletons and clips among its files.
    #[arg(long)]
    dump: bool,
    /// List every file in the dump.
    #[arg(long, requires = "dump")]
    every_file: bool,
    /// Carry stale copies as the mod's own files.
    #[arg(long)]
    keep_stale: bool,
    /// Turn a file fixup off by its id (`legacy-formats`, `moved-assets`). Repeatable.
    #[arg(long = "no-fixup")]
    no_fixup: Vec<String>,
}

/// What `load` came to.
struct Loaded {
    /// The mod, as far as the steps went.
    module: Mod,
    /// Whether a fatal diagnostic ended the load.
    halted: bool,
}

impl Load {
    pub fn run(&self) -> Result<ExitCode> {
        let mut log = Diagnostics::new();
        let env = self.env(&mut log.at(Stage::Read))?;

        let caps = Caps::default();
        let read = Archive::read_file(&self.archive, &caps, &mut log.at(Stage::Read));
        let mut loader = Loader::new(&env).options(self.options()?);
        let loaded = match read {
            Ok(archive) => Some(Self::load(&mut loader, archive, &mut log.at(Stage::Load))?),
            Err(_) => None,
        };
        let book = loader.into_names();
        let speller = Speller::new(&book, env.names());

        let mut report = Report::builder().env(&env);
        if let Some(loaded) = &loaded {
            let m = &loaded.module;
            report = report.module(m);
            if !loaded.halted {
                let mut sink = log.at(Stage::Load);
                let dating = Dating::find(m, env.history()).context("dating the mod")?;
                let inventory = Inventory::take(m, &speller);
                inventory.report(&mut sink);
                let provenance = FileProvenance::find(m, &against(&env, &speller))
                    .context("finding whose bytes the mod's files are")?;
                provenance.report(&mut sink);
                report = report
                    .dating(&dating, m)
                    .inventory(&inventory)
                    .provenance(&provenance);
            }
        }

        let module = loaded.as_ref().map(|loaded| &loaded.module);
        let cx = Render {
            names: &speller,
            module,
        };
        for diagnostic in log.iter() {
            outln!(
                "{}[{}] {}: {}",
                diagnostic.severity(),
                diagnostic.code,
                cx.subject_name(&diagnostic.subject),
                diagnostic.message(&cx)
            );
        }
        if let (true, Some(m)) = (self.dump, module) {
            let cx = DumpCx::new(&speller);
            let cx = if self.every_file { cx.every_file() } else { cx };
            out!("{}", m.dump(&cx));
        }
        if let Some(path) = &self.report {
            let report = report.diagnostics(&log, &cx).build();
            fs::write(path, report.to_json() + "\n")?;
        }
        Ok(match log.halted() {
            Some(_) => ExitCode::from(HALTED),
            None => ExitCode::SUCCESS,
        })
    }

    /// Opens the run's inputs. Each raises what it found about itself.
    fn env(&self, sink: &mut Sink<'_>) -> Result<Env> {
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
        let env = Env::builder().install(install).history(history);
        Ok(env.names(names).build()?)
    }

    fn options(&self) -> Result<LoadOptions> {
        let mut options = LoadOptions::new().keep_stale(self.keep_stale);
        let known: Vec<&str> = options.fixups.iter().map(|fixup| fixup.id()).collect();
        for id in &self.no_fixup {
            if !known.contains(&id.as_str()) {
                bail!("{id} is no file fixup: {}", known.join(", "));
            }
            options = options.without_fixup(id);
        }
        Ok(options)
    }

    /// The steps of `load` in order. A halt keeps the mod as far as it went, for the report.
    fn load(loader: &mut Loader<'_>, archive: Archive, sink: &mut Sink<'_>) -> Result<Loaded> {
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
}
