//! `reforge load`: read one archive, load it, gather what is known of its files, and report.
//! With `--fix`, the fixups are applied to the loaded mod and the report holds what they
//! changed.

use std::process::ExitCode;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use reforge::Porter;
use reforge::diag::{DiagnosticPolicy, Diagnostics, Stage};
use reforge::fixups::FixupSet;
use reforge::model::{Dump, DumpCx};

use crate::fixups::Choice;
use crate::run::{self, Inputs, SchemaInput};

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
    /// one the League Toolkit programs share on this machine). Read with `--fix` only.
    #[arg(long, requires = "fix")]
    schema: Option<Utf8PathBuf>,
    #[command(flatten)]
    choice: Choice,
}

impl Load {
    pub fn run(&self) -> Result<ExitCode> {
        let mut log = Diagnostics::new();
        let set = FixupSet::from(&self.choice.selection()?);
        // Only the fixups read the schema: a load alone says nothing of it.
        let (schema, last) = match self.fix {
            true => (SchemaInput::Read(self.schema.as_deref()), Stage::Fix),
            false => (SchemaInput::Unread, Stage::Load),
        };
        let env = self.inputs.env(schema, &mut log.at(Stage::Read))?;

        let porter = Porter::new(&env).fixups(set).keep_stale(self.keep_stale);
        let mut run = porter.run().log_from(log);
        run::through(&mut run, &self.archive, last)?;

        run::print(&run, &DiagnosticPolicy::default());
        if let (true, Some(m)) = (self.dump, run.module()) {
            let speller = run.speller();
            let cx = DumpCx::new(&speller);
            let cx = if self.every_file { cx.every_file() } else { cx };
            out!("{}", m.dump(&cx));
            if let Some(changes) = run.changes() {
                out!("{}", changes.dump(&cx));
            }
        }
        if let Some(path) = &self.report {
            run::save_report(&run, path)?;
        }
        Ok(run::exit(&run, false))
    }
}
