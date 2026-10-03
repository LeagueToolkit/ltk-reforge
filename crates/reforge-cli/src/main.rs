//! `reforge`: ports League of Legends skin mods. `docs/design/ARCHITECTURE.md`, "Callers".

#[macro_use]
mod term;
mod history;
mod load;

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
    /// The build history: build and inspect the local index.
    History {
        #[command(subcommand)]
        command: history::HistoryCommand,
    },
    /// Read one mod archive, load it and report what it is: WADs, files, characters, units,
    /// what the client loads, and whose bytes its files are.
    Load(load::Load),
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
            Command::History { command } => command.run().map(|()| ExitCode::SUCCESS),
            Command::Load(command) => command.run(),
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
