// No console window beside the app's in a release build.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::process::ExitCode;

use reforge_app::runlog::RunLogLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

fn main() -> ExitCode {
    // The terminal takes what `RUST_LOG` asks for; a run's details take the library's spans
    // and events, and what other crates warn of.
    let details = EnvFilter::new("warn,reforge=debug");
    tracing_subscriber::registry()
        .with(fmt::layer().with_filter(EnvFilter::from_default_env()))
        .with(RunLogLayer.with_filter(details))
        .init();
    match reforge_app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "the app stopped");
            ExitCode::FAILURE
        }
    }
}
