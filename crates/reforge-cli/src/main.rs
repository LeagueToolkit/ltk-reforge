//! `reforge`: ports League of Legends skin mods. `docs/design/ARCHITECTURE.md`, "Callers".

#[macro_use]
mod term;
mod history;

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
}

impl Command {
    fn run(&self) -> Result<()> {
        match self {
            Command::Diagnostics => {
                for code in reforge::codes() {
                    outln!(
                        "{:<8} {:<9} {code}",
                        code.severity.as_str(),
                        code.channel.as_str()
                    );
                }
                Ok(())
            }
            Command::History { command } => command.run(),
        }
    }
}

fn main() {
    // An error may name an entry of a mod: printed cleaned, not by the runtime.
    if let Err(error) = Cli::parse().command.run() {
        errln!("Error: {error:?}");
        std::process::exit(1);
    }
}
