//! The survey: whose WAD each WAD is, the characters in a champion's WAD and one skin unit per
//! skin bin shipped. By path only: no bin is read here.
//!
//! A WAD is a champion's when it ships `data/characters/<stem>/skins/skin<N>.bin` for some N,
//! `<stem>` being its name up to the first dot. Any slot counts, so a mod made only for a
//! premium skin is found.

use std::collections::{BTreeMap, BTreeSet};

use super::findings::{
    ChampionAmbiguous, ChampionGuessed, EqualToGame, LocalePacked, LocaleWad, NoSkin,
    SeveralChampions, WadDuplicate, WadDuplicateDiffers, WadNotCarried,
};
use super::{LoadError, Run, locale_of, locale_tag};
use crate::diag::Sink;
use crate::game::GameError;
use crate::model::entity::{
    CharId, CharKind, Character, FileId, Group, SkinUnit, UnitId, WadId, WadRole,
};
use crate::model::hash::skin_paths;
use crate::model::{Slot, WadHash, WadName, path_hash};

/// The extensions of a champion WAD as the game spells it.
const GAME_SPELLING: &str = "wad.client";

/// A WAD's name after its first dot, lowercase: `wad.client`, `en_us.wad.client`.
fn rest(name: &WadName) -> &str {
    name.lower()
        .split_once('.')
        .map_or("", |(_, extensions)| extensions)
}

/// The step that finds WAD roles, characters and units.
pub(super) struct Survey<'a> {
    run: Run<'a>,
    /// The one WAD carried for each champion.
    carried: BTreeMap<String, WadId>,
    /// Locale WADs taken as their champion's (#MOD-ARTIFACT-0010).
    packed: BTreeSet<WadId>,
    /// WADs named for no champion, with the champion each is taken as (#MOD-ARTIFACT-0011).
    guessed: BTreeMap<WadId, String>,
}

impl<'a> Survey<'a> {
    pub(super) fn new(run: Run<'a>) -> Self {
        Self {
            run,
            carried: BTreeMap::new(),
            packed: BTreeSet::new(),
            guessed: BTreeMap::new(),
        }
    }

    pub(super) fn run(mut self, sink: &mut Sink<'_>) -> Result<(), LoadError> {
        let wads: Vec<WadId> = self.run.module.wads.ids().collect();
        self.carry_named(&wads);
        self.carry_locale_packed(&wads);
        self.guess(&wads, sink);
        for &wad in &wads {
            self.place(wad, sink).map_err(LoadError::Game)?;
        }
        let m = &*self.run.module;
        if m.groups.len() > 1 {
            sink.raise(SeveralChampions {
                champions: m.groups.iter().map(|group| group.champion).collect(),
            });
        }
        if m.groups.is_empty() {
            return Err(sink.halt(NoSkin { wads: wads.len() }).into());
        }
        Ok(())
    }

    fn name(&self, wad: WadId) -> &WadName {
        &self.run.module.wads[wad].name
    }

    /// One WAD per champion (#MOD-ARTIFACT-0012): the one the game spells, else the first.
    fn carry_named(&mut self, wads: &[WadId]) {
        for &wad in wads {
            let name = self.name(wad).clone();
            if locale_tag(&name).is_some() || self.run.module.skin_bins(wad, name.stem()).is_empty()
            {
                continue;
            }
            let kept = self.carried.get(name.stem()).is_some_and(|&kept| {
                rest(self.name(kept)) == GAME_SPELLING || rest(&name) != GAME_SPELLING
            });
            if !kept {
                self.carried.insert(name.stem().to_string(), wad);
            }
        }
    }

    /// #MOD-ARTIFACT-0010: a locale WAD with its champion's skin bins and no champion WAD
    /// beside it.
    fn carry_locale_packed(&mut self, wads: &[WadId]) {
        for &wad in wads {
            let name = self.name(wad).clone();
            if locale_tag(&name).is_some()
                && !self.carried.contains_key(name.stem())
                && !self.run.module.skin_bins(wad, name.stem()).is_empty()
            {
                self.carried.insert(name.stem().to_string(), wad);
                self.packed.insert(wad);
            }
        }
    }

    /// The WAD the game keeps a character's base skin bin in, by its stem. A champion is a
    /// character the game keeps in a WAD of its own name.
    fn is_champion(&self, character: &str) -> bool {
        let base = path_hash(&skin_paths::skin_bin(character, Slot::BASE));
        let wad = self.run.env.game().wad_of(base);
        wad.is_some_and(|wad| wad.stem() == character)
    }

    /// #MOD-ARTIFACT-0011: a WAD named for no champion is taken as the one champion whose skin
    /// bins it ships, when no WAD of that champion's is carried.
    fn guess(&mut self, wads: &[WadId], sink: &mut Sink<'_>) {
        for &wad in wads {
            let name = self.name(wad).clone();
            if locale_tag(&name).is_some() || self.carried.values().any(|&kept| kept == wad) {
                continue;
            }
            let m = &*self.run.module;
            let paths = m.wads[wad].files.iter();
            let paths = paths.filter_map(|&file| m.files[file].path.as_ref());
            let champions: BTreeSet<String> = paths
                .filter_map(|path| skin_paths::parse_skin_bin(path.as_str()))
                .map(|(character, _)| character)
                .filter(|character| character != name.stem() && self.is_champion(character))
                .collect();
            if champions.len() > 1 {
                sink.raise(ChampionAmbiguous {
                    wad,
                    champions: champions.into_iter().collect(),
                });
            } else if let Some(champion) = champions.into_iter().next()
                && !self.carried.contains_key(&champion)
            {
                self.carried.insert(champion.clone(), wad);
                self.guessed.insert(wad, champion);
            }
        }
    }

    /// Gives a WAD its role, and a champion's WAD its characters and units.
    fn place(&mut self, wad: WadId, sink: &mut Sink<'_>) -> Result<(), GameError> {
        let name = self.name(wad).clone();
        let (champion, extensions) = match self.guessed.get(&wad) {
            Some(champion) => {
                sink.raise(ChampionGuessed {
                    wad,
                    champion: champion.clone(),
                });
                (champion.clone(), GAME_SPELLING)
            }
            None => (name.stem().to_string(), rest(&name)),
        };
        let locale = locale_of(extensions);
        if let Some(&carried) = self.carried.get(&champion)
            && carried != wad
            && locale.is_none()
        {
            self.duplicate(wad, carried, champion, sink);
            return Ok(());
        }
        if self.packed.contains(&wad) {
            self.run.module.wads[wad].packed_locale.clone_from(&locale);
            sink.raise(LocalePacked {
                wad,
                champion: champion.clone(),
                locale: locale.unwrap_or_default(),
            });
        } else if let Some(locale) = locale {
            self.run.module.wads[wad].role = WadRole::Locale {
                champion: champion.clone(),
                locale: locale.clone(),
            };
            sink.raise(LocaleWad {
                wad,
                champion,
                locale,
            });
            return Ok(());
        }
        let slots = self.run.module.skin_bins(wad, &champion);
        if slots.is_empty() {
            sink.raise(WadNotCarried { wad });
            return Ok(());
        }
        self.run.module.wads[wad].role = WadRole::Champion {
            champion: champion.clone(),
        };
        self.group(wad, &champion, &slots, sink)
    }

    fn duplicate(&mut self, wad: WadId, carried: WadId, champion: String, sink: &mut Sink<'_>) {
        let m = &*self.run.module;
        let theirs: BTreeSet<WadHash> = m.wads[carried]
            .files
            .iter()
            .map(|&file| m.files[file].hash)
            .collect();
        let ours = m.wads[wad].files.iter().copied();
        let files: Vec<FileId> = ours
            .filter(|&file| !theirs.contains(&m.files[file].hash))
            .collect();
        if files.is_empty() {
            sink.raise(WadDuplicate {
                wad,
                champion,
                carried,
            });
        } else {
            sink.raise(WadDuplicateDiffers {
                wad,
                champion,
                carried,
                files,
            });
        }
    }

    /// The champion of a WAD and its sub-characters: the directories starting with the
    /// champion's name that ship a skin bin in the WAD, as the name tables or the WAD's own
    /// paths spell them.
    fn group(
        &mut self,
        wad: WadId,
        champion: &str,
        slots: &[(Slot, FileId)],
        sink: &mut Sink<'_>,
    ) -> Result<(), GameError> {
        let mut subs = self.run.env.names().characters_like(champion);
        let m = &*self.run.module;
        for &file in &m.wads[wad].files {
            if let Some(path) = m.spelled_path(file)
                && let Some((character, _)) = skin_paths::parse_skin_bin(path.as_str())
                && character.starts_with(champion)
            {
                subs.insert(character);
            }
        }
        subs.remove(champion);

        let id = self.add_character(wad, champion, CharKind::Champion, slots);
        let mut group = Group {
            wad,
            champion: id,
            characters: vec![id],
        };
        for sub in subs {
            let slots = self.run.module.skin_bins(wad, &sub);
            if slots.is_empty() {
                continue;
            }
            let kind = CharKind::Sub { champion: id };
            let character = self.add_character(wad, &sub, kind, &slots);
            group.characters.push(character);
            self.leave_to_game(character, sink)?;
        }
        self.run.module.groups.push(group);
        Ok(())
    }

    /// Marks a sub-character whose every skin bin is the game's byte for byte.
    fn leave_to_game(&mut self, character: CharId, sink: &mut Sink<'_>) -> Result<(), GameError> {
        let m = &mut *self.run.module;
        let game = self.run.env.game();
        let units: Vec<UnitId> = m.characters[character].units.clone();
        let mut equal = true;
        for &unit in &units {
            let file = &m.files[m.units[unit].file];
            equal = equal && game.holds(file.hash, file.size, file.content)?;
        }
        if equal {
            m.equal_to_game.insert(character);
            sink.raise(EqualToGame { character, units });
        }
        Ok(())
    }

    fn add_character(
        &mut self,
        wad: WadId,
        name: &str,
        kind: CharKind,
        slots: &[(Slot, FileId)],
    ) -> CharId {
        let m = &mut *self.run.module;
        let character = m.characters.push(Character {
            name: name.to_string(),
            spelled: name.to_string(),
            kind,
            wad,
            units: Vec::new(),
            root_bin: m.file_at(wad, path_hash(&skin_paths::root_bin(name))),
        });
        for &(slot, file) in slots {
            self.run.book.learn_path(&skin_paths::skin_bin(name, slot));
            let unit = m.units.push(SkinUnit {
                character,
                source_slot: slot,
                file,
                bin: None,
                loads: Vec::new(),
                entry: None,
                entry_name_slot: None,
            });
            m.characters[character].units.push(unit);
        }
        character
    }
}
