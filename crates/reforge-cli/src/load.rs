//! `reforge load`: read one archive, load it, gather what is known of its files, and report.
//! With `--fix`, the fixups are applied to the loaded mod and the report holds what they
//! changed.

use std::collections::BTreeSet;
use std::process::ExitCode;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Args;
use fs_err as fs;
use reforge::archive::{Archive, Caps};
use reforge::diag::{DiagnosticPolicy, Diagnostics, Render, Stage};
use reforge::fixups::{Changes, FixInputs, FixedMod, FixupSet, ObjectSet};
use reforge::game::Speller;
use reforge::load::{Dating, FileProvenance, Inventory, LoadOptions, Loader, against};
use reforge::model::{Dump, DumpCx, Mod};
use reforge::report::Report;

use crate::fixups::Choice;
use crate::run::{self, HALTED, Inputs, SchemaInput};

#[derive(Args)]
pub struct Load {
    /// The mod archive: a `.fantome`, or a zip of the same layout.
    archive: Utf8PathBuf,
    #[command(flatten)]
    inputs: Inputs,
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
    /// Apply the fixups on bin values to the loaded mod, and report what they changed.
    #[arg(long)]
    fix: bool,
    /// The meta schema database the fixups read the target build's types from (default: the
    /// one LTK Manager keeps on this machine). Read with `--fix` only.
    #[arg(long, requires = "fix")]
    schema: Option<Utf8PathBuf>,
    #[command(flatten)]
    choice: Choice,
}

/// What is known of a loaded mod's files before any value is read.
struct Gathered {
    dating: Dating,
    inventory: Inventory,
    provenance: FileProvenance,
}

/// The mod as far as the run took it: as `load` left it, or as the fixups left it.
enum Reached {
    Loaded(Mod),
    Fixed(FixedMod, Changes),
}

impl Reached {
    fn module(&self) -> &Mod {
        match self {
            Reached::Loaded(module) => module,
            Reached::Fixed(fixed, _) => fixed,
        }
    }

    fn changes(&self) -> Option<&Changes> {
        match self {
            Reached::Loaded(_) => None,
            Reached::Fixed(_, changes) => Some(changes),
        }
    }
}

impl Load {
    pub fn run(&self) -> Result<ExitCode> {
        let mut log = Diagnostics::new();
        let set = FixupSet::from(&self.choice.selection()?);
        // Only the fixups read the schema: a load alone says nothing of it.
        let schema = match self.fix {
            true => SchemaInput::Read(self.schema.as_deref()),
            false => SchemaInput::Unread,
        };
        let env = self.inputs.env(schema, &mut log.at(Stage::Read))?;

        let caps = Caps::default();
        let read = Archive::read_file(&self.archive, &caps, &mut log.at(Stage::Read));
        let options = LoadOptions::new()
            .keep_stale(self.keep_stale)
            .file_fixups(set.file_fixups());
        let mut loader = Loader::new(&env).options(options);
        let loaded = match read {
            Ok(archive) => {
                let restored = BTreeSet::new();
                let mut sink = log.at(Stage::Load);
                Some(run::load(&mut loader, archive, &restored, &mut sink)?)
            }
            Err(_) => None,
        };
        let mut book = loader.into_names();

        let mut gathered = None;
        if let Some(loaded) = loaded.as_ref().filter(|loaded| !loaded.halted) {
            let m = &loaded.module;
            let speller = Speller::new(&book, env.names());
            let mut sink = log.at(Stage::Load);
            let dating = Dating::find(m, env.history()).context("dating the mod")?;
            let inventory = Inventory::take(m, &speller);
            inventory.report(&mut sink);
            let provenance = FileProvenance::find(m, &against(&env, &speller))
                .context("finding whose bytes the mod's files are")?;
            provenance.report(&mut sink);
            gathered = Some(Gathered {
                dating,
                inventory,
                provenance,
            });
        }

        let reached = match (loaded, &gathered) {
            (Some(loaded), Some(gathered)) if self.fix => {
                let mut inputs = FixInputs {
                    env: &env,
                    dating: &gathered.dating,
                    names: &mut book,
                };
                let mut sink = log.at(Stage::Fix);
                let (fixed, changes) = set
                    .apply(loaded.module, &ObjectSet::every(), &mut inputs, &mut sink)
                    .context("an input of the run did not answer while the mod was fixed")?;
                Some(Reached::Fixed(fixed, changes))
            }
            (loaded, _) => loaded.map(|loaded| Reached::Loaded(loaded.module)),
        };

        let speller = Speller::new(&book, env.names());
        let module = reached.as_ref().map(Reached::module);
        let changes = reached.as_ref().and_then(Reached::changes);
        let mut report = Report::builder().env(&env);
        if let Some(m) = module {
            report = report.module(m);
            if let Some(gathered) = &gathered {
                report = report
                    .dating(&gathered.dating, m)
                    .inventory(&gathered.inventory)
                    .provenance(&gathered.provenance);
            }
            if let Some(changes) = changes {
                report = report.changes(changes, m, &speller);
            }
        }

        let cx = Render {
            names: &speller,
            module,
        };
        run::print(&log, &cx, &DiagnosticPolicy::default());
        if let (true, Some(m)) = (self.dump, module) {
            let cx = DumpCx::new(&speller);
            let cx = if self.every_file { cx.every_file() } else { cx };
            out!("{}", m.dump(&cx));
            if let Some(changes) = changes {
                out!("{}", changes.dump(&cx));
            }
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
}
