//! `reforge`: ports League of Legends skin mods. `docs/design/ARCHITECTURE.md`, "Callers".

#[macro_use]
mod term;
mod fixups;
mod history;
mod load;
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
    /// Read one mod archive, load it and report what it is: WADs, files, characters, units,
    /// what the client loads, and whose bytes its files are. With `--fix`, apply the fixups
    /// and report what they changed.
    Load(Box<load::Load>),
    /// Repair one mod archive: the mod as it is, at its own slots and paths, with its bins
    /// fixed for the install's build, written as a mod project and, with `--modpkg`, packed.
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
            Command::Load(command) => command.run(),
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
