//! Which of the mod's objects are the game's: byte for byte under their own name (an exact
//! copy), or saved under another name (a renamed copy, `Skin14/Resources` pasted as
//! `Skin0/Resources`), in the install or at some build of the history
//! (#MOD-ARTIFACT-0002).
//!
//! The history finds a renamed copy by the norm key, which leaves the class out: a hit counts
//! only when it is of the object's own class.

use std::collections::BTreeMap;

use serde::Serialize;

use super::{Catalogs, FactsError};
use crate::env::Env;
use crate::fixups::FixedMod;
use crate::history::{Life, ObjectHit};
use crate::model::entity::{CharId, Mod, ObjectId, ObjectNode, WadId};
use crate::model::hash::skin_paths;
use crate::model::{
    BinHash, MAX_SLOT, NormKey, ObjectKey, SideTable, Slot, WadHash, name_hash, path_hash,
};

/// Where a game object was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "via", rename_all = "lowercase")]
pub enum Via {
    /// The install holds it.
    Install,
    /// The game held it at some builds, and the install does not.
    History {
        /// The builds the game held it.
        life: Life,
    },
}

/// Whose an object of the mod is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "origin", rename_all = "kebab-case")]
pub enum ObjectOrigin {
    /// The game's object byte for byte.
    GameExact {
        /// The game object's entry hash.
        #[serde(serialize_with = "super::entry_hex")]
        entry: BinHash,
        /// The skin whose bins hold it, when it is one skin's.
        slot: Option<Slot>,
        /// Where it was found.
        via: Via,
    },
    /// A game object saved under another name.
    GameRenamed {
        /// The game object's entry hash.
        #[serde(serialize_with = "super::entry_hex")]
        entry: BinHash,
        /// The skin whose bins hold it, when every copy is one skin's.
        slot: Option<Slot>,
        /// Where it was found.
        via: Via,
    },
    /// The mod's own, as far as the install and the history reach.
    Own,
}

impl ObjectOrigin {
    /// The skin a game object is of, when it is one skin's.
    pub fn slot(&self) -> Option<Slot> {
        match self {
            ObjectOrigin::GameExact { slot, .. } | ObjectOrigin::GameRenamed { slot, .. } => *slot,
            ObjectOrigin::Own => None,
        }
    }
}

/// The slot a game bin or entry belongs to, by the names skins give them.
struct SlotNames {
    bins: BTreeMap<WadHash, Slot>,
    entries: BTreeMap<BinHash, Slot>,
}

impl SlotNames {
    fn of(character: &str) -> Self {
        let mut names = Self {
            bins: BTreeMap::new(),
            entries: BTreeMap::new(),
        };
        for slot in (0..=MAX_SLOT).map(Slot) {
            let bins = [
                skin_paths::skin_bin(character, slot),
                skin_paths::animation_bin(character, slot),
            ];
            names
                .bins
                .extend(bins.iter().map(|path| (path_hash(path), slot)));
            let entries = [
                skin_paths::skin_entry(character, slot),
                skin_paths::resolver_entry(character, slot),
                skin_paths::graph_entry(character, slot),
            ];
            names
                .entries
                .extend(entries.iter().map(|name| (name_hash(name), slot)));
        }
        names
    }

    fn slot(&self, hit: &ObjectHit) -> Option<Slot> {
        self.bins
            .get(&hit.bin)
            .or_else(|| self.entries.get(&hit.entry))
            .copied()
    }
}

/// The search for one mod's objects.
struct Finder<'a> {
    env: &'a Env,
    catalogs: &'a Catalogs,
    names: BTreeMap<CharId, SlotNames>,
    /// The characters whose bins a WAD holds: its champion, then the sub-characters.
    characters: BTreeMap<WadId, Vec<CharId>>,
}

impl Finder<'_> {
    /// The skin an entry of a character's catalog is of. An object every skin's bins hold
    /// says nothing of which skin.
    fn catalog_slot(&self, character: CharId, entry: BinHash) -> Option<Slot> {
        let catalog = self.catalogs.of(character)?;
        let slots = catalog.slots_of(entry)?;
        if catalog.skins.len() > 1 && slots.len() == catalog.skins.len() {
            return None;
        }
        slots.first().map(|slot| catalog.canon(*slot))
    }

    /// The skin a history hit is of: of the first character whose names its bin or its entry
    /// spells.
    fn history_slot(&self, hit: &ObjectHit, characters: &[CharId]) -> Option<Slot> {
        characters.iter().find_map(|&character| {
            let slot = self.names.get(&character)?.slot(hit)?;
            let catalog = self.catalogs.of(character);
            Some(catalog.map_or(slot, |catalog| catalog.canon(slot)))
        })
    }

    fn origin(&self, m: &Mod, object: &ObjectNode) -> Result<ObjectOrigin, FactsError> {
        let history = self.env.history();
        let wad = m.files[m.bins[object.bin].file].wad;
        let characters = self.characters.get(&wad).map_or(&[][..], Vec::as_slice);
        let catalogs = || {
            characters
                .iter()
                .filter_map(|&c| Some((c, self.catalogs.of(c)?)))
        };

        // The install first, then the history: exact, then renamed.
        for (character, catalog) in catalogs() {
            if let Some(found) = catalog.exact(object.raw) {
                return Ok(ObjectOrigin::GameExact {
                    entry: found.entry,
                    slot: self.catalog_slot(character, found.entry),
                    via: Via::Install,
                });
            }
        }
        // #ADR-0037: an object is asked by the head of its digest.
        if let Some(hit) = history.object(ObjectKey::from_sha256(&object.raw.0))? {
            return Ok(ObjectOrigin::GameExact {
                entry: hit.entry,
                slot: self.history_slot(&hit, characters),
                via: Via::History { life: hit.life },
            });
        }
        for (_, catalog) in catalogs() {
            let found = catalog.renamed(object.body);
            if let Some(first) = found.first() {
                let slot = catalog.canon(first.slot);
                let one_skin = found.iter().all(|other| catalog.canon(other.slot) == slot);
                return Ok(ObjectOrigin::GameRenamed {
                    entry: first.entry,
                    slot: one_skin.then_some(slot),
                    via: Via::Install,
                });
            }
        }
        let hits = history.objects_by_norm(NormKey::from_sha256(&object.body.0))?;
        let hits: Vec<ObjectHit> = hits
            .into_iter()
            .filter(|hit| hit.class == object.class)
            .collect();
        let Some(first) = hits.first() else {
            return Ok(ObjectOrigin::Own);
        };
        let slot = self.history_slot(first, characters);
        let one_skin = hits
            .iter()
            .all(|hit| self.history_slot(hit, characters) == slot);
        let life = hits.iter().fold(first.life, |life, hit| Life {
            first: life.first.min(hit.life.first),
            last: life.last.max(hit.life.last),
        });
        Ok(ObjectOrigin::GameRenamed {
            entry: first.entry,
            slot: slot.filter(|_| one_skin),
            via: Via::History { life },
        })
    }
}

/// For each object of a loaded bin, whose it is.
#[derive(Default)]
pub struct ObjectProvenance {
    origins: SideTable<ObjectNode, ObjectOrigin>,
}

impl ObjectProvenance {
    /// The provenance of a mod's objects. An object is compared by its bytes as the archive
    /// ships it, whatever the fixups changed in it.
    pub fn find(fixed: &FixedMod, env: &Env, catalogs: &Catalogs) -> Result<Self, FactsError> {
        let m: &Mod = fixed;
        let finder = Finder {
            env,
            catalogs,
            names: m
                .characters
                .iter()
                .map(|(id, c)| (id, SlotNames::of(&c.name)))
                .collect(),
            characters: m
                .groups
                .iter()
                .map(|g| (g.wad, g.characters.clone()))
                .collect(),
        };
        let mut out = Self::default();
        for (id, object) in m.objects.iter() {
            out.origins.insert(id, finder.origin(m, object)?);
        }
        Ok(out)
    }

    /// Whose an object is.
    pub fn origin(&self, object: ObjectId) -> Option<&ObjectOrigin> {
        self.origins.get(object)
    }

    /// Every object with its origin, in object order.
    pub fn iter(&self) -> impl Iterator<Item = (ObjectId, &ObjectOrigin)> {
        self.origins.iter()
    }
}
