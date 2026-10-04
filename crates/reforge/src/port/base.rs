//! The game's side of a comparison: the object an author's object is compared with.
//!
//! What the author changed is the mod against the game at the mod's own build (#ADR-0043).
//! Every comparison of the plan and of the build asks its base here, so the build the base is
//! read at is chosen in this file alone. It is the target build.

use std::collections::HashMap;
use std::sync::Arc;

use ltk_meta::BinObject;

use crate::env::Env;
use crate::game::{GameBin, GameError, GameView, SkinCatalog};
use crate::model::{BinHash, ContentId, WadHash};

/// The game an author's objects are compared with.
#[derive(Clone, Copy)]
pub struct Base<'a> {
    game: &'a GameView,
}

/// The game's object under an entry.
#[derive(Debug, Clone, Copy)]
pub struct BaseObject<'a> {
    /// Its value.
    pub value: &'a BinObject,
    /// The digest of its bytes from its entry hash on.
    pub raw: ContentId,
}

/// The objects of the game's bins loaded together, the first under each entry.
#[derive(Debug, Default)]
pub struct BaseObjects {
    bins: Vec<Arc<GameBin>>,
    at: HashMap<BinHash, (usize, usize)>,
}

impl BaseObjects {
    /// The game's object under an entry.
    pub fn object(&self, entry: BinHash) -> Option<BaseObject<'_>> {
        let &(bin, place) = self.at.get(&entry)?;
        let object = self.bins[bin].parsed.objects.get(place)?;
        Some(BaseObject {
            value: &object.value,
            raw: object.raw,
        })
    }

    /// Whether the game holds an object under an entry.
    pub fn holds(&self, entry: BinHash) -> bool {
        self.at.contains_key(&entry)
    }

    /// The entries held.
    pub fn entries(&self) -> impl Iterator<Item = BinHash> + '_ {
        self.at.keys().copied()
    }
}

impl<'a> Base<'a> {
    /// The base at the target build: the game of the environment.
    pub fn at_target(env: &'a Env) -> Self {
        Self { game: env.game() }
    }

    /// The objects of the game's bins at these paths. A path the game has no bin at holds
    /// none.
    pub fn loaded(&self, bins: &[WadHash]) -> Result<BaseObjects, GameError> {
        let mut out = BaseObjects::default();
        for &path in bins {
            let Some(bin) = self.game.bin(path)? else {
                continue;
            };
            let at = out.bins.len();
            for (place, object) in bin.parsed.objects.iter().enumerate() {
                let entry = object.value.path_hash;
                out.at.entry(entry).or_insert((at, place));
            }
            out.bins.push(bin);
        }
        Ok(out)
    }

    /// The game's object under an entry among a character's skins.
    pub fn of_skins<'c>(&self, catalog: &'c SkinCatalog, entry: BinHash) -> Option<&'c BinObject> {
        catalog.object(entry)
    }
}
