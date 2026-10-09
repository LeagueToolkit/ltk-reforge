//! `reforge port`: the mod over the game's own skin at each unit's target. The stages in
//! order: read, load, fix, facts, decide, plan, build, check, write. The report holds what
//! each found.

use std::process::ExitCode;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use reforge::Porter;
use reforge::diag::{Diagnostics, Stage};
use reforge::fixups::FixupSet;
use reforge::model::{Dump, DumpCx};
use reforge::port::{AnimReuse, Policy, TargetPolicy};

use crate::fixups::Choice;
use crate::run::{self, Inputs, SchemaInput};

#[derive(Args)]
pub struct PortCommand {
    /// The mod archive: a `.fantome`, or a zip of the same layout.
    archive: Utf8PathBuf,
    #[command(flatten)]
    inputs: Inputs,
    /// The directory the port is written in: a mod project, `<out>/<project>`.
    #[arg(long, required_unless_present = "no_write")]
    out: Option<Utf8PathBuf>,
    /// Also pack the project, as `<out>/<project>_<version>.modpkg`.
    #[arg(long)]
    modpkg: bool,
    /// Do everything but the write.
    #[arg(long)]
    no_write: bool,
    /// Write the report, as JSON, to this file.
    #[arg(long)]
    report: Option<Utf8PathBuf>,
    /// How the target slot is chosen: `auto`, `base`, `origin`, `slot` or a skin number.
    #[arg(long, default_value = "auto", value_parser = TargetPolicy::parse)]
    target: TargetPolicy,
    /// Whether a slot fits when the unit plays clips only other skins play: `block`,
    /// `same-joints`, `own-rig` or `allow`.
    #[arg(long, default_value = "own-rig", value_parser = AnimReuse::parse)]
    anim_reuse: AnimReuse,
    /// Print the plan and the package.
    #[arg(long)]
    dump: bool,
    /// Carry stale copies as the mod's own files.
    #[arg(long)]
    keep_stale: bool,
    /// Cut a bank's events from the game's events bank only: a bank the author added with
    /// events of its own is reported and not carried.
    #[arg(long)]
    game_events_only: bool,
    /// Ship each DDS as it is, put in TEX's container nowhere.
    #[arg(long)]
    keep_dds: bool,
    /// Ship the units at slots other than the mod's target too, each at its own slot: for a
    /// mod that works on several skins.
    #[arg(long)]
    keep_other_slots: bool,
    /// The meta schema database the fixups and the checks read the target build's types from
    /// (default: the one the League Toolkit programs share on this machine).
    #[arg(long)]
    schema: Option<Utf8PathBuf>,
    #[command(flatten)]
    choice: Choice,
    /// Turn a check off by its id: `apply`, `types`, `round-trip`. Repeatable.
    #[arg(long = "no-check")]
    no_check: Vec<String>,
    /// Refuse the write while the log holds a diagnostic this filter selects: codes or areas
    /// and `key=value` terms, separated by commas (`check,severity=error`). Repeatable.
    #[arg(long)]
    block: Vec<String>,
}

impl PortCommand {
    pub fn run(&self) -> Result<ExitCode> {
        let mut log = Diagnostics::new();
        let set = FixupSet::from(&self.choice.selection()?);
        let checks = run::checks(&self.no_check)?;
        let block = run::blocking(&self.block)?;
        let schema = SchemaInput::Read(self.schema.as_deref());
        let env = self.inputs.env(schema, &mut log.at(Stage::Read))?;

        let policy = Policy {
            target: self.target,
            anim_reuse: self.anim_reuse,
        };
        let porter = Porter::new(&env)
            .policy(policy)
            .fixups(set)
            .checks(checks)
            .keep_stale(self.keep_stale)
            .game_events_only(self.game_events_only)
            .keep_dds(self.keep_dds)
            .keep_other_slots(self.keep_other_slots);
        let mut run = porter.run().log_from(log);
        run::through(&mut run, &self.archive, Stage::Check)?;

        let out = self.out.as_ref().filter(|_| !self.no_write);
        let refused = match (run.package(), out) {
            (Some(_), Some(out)) => run::write(&mut run, out, self.modpkg, &block)?,
            _ => false,
        };

        run::print(&run, &block);
        if let (true, Some(m), Some(port)) = (self.dump, run.module(), run.port()) {
            let speller = run.speller();
            let dump = DumpCx::new(&speller).of(m);
            out!("{}", port.dump(&dump));
            if let Some(package) = run.package() {
                out!("{}", package.dump(&dump));
            }
        }
        if let Some(path) = &self.report {
            run::save_report(&run, path)?;
        }
        Ok(run::exit(&run, refused))
    }
}
