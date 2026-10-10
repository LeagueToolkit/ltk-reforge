//! `reforge`: ports League of Legends skin mods. `docs/design/ARCHITECTURE.md`, "Callers".

#[macro_use]
mod term;
mod fixups;
mod history;
mod inspect;
mod load;
mod port;
mod repair;
mod run;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "reforge",
    version,
    about = "Port League of Legends champion skin mods onto the game's own skins"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List every diagnostic code a run can raise, with its severity and channel.
    Diagnostics,
    /// List every fixup: its id, its phase and build, whether the standard set holds it, the
    /// records it implements.
    Fixups(fixups::Fixups),
    /// The build history: build and inspect the local index.
    History {
        #[command(subcommand)]
        command: history::HistoryCommand,
    },
    /// Inspect one mod archive: read, load and fix it, gather what is true of it and choose
    /// each unit's target slot. Reports the facts, the decision and what the other levels of
    /// animation reuse would choose. Writes no package.
    Inspect(Box<inspect::Inspect>),
    /// Read one mod archive, load it and report what it is: WADs, files, characters, units,
    /// what the client loads, and whose bytes its files are. With `--fix`, apply the fixups
    /// and report what they changed.
    Load(Box<load::Load>),
    /// Port one mod archive: each unit the author made, over the game's own skin at its
    /// target, as a patch of the game's bin with the mod's own objects and files under a
    /// namespace. Written as a mod project and, with `--modpkg`, packed.
    ///
    /// An interrupt (Ctrl-C) ends the run after the step under way: no package is written,
    /// and the report holds the run as far as it went. A second interrupt ends it at once.
    /// Exits with 0 when done, 1 on a fault, 2 when a fatal diagnostic ended the run, 3 when
    /// `--block` refused the write, 4 when an interrupt cancelled the run.
    Port(Box<port::PortCommand>),
    /// Repair one mod archive: the mod as it is, at its own slots and paths, with its bins
    /// fixed for the install's build, written as a mod project and, with `--modpkg`, packed.
    ///
    /// An interrupt (Ctrl-C) ends the run after the step under way: no package is written,
    /// and the report holds the run as far as it went. A second interrupt ends it at once.
    /// Exits with 0 when done, 1 on a fault, 2 when a fatal diagnostic ended the run, 3 when
    /// `--block` refused the write, 4 when an interrupt cancelled the run.
    Repair(Box<repair::Repair>),
}

impl Command {
    fn run(&self) -> Result<ExitCode> {
        match self {
            Command::Diagnostics => {
                for code in reforge::codes() {
                    outln!(
                        "{:<8} {:<9} {code}",
                        code.severity.as_str(),
                        code.channel.as_str()
                    );
                }
                Ok(ExitCode::SUCCESS)
            }
            Command::Fixups(command) => command.run(),
            Command::History { command } => command.run().map(|()| ExitCode::SUCCESS),
            Command::Inspect(command) => command.run(),
            Command::Load(command) => command.run(),
            Command::Port(command) => command.run(),
            Command::Repair(command) => command.run(),
        }
    }
}

fn main() -> ExitCode {
    // An error may name an entry of a mod: printed cleaned, not by the runtime.
    Cli::parse().command.run().unwrap_or_else(|error| {
        errln!("Error: {error:?}");
        ExitCode::FAILURE
    })
}
