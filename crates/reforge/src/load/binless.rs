//! Binless mods (#MOD-ARTIFACT-0013): a champion WAD with no skin bin, only files at the game's
//! own paths, which override the game's for every skin that names them. The game's bins of
//! those skins are taken as the mod's own, so the stages after `load` treat the overrides as
//! any mod's files.
//!
//! - **Overridden** is a file at a path the game ships, with other bytes, that is no stale
//!   copy.
//! - **Which skin.** A file in a skin's folder goes with that skin when the skin's bins name
//!   it. When only other skins name it, no skin claims it: a base mod is not spread onto a
//!   premium skin. A file in no skin's folder goes with the base when the base names it, else
//!   with the lowest skin naming it that is no chroma. A bin a skin links, which the mod ships
//!   its own copy of, goes with the skins linking it, the base first.
//! - **Which bins.** The skin bin, and the bins it links in the character's folder that name an
//!   override. Where the mod ships a bin at one of those paths, its own stands and the game's
//!   is not taken: a WAD holds one chunk per path.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::findings::{Restoration, TakenFromGame, TakenSkin};
use super::rework::Around;
use super::stale::Staleness;
use super::{Fault, Run, locale_tag};
use crate::diag::Sink;
use crate::game::GameSkin;
use crate::model::entity::{FileId, FileKind, FileNode, FileSource, FileUse, WadId};
use crate::model::hash::skin_paths;
use crate::model::{ByteView, MAX_SLOT, Slot, Spell, WadHash, path_hash};

/// The files of one WAD against the game's files at their paths.
#[derive(Default)]
struct Overrides {
    /// The paths at which the WAD overrides the game's file.
    overridden: BTreeSet<WadHash>,
    /// Paths the game does not ship, and none of the three counts below.
    former: BTreeSet<WadHash>,
    /// Files at paths the history never saw.
    never: usize,
    /// Files at paths the game does not ship whose asset has another file of the mod.
    covered: usize,
    /// Stale copies.
    stale: usize,
}

/// The skin whose folder a path is in: `skins/base/` is the base, `skins/skinNN/` skin NN.
fn folder_slot(path: &str) -> Option<Slot> {
    if path.to_ascii_lowercase().contains("/skins/base/") {
        Some(Slot::BASE)
    } else {
        skin_paths::slot_in_path(path)
    }
}

/// The step that takes the game's bins for the WADs with no skin bin.
pub(super) struct Binless<'a> {
    run: Run<'a>,
}

impl<'a> Binless<'a> {
    pub(super) fn new(run: Run<'a>) -> Self {
        Self { run }
    }

    pub(super) fn take(mut self, sink: &mut Sink<'_>) -> Result<(), Fault> {
        let wads: Vec<WadId> = self.run.module.wads.ids().collect();
        // Before any file is judged stale: a restoration's older bytes are its own.
        for &wad in &wads {
            self.restoration(wad, sink)?;
        }
        for wad in wads {
            self.take_for(wad, sink)?;
        }
        Ok(())
    }

    fn restoration(&mut self, wad: WadId, sink: &mut Sink<'_>) -> Result<(), Fault> {
        let history = self.run.env.history();
        let reworks = &self.run.options.reworks;
        let module = &mut *self.run.module;
        let champion = module.wads[wad].name.stem().to_string();
        let around = Around::of(module, history, reworks, &champion)?;
        let Some(around) = around.filter(Around::restores) else {
            return Ok(());
        };
        module.restorations.insert(wad);
        sink.raise(Restoration {
            wad,
            champion,
            patch: around.patch,
            older: around.older,
            newer: around.newer,
        });
        Ok(())
    }

    fn take_for(&mut self, wad: WadId, sink: &mut Sink<'_>) -> Result<(), Fault> {
        let game = self.run.env.game();
        let name = self.run.module.wads[wad].name.clone();
        let champion = name.stem();
        let base = path_hash(&skin_paths::skin_bin(champion, Slot::BASE));
        if locale_tag(&name).is_some()
            || !self.run.module.skin_bins(wad, champion).is_empty()
            || !game.contains(base)
        {
            return Ok(());
        }
        let files = self.overrides(wad)?;
        if files.overridden.is_empty() {
            return Ok(());
        }
        let subs = self.run.env.names().characters_like(champion);
        let subs = subs.into_iter().filter(|sub| sub != champion);
        let characters: Vec<String> = std::iter::once(champion.to_string()).chain(subs).collect();
        let mut claimed = BTreeSet::new();
        let mut skins = Vec::new();
        let mut bins = Vec::new();
        for character in characters {
            let mut game_skins: BTreeMap<Slot, Arc<GameSkin>> = BTreeMap::new();
            for slot in (0..=MAX_SLOT).map(Slot) {
                if let Some(skin) = game.skin(&character, slot)? {
                    game_skins.insert(slot, skin);
                }
            }
            let chosen = self.claim(&game_skins, &files.overridden, &mut claimed);
            for slot in chosen {
                let taken = self.take_skin(wad, &game_skins[&slot], &files.overridden);
                // Every bin of the skin that counts is the mod's own already.
                if taken.is_empty() {
                    continue;
                }
                bins.extend(taken);
                skins.push(TakenSkin {
                    character: character.clone(),
                    slot,
                });
            }
        }
        if skins.is_empty() {
            return Ok(());
        }
        let unclaimed = files.overridden.difference(&claimed).copied().collect();
        sink.raise(TakenFromGame {
            wad,
            skins,
            bins,
            overridden: files.overridden.len(),
            unclaimed,
            former: files.former.into_iter().collect(),
            never: files.never,
            covered: files.covered,
            stale: files.stale,
            history: self.run.env.has_history(),
        });
        Ok(())
    }

    /// Sorts a WAD's files by what they are against the game's file at their path.
    fn overrides(&self, wad: WadId) -> Result<Overrides, Fault> {
        let m = &*self.run.module;
        let env = self.run.env;
        let game = env.game();
        let stale = Staleness::new(env, self.run.options);
        let made_from: BTreeSet<FileId> = m.wads[wad]
            .files
            .iter()
            .filter_map(|&file| match m.files[file].source {
                FileSource::Made { from, .. } => Some(from),
                FileSource::Archive | FileSource::Rewritten { .. } | FileSource::Game => None,
            })
            .collect();
        let mut out = Overrides::default();
        for &id in &m.wads[wad].files {
            let file = &m.files[id];
            match game.contains(file.hash) {
                // The file made of it stands for it.
                false if made_from.contains(&id) => {}
                false if self.covered(wad, id) => out.covered += 1,
                false if env.has_history() && env.history().path(file.hash)?.is_none() => {
                    out.never += 1;
                }
                false => {
                    out.former.insert(file.hash);
                }
                true if stale.of(m, id)? => out.stale += 1,
                true => {
                    if !game.holds(file.hash, file.size, file.content)? {
                        out.overridden.insert(file.hash);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Whether a file at a path the game does not ship has another file of the WAD where a
    /// file fixup would put it.
    fn covered(&self, wad: WadId, file: FileId) -> bool {
        let m = &*self.run.module;
        let Some(path) = &m.files[file].path else {
            return false;
        };
        let game = self.run.env.game();
        self.run.options.fixups.iter().any(|fixup| {
            let made_at = fixup.destinations(path, game);
            made_at.iter().any(|to| m.file_at(wad, to.hash()).is_some())
        })
    }

    /// The skins of one character that claim an override, and what they claim.
    fn claim(
        &self,
        skins: &BTreeMap<Slot, Arc<GameSkin>>,
        overridden: &BTreeSet<WadHash>,
        claimed: &mut BTreeSet<WadHash>,
    ) -> BTreeSet<Slot> {
        let mut naming: BTreeMap<WadHash, Vec<Slot>> = BTreeMap::new();
        // Bins the mod overrides that a skin links: the bins shared between skins.
        let mut linked: BTreeSet<WadHash> = BTreeSet::new();
        for (&slot, skin) in skins {
            let named = skin.bins.iter().flat_map(|(_, bin)| &bin.files);
            for path in named.filter(|path| overridden.contains(path)) {
                naming.entry(*path).or_default().push(slot);
            }
            for (path, _) in skin.bins.iter().skip(1) {
                let hash = path.hash();
                if overridden.contains(&hash) {
                    linked.insert(hash);
                    naming.entry(hash).or_default().push(slot);
                }
            }
        }
        let speller = self.run.speller();
        let mut chosen = BTreeSet::new();
        for (path, slots) in &naming {
            let folder = if linked.contains(path) {
                None
            } else {
                speller.path(*path).and_then(|name| folder_slot(&name))
            };
            let pick = match folder {
                Some(folder) => slots.contains(&folder).then_some(folder),
                None if slots.contains(&Slot::BASE) => Some(Slot::BASE),
                None => {
                    let no_chroma = slots.iter().find(|slot| !skins[slot].chroma);
                    no_chroma.or(slots.first()).copied()
                }
            };
            if let Some(pick) = pick {
                chosen.insert(pick);
                claimed.insert(*path);
            }
        }
        chosen
    }

    /// Adds the game's bins of one skin to the WAD as the mod's own.
    fn take_skin(
        &mut self,
        wad: WadId,
        skin: &GameSkin,
        overridden: &BTreeSet<WadHash>,
    ) -> Vec<FileId> {
        let mut taken = Vec::new();
        for (i, (path, bin)) in skin.bins.iter().enumerate() {
            let names_an_override = bin.files.iter().any(|file| overridden.contains(file));
            // The skin bin always, a linked bin when it names an override.
            if i > 0 && !names_an_override {
                continue;
            }
            self.run.book.learn_path(path.as_str());
            let content = self.run.module.blobs().keep(ByteView::from(&*bin.bytes));
            let added = self.run.module.push_file(FileNode {
                wad,
                hash: path.hash(),
                path: Some(path.clone()),
                size: bin.bytes.len() as u64,
                content,
                kind: FileKind::Bin,
                used: FileUse::Unused,
                source: FileSource::Game,
            });
            // A WAD holds one chunk per path. Where the mod ships a bin at the path, a
            // sub-character's skin bin or a bin shared between skins, its own stands, and the
            // client loads it through the game's link.
            if let Ok(file) = added {
                taken.push(file);
            }
        }
        taken
    }
}
