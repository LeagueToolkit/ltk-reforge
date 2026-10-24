//! `reforge repair`: the mod as it is, at its own slots and paths, with its bins fixed for
//! the target build (#ADR-0011). The stages in order: read, load, fix, build in place, check,
//! write. The report holds what each found.

use std::process::ExitCode;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use reforge::Repairer;
use reforge::diag::{Diagnostics, Stage};
use reforge::fixups::FixupSet;
use reforge::load::KeptFile;
use reforge::model::{Dump, DumpCx};
use reforge::report::AgainRow;

use crate::fixups::Choice;
use crate::run::{self, Cache, Inputs, Packed, SchemaInput};

#[derive(Args)]
pub struct Repair {
    /// The mod archive: a `.fantome`, or a zip of the same layout. With `--store`, the
    /// archive's SHA-256 in its place starts from what the store holds of it.
    archive: Utf8PathBuf,
    #[command(flatten)]
    inputs: Inputs,
    #[command(flatten)]
    cache: Cache,
    /// The directory the repaired mod is written in: a mod project, `<out>/<project>`.
    #[arg(long, required_unless_present = "no_write")]
    out: Option<Utf8PathBuf>,
    #[command(flatten)]
    packed: Packed,
    /// Do everything but the write.
    #[arg(long)]
    no_write: bool,
    /// Write the report, as JSON, to this file.
    #[arg(long)]
    report: Option<Utf8PathBuf>,
    /// Take the package through the run a second time, in memory, and report what the second
    /// repair changed: nothing, for a repair that is whole.
    #[arg(long)]
    again: bool,
    /// Print the package as built.
    #[arg(long)]
    dump: bool,
    /// Carry stale copies as the mod's own files.
    #[arg(long)]
    keep_stale: bool,
    /// Carry this file as the mod's own where it is a stale copy: `<wad>/<path>`, or
    /// `<wad>/<hash>.<ext>` with the path hash's sixteen hex digits for a file no table names.
    /// Repeatable.
    #[arg(long = "keep-file", value_parser = KeptFile::parse)]
    keep_file: Vec<KeptFile>,
    /// Ship each DDS as it is, put in TEX's container nowhere.
    #[arg(long)]
    keep_dds: bool,
    /// The meta schema database the fixups and the checks read the target build's types from
    /// (default: the one the League Toolkit programs share on this machine).
    #[arg(long)]
    schema: Option<Utf8PathBuf>,
    #[command(flatten)]
    choice: Choice,
    /// Turn a check off by its id: `types`, `round-trip`. Repeatable.
    #[arg(long = "no-check")]
    no_check: Vec<String>,
    /// Refuse the write while the log holds a diagnostic this filter selects: codes or areas
    /// and `key=value` terms, separated by commas (`check,severity=error`). Repeatable.
    #[arg(long)]
    block: Vec<String>,
}

impl Repair {
    pub fn run(&self) -> Result<ExitCode> {
        let mut log = Diagnostics::new();
        let set = FixupSet::from(&self.choice.selection()?);
        let checks = run::checks(&self.no_check)?;
        let policy = run::blocking(&self.block)?;
        let schema = SchemaInput::Read(self.schema.as_deref());
        let env = self.inputs.env(schema, &mut log.at(Stage::Read))?;

        let repairer = Repairer::new(&env)
            .fixups(set)
            .checks(checks)
            .keep_stale(self.keep_stale)
            .keep_files(self.keep_file.clone())
            .keep_dds(self.keep_dds)
            .cancel(run::interrupt()?);
        let caps = repairer.settings().caps();
        let mut run = self
            .cache
            .over(repairer.run().log_from(log), &self.archive, caps)?;
        run::through(&mut run, &self.archive, Stage::Check)?;
        // Before the write: the report beside the package holds the second run.
        if self.again && run.package().is_some() && run.cancelled_at().is_none() {
            run.again()?;
        }

        let out = self.out.as_ref().filter(|_| !self.no_write);
        let refused = match (run.package(), out) {
            (Some(_), Some(out)) => run::write(&mut run, out, self.packed, &policy)?,
            _ => false,
        };

        run::print(&run, &policy);
        if let Some(again) = run.repaired_again() {
            outln!("{}", Self::again_line(again));
        }
        if let (true, Some(package)) = (self.dump, run.package()) {
            let speller = run.speller();
            out!("{}", package.dump(&DumpCx::new(&speller)));
        }
        if let Some(path) = &self.report {
            run::save_report(&run, path)?;
        }
        Ok(run::exit(&run, refused))
    }

    /// What the second repair came to, on one line.
    fn again_line(again: &AgainRow) -> String {
        if again.same() {
            return "repaired again: nothing changes".to_string();
        }
        if !again.built {
            return "repaired again: the second run built no package".to_string();
        }
        let fixups: Vec<String> = again
            .fixups
            .iter()
            .map(|row| format!("{} ({})", row.fixup, row.values))
            .collect();
        let fixups = match fixups.is_empty() {
            true => "none".to_string(),
            false => fixups.join(", "),
        };
        format!(
            "repaired again: {} chunks differ; values the fixups changed again: {fixups}",
            again.chunks_differing
        )
    }
}
