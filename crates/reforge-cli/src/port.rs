//! `reforge port`: the mod over the game's own skin at each unit's target. The stages in
//! order: read, load, fix, facts, decide, plan, build, check, write. The report holds what
//! each found.

use std::process::ExitCode;

use anyhow::{Result, bail};
use camino::Utf8PathBuf;
use clap::Args;
use reforge::Porter;
use reforge::diag::{Diagnostics, Stage};
use reforge::env::Env;
use reforge::fixups::FixupSet;
use reforge::load::KeptFile;
use reforge::model::{Dump, DumpCx};
use reforge::port::{AnimReuse, Policy, PolicyError, TargetPolicy, UnitTarget};

use crate::fixups::Choice;
use crate::run::{self, Cache, Inputs, Packed, SchemaInput};

#[derive(Args)]
pub struct PortCommand {
    /// The mod archive: a `.fantome`, or a zip of the same layout. With `--store`, the
    /// archive's SHA-256 in its place starts from what the store holds of it.
    archive: Utf8PathBuf,
    #[command(flatten)]
    inputs: Inputs,
    #[command(flatten)]
    cache: Cache,
    /// The directory the port is written in: a mod project, `<out>/<project>`.
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
    /// How the target slot is chosen for every unit: `auto` (the default), `base`, `origin`,
    /// `slot` or a skin number. Or one unit's own target in place of it,
    /// `<character>:<slot>=<slot>`: the unit by its character and the slot the mod ships it
    /// at, and the slot it ships to, `=none` to leave it to the game. Repeatable: one policy,
    /// and a target for each unit named.
    #[arg(long, value_parser = Target::parse)]
    target: Vec<Target>,
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
    /// Carry this file as the mod's own where it is a stale copy: `<wad>/<path>`, or
    /// `<wad>/<hash>.<ext>` with the path hash's sixteen hex digits for a file no table names.
    /// Repeatable.
    #[arg(long = "keep-file", value_parser = KeptFile::parse)]
    keep_file: Vec<KeptFile>,
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

        let porter = self.porter(&env)?.fixups(set).checks(checks);
        let porter = porter.cancel(run::interrupt()?);
        let caps = porter.settings().caps();
        let mut run = self
            .cache
            .over(porter.run().log_from(log), &self.archive, caps)?;
        run::through(&mut run, &self.archive, Stage::Check)?;

        let out = self.out.as_ref().filter(|_| !self.no_write);
        let refused = match (run.package(), out) {
            (Some(_), Some(out)) => run::write(&mut run, out, self.packed, &block)?,
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

    /// A porter under the settings the command line gives, but the fixups, the checks and the
    /// cancel.
    fn porter<'e>(&self, env: &'e Env) -> Result<Porter<'e>> {
        let (target, unit_targets) = targets(&self.target)?;
        let policy = Policy {
            target,
            anim_reuse: self.anim_reuse,
        };
        Ok(Porter::new(env)
            .policy(policy)
            .unit_targets(unit_targets)
            .keep_stale(self.keep_stale)
            .keep_files(self.keep_file.clone())
            .game_events_only(self.game_events_only)
            .keep_dds(self.keep_dds)
            .keep_other_slots(self.keep_other_slots))
    }
}

/// What one `--target` names: the policy for every unit, or one unit's own target.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Policy(TargetPolicy),
    Unit(UnitTarget),
}

impl Target {
    /// A unit's target holds `=`, which no policy's word does.
    fn parse(text: &str) -> Result<Self, PolicyError> {
        if text.contains('=') {
            UnitTarget::parse(text).map(Target::Unit)
        } else {
            TargetPolicy::parse(text).map(Target::Policy)
        }
    }
}

/// The policy the `--target`s name, `auto` when none does, and the units' own targets in the
/// order given.
fn targets(given: &[Target]) -> Result<(TargetPolicy, Vec<UnitTarget>)> {
    let mut policy = None;
    let mut units = Vec::new();
    for target in given {
        match target {
            Target::Policy(named) => {
                if let Some(first) = policy.replace(*named) {
                    bail!("--target names the policy twice: {first} and {named}");
                }
            }
            Target::Unit(unit) => units.push(unit.clone()),
        }
    }
    Ok((policy.unwrap_or_default(), units))
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use reforge::model::Slot;

    use super::*;

    #[derive(Parser)]
    struct Line {
        #[command(flatten)]
        port: PortCommand,
    }

    fn line(args: &[&str]) -> PortCommand {
        let fixed = ["reforge", "mod.fantome", "--game", "game", "--no-write"];
        let args = fixed.iter().chain(args);
        Line::try_parse_from(args).expect("the line parses").port
    }

    #[test]
    fn the_target_names_one_policy_and_any_number_of_units() {
        let port = line(&["--target", "test:0=3", "--target", "base"]);
        let (policy, units) = targets(&port.target).expect("one policy");
        assert_eq!(policy, TargetPolicy::Base);
        assert_eq!(units, [UnitTarget::parse("test:0=3").expect("a target")]);
        let port = line(&["--target", "testpet:2=none"]);
        let (policy, units) = targets(&port.target).expect("no policy");
        assert_eq!((policy, units[0].target), (TargetPolicy::Auto, None));
        let port = line(&["--target", "3", "--target", "base"]);
        assert!(targets(&port.target).is_err(), "two policies");
        assert_eq!(
            targets(&line(&[]).target).expect("none").0,
            TargetPolicy::Auto
        );
        assert!(
            Line::try_parse_from(["reforge", "m", "--game", "g", "--target", "test:x=3"]).is_err()
        );
        assert_eq!(
            line(&["--target", "7"]).target,
            [Target::Policy(TargetPolicy::Fixed(Slot(7)))]
        );
    }

    #[test]
    fn the_files_to_keep_are_repeated_each_by_its_wad_and_path() {
        let port = line(&[
            "--keep-file",
            "Test.wad.client/assets/a.tex",
            "--keep-file",
            "Test.wad.client/0123456789abcdef.tex",
        ]);
        let kept: Vec<String> = port.keep_file.iter().map(ToString::to_string).collect();
        assert_eq!(
            kept,
            [
                "test.wad.client/assets/a.tex",
                "test.wad.client/0123456789abcdef.tex"
            ]
        );
        assert!(
            Line::try_parse_from(["reforge", "m", "--game", "g", "--keep-file", "a.tex"]).is_err()
        );
    }
}
