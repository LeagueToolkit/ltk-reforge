//! Whose the clips of a unit's banks are: for each bank unit of a representative unit's audio
//! block, the copies of its banks the archive ships, and for each clip of them whether the
//! game shipped its bytes or they are the mod's own.
//!
//! A clip the game shipped at any build is the game's, by the history's clips. Without a
//! history a clip is the game's when the game's bank at the same path holds it under the same
//! id, and an older clip of the game then passes as the mod's own.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use reforge_wwise::Bank;
use serde::ser::SerializeStruct as _;
use serde::{Deserialize, Serialize, Serializer};

use super::copies::Copies;
use super::findings::{BankUnreadable, UnreadBank};
use super::{Catalogs, FactsError};
use crate::diag::Sink;
use crate::env::Env;
use crate::fixups::FixedMod;
use crate::history::{Position, UnitHit};
use crate::load::Dating;
use crate::model::entity::{FileId, Mod, SkinUnit, UnitId, WadRole};
use crate::model::hash::skin_paths;
use crate::model::{
    AudioUnit, BankClip, BankRole, Dump, DumpCx, SideTable, SkinEntry, clean, game_bank_path,
    path_hash,
};

/// Whose a clip's bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "kebab-case")]
pub enum ClipOrigin {
    /// The game shipped these bytes, first at this build.
    Game {
        /// The first build that shipped them.
        first: Position,
    },
    /// The game's bank at the same path holds them under the same id. Judged without a
    /// history.
    GameBank,
    /// The mod's own.
    Own,
}

/// One clip of a bank the mod ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundClip {
    /// The id sounds name the clip by.
    pub id: u32,
    /// The size of its bytes.
    pub size: u64,
    /// Whose its bytes are.
    #[serde(flatten)]
    pub origin: ClipOrigin,
}

fn clip_counts<S: Serializer>(clips: &[SoundClip], s: S) -> Result<S::Ok, S::Error> {
    let own = clips.iter().filter(|c| c.origin == ClipOrigin::Own).count();
    let mut counts = s.serialize_struct("clips", 2)?;
    counts.serialize_field("total", &clips.len())?;
    counts.serialize_field("own", &own)?;
    counts.end()
}

/// One copy the archive ships of a bank a unit names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankFile {
    /// The path the unit names it by, lowercase.
    pub path: String,
    /// The file.
    pub file: FileId,
    /// The locale of the WAD it ships in: a locale WAD's, or the locale of a WAD packed whole
    /// as a locale's where the game localizes the path.
    pub locale: Option<String>,
    /// What the file holds.
    pub role: BankRole,
    /// The bank's version.
    pub version: u32,
    /// The id in its header.
    pub bank_id: u32,
    /// How many objects its hierarchy holds.
    pub objects: usize,
    /// Its clips, in stored order. A report holds how many there are and how many are the
    /// mod's own.
    #[serde(serialize_with = "clip_counts", skip_deserializing)]
    pub clips: Vec<SoundClip>,
    /// Why it does not read, when it does not.
    pub unreadable: Option<String>,
}

impl BankFile {
    /// The ids of the clips that are the mod's own.
    pub fn own_ids(&self) -> BTreeSet<u32> {
        let own = self.clips.iter().filter(|c| c.origin == ClipOrigin::Own);
        own.map(|clip| clip.id).collect()
    }

    /// How many clips are the mod's own.
    pub fn own(&self) -> usize {
        let own = self.clips.iter().filter(|c| c.origin == ClipOrigin::Own);
        own.count()
    }
}

/// A bank unit of a skin entry's audio block, with the copies of its banks the archive ships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundUnit {
    /// The unit as the entry declares it.
    #[serde(flatten)]
    pub unit: AudioUnit,
    /// The copies of its banks the archive ships, in the order of its paths, then of the WADs.
    pub files: Vec<BankFile>,
}

impl SoundUnit {
    /// How many clips of its banks are the mod's own.
    pub fn own(&self) -> usize {
        self.files.iter().map(BankFile::own).sum()
    }
}

/// A bank unit of the game's skin that the mod's audio block leaves out, which the game's skin
/// held when the mod was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeftOut {
    /// The game unit's name.
    pub name: String,
    /// The first build the game's skin held it.
    pub first: Position,
}

/// The audio of one skin unit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitSounds {
    /// The tags of its audio block.
    pub tags: Vec<String>,
    /// Its bank units.
    pub units: Vec<SoundUnit>,
    /// The bank units of the game's skin at the unit's own slot that its block leaves out
    /// (#ADR-0063), in the game's order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub left_out: Vec<LeftOut>,
}

impl UnitSounds {
    /// How many clips of its banks are the mod's own.
    pub fn own(&self) -> usize {
        self.units.iter().map(SoundUnit::own).sum()
    }
}

/// What the sound facts are found from, besides the mod.
pub struct SoundInputs<'a> {
    /// The game and the history.
    pub env: &'a Env,
    /// Which units are copies of which.
    pub copies: &'a Copies,
    /// The game's skins of each character.
    pub catalogs: &'a Catalogs,
    /// The mod's dating.
    pub dating: &'a Dating,
}

/// The audio of each representative unit whose entry holds an audio block.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(from = "SoundsRecord", into = "SoundsRecord")]
pub struct Sounds {
    units: SideTable<SkinUnit, UnitSounds>,
}

/// The sounds as a snapshot keeps them: beside each unit's as the report writes them, the
/// clips of each bank file, which the report counts.
#[derive(Serialize, Deserialize)]
struct SoundsRecord {
    units: SideTable<SkinUnit, UnitSounds>,
    /// By unit, by bank unit, by file.
    clips: SideTable<SkinUnit, Vec<Vec<Vec<SoundClip>>>>,
}

impl From<Sounds> for SoundsRecord {
    fn from(sounds: Sounds) -> Self {
        let mut clips = SideTable::default();
        for (unit, of_unit) in sounds.units.iter() {
            let files = |sound: &SoundUnit| sound.files.iter().map(|f| f.clips.clone()).collect();
            clips.insert(unit, of_unit.units.iter().map(files).collect());
        }
        Self {
            units: sounds.units,
            clips,
        }
    }
}

impl From<SoundsRecord> for Sounds {
    fn from(record: SoundsRecord) -> Self {
        let mut units = record.units;
        for (unit, of_unit) in record.clips.iter() {
            let Some(sounds) = units.get_mut(unit) else {
                continue;
            };
            for (sound, files) in sounds.units.iter_mut().zip(of_unit) {
                for (file, clips) in sound.files.iter_mut().zip(files) {
                    file.clips.clone_from(clips);
                }
            }
        }
        Self { units }
    }
}

/// The judging of one bank's clips.
struct Judge<'a> {
    env: &'a Env,
    /// The clips of the game's bank at the unit's path, asked once and only without a history.
    game: Option<Option<std::sync::Arc<[BankClip]>>>,
}

impl Judge<'_> {
    fn clip(&mut self, path: &str, clip: BankClip) -> Result<ClipOrigin, FactsError> {
        if self.env.has_history() {
            let life = self.env.history().clip(clip.key)?;
            return Ok(life.map_or(ClipOrigin::Own, |life| ClipOrigin::Game {
                first: life.first,
            }));
        }
        if self.game.is_none() {
            let at = path_hash(&game_bank_path(path));
            self.game = Some(self.env.game().bank_clips(at)?);
        }
        let game = self.game.as_ref().and_then(Option::as_ref);
        let held = game.is_some_and(|clips| {
            let mut clips = clips.iter();
            clips.any(|held| held.id == clip.id && held.key == clip.key && held.size == clip.size)
        });
        Ok(match held {
            true => ClipOrigin::GameBank,
            false => ClipOrigin::Own,
        })
    }
}

impl Sounds {
    /// Reads the banks each representative unit's audio block names, judges their clips, and
    /// finds the game's units the block leaves out.
    pub fn find(fixed: &FixedMod, inputs: &SoundInputs<'_>) -> Result<Self, FactsError> {
        let m: &Mod = fixed;
        let env = inputs.env;
        let mut out = Self::default();
        for unit in inputs.copies.representatives() {
            let Some(entry) = m.units[unit].entry else {
                continue;
            };
            let block = SkinEntry::read(&m.objects[entry].value).audio();
            if block.is_empty() {
                continue;
            }
            let mut sounds = UnitSounds {
                tags: block.tags,
                units: Vec::new(),
                left_out: Vec::new(),
            };
            for declared in block.units {
                let mut files = Vec::new();
                for path in &declared.paths {
                    Self::copies_of(m, env, path, &mut files)?;
                }
                let unit = declared;
                sounds.units.push(SoundUnit { unit, files });
            }
            sounds.left_out = Self::left_out(m, inputs, unit, &sounds)?;
            out.units.insert(unit, sounds);
        }
        Ok(out)
    }

    /// The game's bank units at the unit's own slot that its audio block leaves out, where the
    /// game's skin held them when the mod was made (#ADR-0063). None for a mod the history does
    /// not date, and for a block that names none of the game's units there: it was made from
    /// another skin's, a premium skin's pasted (#MOD-ARTIFACT-0002). A game unit is left out
    /// only when the block holds no unit of its kind, voice or effects: one that does swapped
    /// it for another skin's, whose game banks a port does not carry.
    fn left_out(
        m: &Mod,
        inputs: &SoundInputs<'_>,
        unit: UnitId,
        sounds: &UnitSounds,
    ) -> Result<Vec<LeftOut>, FactsError> {
        let env = inputs.env;
        let (Some(made), true) = (inputs.dating.position(), env.has_history()) else {
            return Ok(Vec::new());
        };
        let node = &m.units[unit];
        let skin = inputs.catalogs.of(node.character);
        let Some(skin) = skin.and_then(|catalog| catalog.skin(node.source_slot)) else {
            return Ok(Vec::new());
        };
        let named = |name: &str| {
            let mut mine = sounds.units.iter();
            mine.any(|held| held.unit.name.eq_ignore_ascii_case(name))
        };
        let game = &skin.audio.units;
        if !game.iter().any(|held| named(&held.name)) {
            return Ok(Vec::new());
        }
        let character = &m.characters[node.character].name;
        let bin = path_hash(&skin_paths::skin_bin(character, node.source_slot));
        // The skin's unit of that name loaded the bank at the build the mod was made at.
        let held_then = |hit: &UnitHit, name: &str| {
            let life = hit.life;
            hit.bin == bin && hit.unit == name && life.first <= made && made <= life.last
        };
        let kind_held = |voice: bool| sounds.units.iter().any(|mine| mine.unit.voice == voice);
        let left = game
            .iter()
            .filter(|held| !named(&held.name) && !kind_held(held.voice));
        let mut out = Vec::new();
        for held in left {
            let mut firsts = Vec::new();
            for path in &held.paths {
                let hits = env
                    .history()
                    .units_loading(path_hash(&game_bank_path(path)))?;
                let hits = hits.iter().filter(|hit| held_then(hit, &held.name));
                firsts.extend(hits.map(|hit| hit.life.first));
            }
            if let Some(first) = firsts.into_iter().min() {
                let name = held.name.clone();
                out.push(LeftOut { name, first });
            }
        }
        Ok(out)
    }

    /// The copies the archive ships of the bank at a path, WAD by WAD.
    fn copies_of(
        m: &Mod,
        env: &Env,
        path: &str,
        files: &mut Vec<BankFile>,
    ) -> Result<(), FactsError> {
        let lower = path.to_ascii_lowercase();
        let hash = path_hash(&lower);
        let mut judge = Judge { env, game: None };
        for (wad, node) in m.wads.iter() {
            let Some(file) = m.file_at(wad, hash) else {
                continue;
            };
            let locale = match &node.role {
                WadRole::Locale { locale, .. } => Some(locale.clone()),
                // #MOD-ARTIFACT-0010: the files the game localizes keep the locale.
                _ => node.packed_locale.clone().filter(|locale| {
                    let game = env.game().wad_of(hash);
                    game.is_some_and(|wad| wad.locale().is_some())
                        || lower.contains(&format!("/{locale}/"))
                }),
            };
            let mut bank = BankFile {
                path: lower.clone(),
                file,
                locale,
                role: BankRole::Stub,
                version: 0,
                bank_id: 0,
                objects: 0,
                clips: Vec::new(),
                unreadable: None,
            };
            match Bank::read(&m.files[file].bytes) {
                Ok(read) => {
                    bank.role = BankRole::of(&read);
                    bank.version = read.version;
                    bank.bank_id = read.id();
                    bank.objects = read.objects.len();
                    for wem in &read.media {
                        let clip = BankClip::of(wem.id, &wem.bytes);
                        bank.clips.push(SoundClip {
                            id: clip.id,
                            size: clip.size,
                            origin: judge.clip(&lower, clip)?,
                        });
                    }
                }
                Err(error) => bank.unreadable = Some(error.to_string()),
            }
            files.push(bank);
        }
        Ok(())
    }

    /// The audio of a unit.
    pub fn of(&self, unit: UnitId) -> Option<&UnitSounds> {
        self.units.get(unit)
    }

    /// Every unit with audio, in unit order.
    pub fn iter(&self) -> impl Iterator<Item = (UnitId, &UnitSounds)> {
        self.units.iter()
    }

    /// Raises the banks that do not read.
    pub fn report(&self, sink: &mut Sink<'_>) {
        for (unit, sounds) in self.iter() {
            let files = sounds.units.iter().flat_map(|unit| &unit.files);
            let mut banks: Vec<UnreadBank> = Vec::new();
            for file in files {
                let Some(reason) = &file.unreadable else {
                    continue;
                };
                if !banks.iter().any(|bank| bank.file == file.file) {
                    banks.push(UnreadBank {
                        file: file.file,
                        reason: reason.clone(),
                    });
                }
            }
            if !banks.is_empty() {
                sink.raise(BankUnreadable { unit, banks });
            }
        }
    }
}

impl Dump for Sounds {
    fn dump(&self, _: &DumpCx<'_>) -> String {
        let mut out = String::new();
        for (unit, sounds) in self.iter() {
            let tags = sounds.tags.iter().map(|tag| clean(tag).into_owned());
            let tags: Vec<String> = tags.collect();
            let _ = writeln!(out, "sounds {unit} tags [{}]", tags.join(", "));
            for bank_unit in &sounds.units {
                let voice = if bank_unit.unit.voice { " voice" } else { "" };
                let _ = writeln!(
                    out,
                    "   unit {}{voice}: {} banks named, {} shipped, {} own clips",
                    clean(&bank_unit.unit.name),
                    bank_unit.unit.paths.len(),
                    bank_unit.files.len(),
                    bank_unit.own()
                );
                for file in &bank_unit.files {
                    let locale = file.locale.as_deref();
                    let locale = locale.map_or(String::new(), |locale| format!(" [{locale}]"));
                    let unreadable = if file.unreadable.is_some() {
                        " unreadable"
                    } else {
                        ""
                    };
                    let _ = writeln!(
                        out,
                        "      {} v{} {}{locale}: {} objects, {} clips, {} own{unreadable}",
                        file.role.as_str(),
                        file.version,
                        file.file,
                        file.objects,
                        file.clips.len(),
                        file.own()
                    );
                }
            }
            for left in &sounds.left_out {
                let name = clean(&left.name);
                let _ = writeln!(
                    out,
                    "   left out: the game's unit {name}, since {}",
                    left.first
                );
            }
        }
        out
    }
}
