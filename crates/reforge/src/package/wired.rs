//! What a package may hold (#ADR-0039): a file the client reaches. A bin the client loads, a
//! file a loaded bin names, a file at a path the game's own bins read, and what a run made
//! for one of those. A repair and a port ask the same judge.

use std::collections::HashSet;

use ltk_meta::BinObject;
use serde::Serialize;

use crate::model::WadHash;
use crate::model::entity::{FileNode, FileUse};
use crate::model::value;

/// Why a file of the mod is left out of a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LeftOut {
    /// Nothing loads it and nothing names it.
    Unused,
    /// At a path the game shipped at earlier builds and does not ship, where nothing loads
    /// or names it: a bin of an older layout, a texture the game moved.
    FormerPath,
    /// A stale copy (#MOD-ARTIFACT-0004): the game's newer bytes stand.
    Stale,
    /// Only values the fixups removed named it.
    Orphaned,
    /// A port names the game's own file for it: the game holds its bytes where the champion
    /// plays.
    Game,
    /// No object a port carries names it: a bin whose objects the port carries or leaves to
    /// the game, a file only a unit that left the port names.
    NotCarried,
    /// A bank whose own clips ship in a bank the port made. Its other clips are the game's.
    Recut,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// How many files a package leaves out, by the reason.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct LeftBehind {
    /// Files nothing loads and nothing names.
    pub unused: usize,
    /// Files at a former path that nothing loads or names.
    pub former_path: usize,
    /// Stale copies.
    pub stale: usize,
    /// Files only values the fixups removed named.
    pub orphaned: usize,
    /// Files a port names the game's own file for.
    #[serde(skip_serializing_if = "is_zero")]
    pub game: usize,
    /// Files no object a port carries names.
    #[serde(skip_serializing_if = "is_zero")]
    pub not_carried: usize,
    /// Banks whose own clips ship in a bank a port made.
    #[serde(skip_serializing_if = "is_zero")]
    pub recut: usize,
}

impl LeftBehind {
    /// Counts one file left out.
    pub fn count(&mut self, reason: LeftOut) {
        match reason {
            LeftOut::Unused => self.unused += 1,
            LeftOut::FormerPath => self.former_path += 1,
            LeftOut::Stale => self.stale += 1,
            LeftOut::Orphaned => self.orphaned += 1,
            LeftOut::Game => self.game += 1,
            LeftOut::NotCarried => self.not_carried += 1,
            LeftOut::Recut => self.recut += 1,
        }
    }

    /// How many files are left out, whatever the reason.
    pub fn total(&self) -> usize {
        let load = self.unused + self.former_path + self.stale + self.orphaned;
        load + self.game + self.not_carried + self.recut
    }

    /// The counts that are not zero, each with its reason as a message says it.
    pub fn reasons(&self) -> Vec<(usize, &'static str)> {
        let all = [
            (self.unused, "that nothing loads or names"),
            (self.former_path, "at a former path that nothing names"),
            (self.stale, "stale copies of the game's"),
            (self.orphaned, "that only removed values named"),
            (self.game, "the game holds, named where the game has them"),
            (self.not_carried, "that no carried object names"),
            (self.recut, "banks whose own clips ship in a bank made"),
        ];
        all.into_iter().filter(|(n, _)| *n > 0).collect()
    }
}

/// Judges which files of a mod the client reaches through the objects a package ships.
#[derive(Debug, Clone, Default)]
pub struct Wired {
    named: HashSet<WadHash>,
}

impl Wired {
    /// A judge by the objects a package ships, as the fixups left them: a file one of them
    /// names is reached.
    pub fn by<'a>(objects: impl IntoIterator<Item = &'a BinObject>) -> Self {
        let refs = objects.into_iter().flat_map(value::file_refs);
        Self {
            named: refs.map(|file| file.hash()).collect(),
        }
    }

    /// Whether the client reaches a file, by the use `load` found for it, and why not.
    ///
    /// A file a fixup made is judged as any other: it is at the path its use was found at.
    /// A stale copy kept on the caller's wish is no stale copy to `load`, which marks it an
    /// override.
    pub fn judge(&self, file: &FileNode) -> Result<(), LeftOut> {
        match file.used {
            FileUse::Loaded | FileUse::Override => Ok(()),
            FileUse::Referenced if self.named.contains(&file.hash) => Ok(()),
            FileUse::Referenced => Err(LeftOut::Orphaned),
            FileUse::Unused => Err(LeftOut::Unused),
            FileUse::FormerPath => Err(LeftOut::FormerPath),
            FileUse::Stale => Err(LeftOut::Stale),
        }
    }
}
