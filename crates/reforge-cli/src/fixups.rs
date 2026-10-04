//! `reforge fixups`: the catalog of fixups, and the choice of them a command line makes.

use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use clap::Args;
use reforge::fixups::{FixupSelection, FixupSet};

#[derive(Args)]
pub struct Fixups {
    /// Print the catalog as JSON: what a caller lists and toggles the fixups by.
    #[arg(long)]
    json: bool,
}

impl Fixups {
    pub fn run(&self) -> Result<ExitCode> {
        let catalog = FixupSet::catalog();
        if self.json {
            outln!("{}", serde_json::to_string_pretty(&catalog)?);
            return Ok(ExitCode::SUCCESS);
        }
        outln!(
            "{:<20} {:<8} {:<6} {:<8} {:<38} title",
            "id",
            "phase",
            "build",
            "default",
            "records"
        );
        for info in catalog {
            let build = info.phase.build();
            let build = build.map_or_else(|| "-".to_string(), |build| build.to_string());
            let records: Vec<String> = info.records.iter().map(ToString::to_string).collect();
            outln!(
                "{:<20} {:<8} {:<6} {:<8} {:<38} {}",
                info.id,
                info.phase.name(),
                build,
                if info.default { "on" } else { "off" },
                records.join(" "),
                info.title
            );
        }
        Ok(ExitCode::SUCCESS)
    }
}

/// The choice of fixups a command line makes: a selection file, and fixups turned off by id.
#[derive(Args)]
pub struct Choice {
    /// A selection of fixups, as JSON: the ids turned off and on, and the rules added.
    #[arg(long)]
    fixups: Option<Utf8PathBuf>,
    /// Turn a fixup off by its id (`reforge fixups` lists them). Repeatable.
    #[arg(long = "no-fixup")]
    no_fixup: Vec<String>,
}

impl Choice {
    /// The selection: the file's, less the fixups turned off. An id that names no fixup is
    /// refused.
    pub fn selection(&self) -> Result<FixupSelection> {
        let mut selection = match &self.fixups {
            Some(path) => FixupSelection::open(path)
                .with_context(|| format!("reading the selection of fixups {path}"))?,
            None => FixupSelection::standard(),
        };
        for id in &self.no_fixup {
            selection = selection.without(id.as_str());
        }
        if let Some(unknown) = selection.unknown().first() {
            let known: Vec<String> = FixupSet::catalog()
                .iter()
                .map(|info| info.id.to_string())
                .collect();
            bail!("{unknown} is no fixup: {}", known.join(", "));
        }
        Ok(selection)
    }
}
