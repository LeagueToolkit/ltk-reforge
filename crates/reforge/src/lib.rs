//! A porter for League of Legends skin mods. A run takes one mod archive through its stages,
//! each a module here, and a module uses only the modules of earlier stages.
//!
//! `docs/design/ARCHITECTURE.md`.

pub mod model;

pub mod diag;

pub mod archive;
