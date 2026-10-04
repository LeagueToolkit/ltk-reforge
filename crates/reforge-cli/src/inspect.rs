//! `reforge inspect`: what is true of a mod and where each of its units would go, with no
//! package. The stages in order: read, load, fix, facts, decide. The report holds what each
//! found, and what the policy's target rule chooses at the other levels of animation reuse.

use std::process::ExitCode;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use reforge::Porter;
use reforge::diag::{DiagnosticPolicy, Diagnostics, Stage};
use reforge::fixups::FixupSet;
use reforge::model::{Dump, DumpCx};
use reforge::port::{AnimReuse, Policy, TargetPolicy};

use crate::fixups::Choice;
use crate::run::{self, Inputs, SchemaInput};

#[derive(Args)]
pub struct Inspect {
    /// The mod archive: a `.fantome`, or a zip of the same layout.
    archive: Utf8PathBuf,
    #[command(flatten)]
    inputs: Inputs,
    /// How the target slot is chosen: `auto`, `base`, `origin`, `slot` or a skin number.
    #[arg(long, default_value = "auto", value_parser = TargetPolicy::parse)]
    target: TargetPolicy,
    /// Whether a slot fits when the unit plays clips only other skins play: `block`,
    /// `same-joints`, `own-rig` or `allow`.
    #[arg(long, default_value = "own-rig", value_parser = AnimReuse::parse)]
    anim_reuse: AnimReuse,
    /// Write the report, as JSON, to this file.
    #[arg(long)]
    report: Option<Utf8PathBuf>,
    /// Print the facts and the decisions.
    #[arg(long)]
    dump: bool,
    /// Carry stale copies as the mod's own files.
    #[arg(long)]
    keep_stale: bool,
    /// The meta schema database the fixups read the target build's types from (default: the
    /// one LTK Manager keeps on this machine).
    #[arg(long)]
    schema: Option<Utf8PathBuf>,
    #[command(flatten)]
    choice: Choice,
}

impl Inspect {
    pub fn run(&self) -> Result<ExitCode> {
        let mut log = Diagnostics::new();
        let set = FixupSet::from(&self.choice.selection()?);
        let schema = SchemaInput::Read(self.schema.as_deref());
        let env = self.inputs.env(schema, &mut log.at(Stage::Read))?;
        let policy = Policy {
            target: self.target,
            anim_reuse: self.anim_reuse,
        };

        // Only the policy asked for speaks. The other levels are recorded.
        let porter = Porter::new(&env)
            .policy(policy)
            .compare(policy.other_levels())
            .fixups(set)
            .keep_stale(self.keep_stale);
        let mut run = porter.run().log_from(log);
        run::through(&mut run, &self.archive, Stage::Decide)?;

        run::print(&run, &DiagnosticPolicy::default());
        if let (true, Some(m), Some(facts), Some(targets)) =
            (self.dump, run.module(), run.facts(), run.targets())
        {
            let speller = run.speller();
            let cx = DumpCx::new(&speller).of(m);
            out!("{}", facts.dump(&cx));
            out!("{}", targets.dump(&cx));
        }
        if let Some(path) = &self.report {
            run::save_report(&run, path)?;
        }
        Ok(run::exit(&run, false))
    }
}
