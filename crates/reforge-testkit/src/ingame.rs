//! Test mods for checks in game: a champion's base skin bin as the game holds it, changed by one
//! case, as the bytes of a `.fantome` that installs it over the game's own. The examples of
//! `reforge` that build them read the game; this module takes it as a [`Game`].

use ltk_meta::property::values;
use ltk_meta::{Bin, BinObject, PropertyValueEnum};
use reforge::game::Game;
use reforge::model::value::as_str;
use reforge::model::walk::{Fields, structs_mut};
use reforge::model::{WadName, bin, name_hash, path_hash};

use crate::archive::{DEFLATED, zip};

/// A game bin read whole: its objects in file order and the bins it links.
#[derive(Debug, Clone)]
pub struct GameBin {
    /// Its objects, in file order.
    pub objects: Vec<BinObject>,
    /// The paths of the bins it links, as it spells them.
    pub links: Vec<String>,
}

impl GameBin {
    /// The bin at `path` in the game.
    ///
    /// # Panics
    ///
    /// When the game holds no chunk at `path`, or the chunk does not read as a bin.
    pub fn read(game: &dyn Game, path: &str) -> Self {
        let bytes = game
            .read(path_hash(path))
            .unwrap_or_else(|error| panic!("{path} does not read from the game: {error}"))
            .unwrap_or_else(|| panic!("the game holds no {path}"));
        let parsed =
            bin::parse(&bytes).unwrap_or_else(|error| panic!("{path} does not parse: {error}"));
        Self {
            objects: parsed.objects.into_iter().map(|o| o.value).collect(),
            links: parsed.dependencies,
        }
    }

    /// The object under an entry name.
    ///
    /// # Panics
    ///
    /// When the bin holds no object under it.
    pub fn object(&self, entry_name: &str) -> &BinObject {
        let hash = name_hash(entry_name);
        self.objects
            .iter()
            .find(|object| object.path_hash == hash)
            .unwrap_or_else(|| panic!("no object {entry_name} in the bin"))
    }

    /// The object under an entry name, to change.
    ///
    /// # Panics
    ///
    /// When the bin holds no object under it.
    pub fn object_mut(&mut self, entry_name: &str) -> &mut BinObject {
        let hash = name_hash(entry_name);
        self.objects
            .iter_mut()
            .find(|object| object.path_hash == hash)
            .unwrap_or_else(|| panic!("no object {entry_name} in the bin"))
    }

    /// Changes the emitter named `emitter` of the system under the entry name `system`.
    ///
    /// # Panics
    ///
    /// When the bin holds no object under `system`, or the system holds no emitter of that name
    /// or more than one.
    pub fn change_emitter(
        &mut self,
        system: &str,
        emitter: &str,
        mut change: impl FnMut(&mut Fields),
    ) {
        let class = name_hash("VfxEmitterDefinitionData");
        let mut found = 0;
        structs_mut(self.object_mut(system), |mut s| {
            let name = s.fields().get(&name_hash("emitterName")).and_then(as_str);
            if s.class() == class && name == Some(emitter) {
                found += 1;
                change(s.fields_mut());
            }
        });
        assert_eq!(found, 1, "{system} holds {found} emitters named {emitter}");
    }

    /// Changes every emitter the system under the entry name `system` lists in `list`:
    /// `complexEmitterDefinitionData` or `simpleEmitterDefinitionData`. Returns how many.
    ///
    /// # Panics
    ///
    /// When the bin holds no object under `system`, or the system's `list` is no list of
    /// emitters.
    pub fn change_emitters(
        &mut self,
        system: &str,
        list: &str,
        mut change: impl FnMut(&mut Fields),
    ) -> usize {
        let emitters = match self.object_mut(system).properties.get_mut(&name_hash(list)) {
            Some(PropertyValueEnum::Container(emitters)) => emitters,
            None => return 0,
            Some(other) => panic!("{system}'s {list} is no list: {other:?}"),
        };
        for i in 0..emitters.len() {
            let mut slot = emitters
                .slot(i)
                .unwrap_or_else(|| panic!("{system}'s {list} has no item {i}"));
            let emitter = slot
                .get_mut::<values::Struct>()
                .unwrap_or_else(|| panic!("item {i} of {system}'s {list} is no pointer"));
            change(&mut emitter.properties);
        }
        emitters.len()
    }

    /// The bin as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        bin::write(&Bin::new(self.objects.clone(), self.links.clone()))
    }
}

/// A champion's base skin as the game holds it: its bin and the WAD the client reads it from.
#[derive(Debug, Clone)]
pub struct BaseSkin {
    /// The champion, lower case.
    pub champion: String,
    /// The bin's path, `data/characters/<champion>/skins/skin0.bin`.
    pub path: String,
    /// The WAD the bin is read from.
    pub wad: WadName,
    /// The bin.
    pub bin: GameBin,
}

impl BaseSkin {
    /// The base skin of a champion of the game.
    ///
    /// # Panics
    ///
    /// When the game holds no base skin bin for the champion, or it does not read.
    pub fn read(game: &dyn Game, champion: &str) -> Self {
        let champion = champion.to_lowercase();
        let path = format!("data/characters/{champion}/skins/skin0.bin");
        let bin = GameBin::read(game, &path);
        let wad = game
            .wads(path_hash(&path))
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("no WAD of the game holds {path}"));
        Self {
            champion,
            path,
            wad,
            bin,
        }
    }

    /// The entry name of the skin's own object, `Characters/<champion>/Skins/Skin0`.
    pub fn entry_name(&self) -> String {
        format!("Characters/{}/Skins/Skin0", self.champion)
    }

    /// A test mod that replaces this skin's bin with `bytes`. Its `info.json` names it `name`
    /// and holds what the case asks as its description.
    ///
    /// # Panics
    ///
    /// When the `info.json` does not write, which no name or text makes it do.
    pub fn fantome(&self, name: &str, asks: &str, bytes: &[u8]) -> Vec<u8> {
        let info = serde_json::json!({
            "Name": name,
            "Author": "reforge",
            "Version": "1.0.0",
            "Description": asks,
        });
        let info = serde_json::to_vec_pretty(&info)
            .unwrap_or_else(|error| panic!("the info of {name} does not write: {error}"));
        let entry = format!("WAD/{}/{}", self.wad, self.path);
        zip(&[
            ("META/info.json", info.as_slice(), DEFLATED),
            (entry.as_str(), bytes, DEFLATED),
        ])
    }
}
