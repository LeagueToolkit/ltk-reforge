//! A porter for League of Legends skin mods. A run takes one mod archive through its stages,
//! each a module here, and a module uses only the modules of earlier stages.
//!
//! `docs/design/ARCHITECTURE.md`.

pub mod model;

pub mod diag;

pub mod archive;

pub mod game;

pub mod history;

pub mod env;

pub mod load;

pub mod fixups;

pub mod report;

/// Every diagnostic code a run can raise, in the order of the modules that declare them.
/// `docs/design/CODES.md` lists the same codes.
pub fn codes() -> impl Iterator<Item = diag::Code> {
    [
        archive::CODES,
        game::CODES,
        history::CODES,
        load::CODES,
        load::legacy::CODES,
        load::moved::CODES,
        load::dating::CODES,
        load::inventory::CODES,
        load::provenance::CODES,
        load::bank::CODES,
        fixups::CODES,
    ]
    .into_iter()
    .flatten()
    .copied()
}
