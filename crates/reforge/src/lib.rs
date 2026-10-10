//! A porter for League of Legends skin mods. A run takes one mod archive through its stages,
//! each a module here, and a module uses only the modules of earlier stages.
//!
//! `docs/design/ARCHITECTURE.md`.

pub mod model;

pub mod diag;

pub mod store;

pub mod archive;

pub mod game;

pub mod history;

pub mod env;

pub mod load;

pub mod fixups;

pub mod facts;

pub mod port;

pub mod package;

pub mod peek;

pub mod report;

pub mod snapshot;

pub mod porter;

pub use self::porter::{Evidence, Porter, Repairer, Run, RunError, Settings, Start, codes};
