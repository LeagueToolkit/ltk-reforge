//! The audio of a port: the mod's own clips in bank units after the game's, voice in layers of
//! its own (#ADR-0028). The game's audio block at the target stays as it is.
//!
//! A media bank or a voice package of the mod with own clips becomes one bank unit, in one of
//! two shapes:
//!
//! - **Over the game's.** The bank sits at a path the game's units at the target load. Its
//!   clips carry the game's ids, so the game's events play them (#GAME-BEHAVIOR-0012). The
//!   unit holds a media bank of the own clips and an events bank with no object.
//! - **Of its own.** The bank sits at any other path. The events that reach an own clip are
//!   cut from the game's events bank at the unit's events path, else from the mod's own of the
//!   client's version (#ADR-0030), the first that plays one (#ADR-0075), else from the game's
//!   at the target build at a path where one of the unit's older events banks was the game's
//!   (#ADR-0084). Every other object of the source stays (#GAME-BEHAVIOR-0017). A kept event is
//!   renamed under the mod's tag, token by token, and the tag goes after the game's
//!   (#GAME-BEHAVIOR-0011).
//!
//! Every bank made takes its header from an events bank of the game's units at the target, so
//! it is of the version the client loads (#GAME-BEHAVIOR-0015), and its id is the hash of its
//! unit's name (#GAME-BEHAVIOR-0014). A clip the game shipped is never carried. A media bank
//! made from a mod's bank carries the buses that bank defines and the game's init bank does not
//! (#ADR-0064).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use reforge_wwise::{Bank, CLIENT_VERSION, Cut, Object, Wem, id_of};
use serde::{Deserialize, Serialize};

use super::findings::{
    AudioCarried, AudioNotCarried, AudioUnitsLeftOut, CarriedBank, CarriedOnce, Carry,
    LiteralsRenamed, NotCarried,
};
use super::layout::Layout;
use super::plan::{Objects, PlanError, PortInputs};
use super::reach::{Fate, Reach};
use super::shipping::Shipping;
use crate::diag::Sink;
use crate::facts::{BankFile, SoundUnit, Sounds, UnitSounds};
use crate::load::provenance::BytesOrigin;
use crate::model::entity::{FileId, Mod, UnitId};
use crate::model::{
    AudioBlock, BankRole, ChunkPath, Dump, DumpCx, EventsBank, INIT_BANK, Layer, Spell, WadHash,
    WadName, clean, game_bank_path, path_hash, value,
};

/// One clip of a bank: the bank's path and the clip's id in it.
pub type ClipRef = (String, u32);

/// The clips of each unit's banks that are the mod's own. `Shipping` reads it: a unit that
/// names an own clip no other unit carries is not the game's own skin (#ADR-0020), and a
/// chroma with an own clip of its own does not fold (#ADR-0018).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnClips {
    units: BTreeMap<UnitId, BTreeSet<ClipRef>>,
}

impl OwnClips {
    /// The own clips of no unit.
    pub fn none() -> Self {
        Self::default()
    }

    /// The own clips of each unit, from the facts of its banks.
    pub fn found(sounds: &Sounds) -> Self {
        let mut units = BTreeMap::new();
        for (unit, sounds) in sounds.iter() {
            let files = sounds.units.iter().flat_map(|unit| &unit.files);
            let own = files.flat_map(|file| {
                let ids = file.own_ids().into_iter();
                ids.map(move |id| (file.path.clone(), id))
            });
            let own: BTreeSet<ClipRef> = own.collect();
            if !own.is_empty() {
                units.insert(unit, own);
            }
        }
        Self { units }
    }

    /// The own clips a unit's audio names.
    pub fn of(&self, unit: UnitId) -> BTreeSet<ClipRef> {
        self.units.get(&unit).cloned().unwrap_or_default()
    }
}

/// A bank unit the port adds to a skin entry's audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankUnit {
    /// The unit's name.
    pub name: String,
    /// The paths of its banks, as the port ships them.
    pub banks: Vec<ChunkPath>,
    /// Its events, renamed where the port renamed them.
    pub events: Vec<String>,
    /// Whether it is voice.
    pub voice: bool,
    /// The layer its banks and its record go to.
    pub layer: Layer,
    /// The tag its events are named under, for a unit with events of its own.
    pub tag: Option<String>,
    /// The game's unit it takes the place of (#MOD-ARTIFACT-0028).
    pub replaces: Option<String>,
}

/// The audio one skin unit's port adds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitAudio {
    /// The skin unit.
    pub unit: UnitId,
    /// The bank units added after the game's, the base layer's and the voice layers'.
    pub units: Vec<BankUnit>,
    /// The tags the base layer adds after the game's.
    pub tags: Vec<String>,
    /// The events of the base layer's units that were renamed: the name in the mod,
    /// lowercase, and the name in the port.
    pub renamed: BTreeMap<String, String>,
    /// The game's units at the target that the block leaves out, as the mod's own block does
    /// (#ADR-0063).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub left_out: Vec<String>,
}

/// The bank of a made bank read from a report, which holds none.
fn no_bank() -> Bank {
    Bank::new_wpk(Vec::new())
}

/// A bank the port makes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MadeBank {
    /// Where it ships.
    pub path: ChunkPath,
    /// The WAD it ships in.
    pub wad: WadName,
    /// The layer it ships in.
    pub layer: Layer,
    /// The skin unit whose audio names it.
    pub unit: UnitId,
    /// The bank unit that loads it.
    pub bank_unit: String,
    /// The bank of the mod its clips are from.
    pub from: FileId,
    /// How many clips it holds.
    pub clips: usize,
    /// How many objects it holds.
    pub objects: usize,
    /// The bank, as the build writes it. A snapshot keeps it in the store (#ADR-0094).
    #[serde(skip, default = "no_bank")]
    pub bank: Bank,
}

/// The audio of a port: the bank units and tags each skin unit adds, and the banks made.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioPlan {
    /// What each skin unit adds.
    pub units: Vec<UnitAudio>,
    /// The banks made, each with its place.
    pub banks: Vec<MadeBank>,
}

/// The tags an event of a bank unit may be named under that the target's block does not
/// hold, lowercase: the mod's tags beyond the game's, and the one the unit's name spells
/// (`Jinx_Skin04_SFX` spells `JinxSkin04`).
fn premium_tags(mod_tags: &[String], game_tags: &[String], unit: &str) -> BTreeSet<String> {
    let lower = |tags: &[String]| -> BTreeSet<String> {
        tags.iter().map(|tag| tag.to_ascii_lowercase()).collect()
    };
    let game = lower(game_tags);
    let mut out: BTreeSet<String> = lower(mod_tags).difference(&game).cloned().collect();
    let mut parts = unit.split('_');
    if let (Some(first), Some(second)) = (parts.next(), parts.next()) {
        let spelled = format!("{first}{second}").to_ascii_lowercase();
        if !game.contains(&spelled) {
            out.insert(spelled);
        }
    }
    out
}

/// An event's name with every token that is one of `premium` replaced by `tag`. None when no
/// token is.
fn retag(name: &str, premium: &BTreeSet<String>, tag: &str) -> Option<String> {
    let mut changed = false;
    let tokens = name.split('_').map(|token| {
        if premium.contains(&token.to_ascii_lowercase()) {
            changed = true;
            tag
        } else {
            token
        }
    });
    let renamed = tokens.collect::<Vec<&str>>().join("_");
    changed.then_some(renamed)
}

/// The events of a unit of its own: the cut, and the names the unit lists.
struct OwnEvents {
    cut: Cut,
    names: Vec<String>,
    carry: Carry,
}

/// The planning of one skin unit's audio.
struct Planning<'a> {
    inputs: &'a PortInputs<'a>,
    unit: UnitId,
    sounds: &'a UnitSounds,
    /// The game's audio block at the target.
    game: &'a AudioBlock,
    /// The paths of the banks the game's units at the target load, lowercase.
    loaded: BTreeSet<String>,
    /// The header every bank made takes.
    header: Vec<u8>,
    /// The ids of the buses the game's init bank defines; none when the game has no init bank.
    game_buses: Option<&'a BTreeSet<u32>>,
    /// The unit's directory.
    dir: String,
    tag_id: String,
    base_tag: String,
    mod_tag: String,
    /// How many units of their own were made.
    own_units: usize,
    /// How many units were made per directory and role.
    roles: BTreeMap<(String, &'static str), usize>,
    out: UnitAudio,
    banks: Vec<MadeBank>,
    carried: Vec<CarriedBank>,
    once: Vec<CarriedOnce>,
    left: Vec<NotCarried>,
}

impl Planning<'_> {
    /// The game's events bank at the bank unit's events path.
    fn game_events(&self, unit: &SoundUnit) -> Result<Option<Bank>, PlanError> {
        let paths = unit.unit.paths.iter().map(|path| game_bank_path(path));
        let Some(path) = paths.into_iter().find(|p| p.ends_with("_events.bnk")) else {
            return Ok(None);
        };
        let bank = self.inputs.env.game().events_bank(path_hash(&path))?;
        Ok(bank.map(|bank| bank.bank()))
    }

    /// The mod's own events bank of the unit for its own clips, of the version the client loads
    /// (#GAME-BEHAVIOR-0015): the first whose events reach one of the clips, else the first
    /// (#ADR-0075).
    fn mod_events(&self, unit: &SoundUnit, own: &BTreeSet<u32>) -> Option<Bank> {
        let m: &Mod = self.inputs.fixed;
        let events = unit
            .files
            .iter()
            .filter(|file| file.role == BankRole::Events && file.version == CLIENT_VERSION);
        let banks = events.filter_map(|file| Bank::read(&m.bytes(file.file)).ok());
        let mut banks: Vec<Bank> = banks.collect();
        let playing = banks
            .iter()
            .position(|bank| !bank.cut_to(own).events.is_empty());
        match playing {
            Some(at) => Some(banks.swap_remove(at)),
            None => banks.into_iter().next(),
        }
    }

    /// The game's events bank at the target build at a path where one of the unit's events
    /// banks of another version than the client's was the game's (#ADR-0084): the first whose
    /// events reach one of the clips, else the first. With the path.
    fn successor_events(
        &self,
        unit: &SoundUnit,
        own: &BTreeSet<u32>,
    ) -> Result<Option<(Bank, WadHash)>, PlanError> {
        let older = unit
            .files
            .iter()
            .filter(|file| file.role == BankRole::Events && file.version != CLIENT_VERSION);
        let mut banks = Vec::new();
        for file in older {
            let origin = self.inputs.files.fact(file.file).map(|fact| &fact.origin);
            let Some(BytesOrigin::History { path, .. }) = origin else {
                continue;
            };
            if let Some(bank) = self.inputs.env.game().events_bank(*path)? {
                banks.push((bank.bank(), *path));
            }
        }
        let playing = banks
            .iter()
            .position(|(bank, _)| !bank.cut_to(own).events.is_empty());
        Ok(match playing {
            Some(at) => Some(banks.swap_remove(at)),
            None => banks.into_iter().next(),
        })
    }

    /// The events of a unit of its own, cut to the own clips and renamed. None when no events
    /// bank plays the clips, which is noted.
    fn own_events(
        &mut self,
        unit: &SoundUnit,
        own: &BTreeSet<u32>,
        (name, layer): (&str, &Layer),
    ) -> Result<Option<OwnEvents>, PlanError> {
        let source_name = &unit.unit.name;
        let mut game_source = self.game_events(unit)?;
        let of_mod = self.mod_events(unit, own);
        let mut successor = None;
        if game_source.is_none()
            && of_mod.is_none()
            && let Some((bank, path)) = self.successor_events(unit, own)?
        {
            // #ADR-0084: the successor stands in for the path's bank from here on.
            game_source = Some(bank);
            successor = Some(self.inputs.names.path_or_hex(path));
        }
        let Some(mut source) = game_source.clone().or_else(|| of_mod.clone()) else {
            self.left.push(NotCarried::NoEventsBank {
                clips: own.len(),
                source: source_name.clone(),
            });
            return Ok(None);
        };
        let mut cut = source.cut_to(own);
        let mut from_mod = game_source.is_none();
        // #ADR-0030: a bank the author added, which no event of the game's plays.
        if cut.events.is_empty()
            && !from_mod
            && !self.inputs.options.game_events_only
            && let Some(theirs) = of_mod
        {
            let of_mod = theirs.cut_to(own);
            if !of_mod.events.is_empty() {
                (source, cut, from_mod) = (theirs, of_mod, true);
            }
        }
        if cut.events.is_empty() {
            self.left.push(NotCarried::NoEvent {
                clips: own.len(),
                source: source_name.clone(),
            });
            return Ok(None);
        }
        // An event is named by the lists of the mod's units. A kept event is renamed by token.
        let listed = self.sounds.units.iter().flat_map(|unit| &unit.unit.events);
        let known: BTreeMap<u32, &String> = listed.map(|name| (id_of(name), name)).collect();
        let premium = premium_tags(&self.sounds.tags, &self.game.tags, source_name);
        let mut renamed: BTreeMap<u32, u32> = BTreeMap::new();
        let (mut names, mut unnamed) = (Vec::new(), Vec::new());
        for id in &cut.events {
            let Some(listed) = known.get(id) else {
                unnamed.push(*id);
                continue;
            };
            match retag(listed, &premium, &self.mod_tag) {
                Some(new) => {
                    renamed.insert(*id, id_of(&new));
                    // #ADR-0028: a literal name never names a layer's renamed event.
                    if *layer == Layer::Base {
                        let old = listed.to_ascii_lowercase();
                        self.out.renamed.insert(old, new.clone());
                    }
                    names.push(new);
                }
                None => names.push((*listed).clone()),
            }
        }
        // #GAME-BEHAVIOR-0014: a play action names its bank by the id in the header.
        let events_id = id_of(&format!("{}_events", name.to_ascii_lowercase()));
        let source_id = source.id();
        for object in &mut cut.objects {
            if let Some(new) = renamed.get(&object.id) {
                object.id = *new;
            }
            if object.play_bank() == Some(source_id) {
                object.set_play_bank(events_id);
            }
        }
        let shared = match (&game_source, from_mod) {
            (Some(game), true) => {
                let kept = cut.events.iter();
                let shared =
                    kept.filter(|id| !renamed.contains_key(id) && game.object(**id).is_some());
                shared.count()
            }
            _ => 0,
        };
        let carry = Carry::Own {
            tag: self.mod_tag.clone(),
            events: cut.events.len(),
            from_mod,
            shared,
            silent: cut.silent,
            unnamed,
            successor,
        };
        Ok(Some(OwnEvents { cut, names, carry }))
    }

    /// The buses among a mod's bank's objects that the game's init bank does not define
    /// (#GAME-BEHAVIOR-0025). A copy of a game bus is left out (#ADR-0064). None when the game
    /// has no init bank to tell them by.
    fn own_buses(&self, objects: &[Object]) -> Vec<Object> {
        let Some(game) = self.game_buses else {
            return Vec::new();
        };
        let own = objects
            .iter()
            .filter(|o| o.is_bus() && !game.contains(&o.id));
        own.cloned().collect()
    }

    /// A bank of the game's header under the id of a name.
    fn bank(&self, stem: &str, hirc: bool) -> Bank {
        let mut bank = Bank::new_bnk(self.header.clone(), hirc);
        bank.set_id(id_of(stem));
        bank
    }

    /// Plans the unit for one copy of a media bank or a package with own clips.
    fn file(
        &mut self,
        layout: &mut Layout,
        unit: &SoundUnit,
        file: &BankFile,
        replaces: Option<&String>,
    ) -> Result<(), PlanError> {
        let m: &Mod = self.inputs.fixed;
        let own = file.own_ids();
        if own.is_empty() {
            return Ok(());
        }
        let source_name = &unit.unit.name;
        let locale = file.locale.as_deref();
        if let Some(locale) = locale {
            // The champion WAD's copy, when it has own clips, is carried from there for
            // every locale, and a copy with other own clips is not (#ADR-0074).
            let mut copies = unit.files.iter();
            let champion = copies
                .find(|copy| copy.locale.is_none() && copy.path == file.path && copy.own() > 0);
            match champion {
                Some(copy) if copy.own_ids() == own => {
                    self.once.push(CarriedOnce {
                        source: source_name.clone(),
                        locale: locale.to_string(),
                    });
                    return Ok(());
                }
                Some(_) => {
                    self.left.push(NotCarried::LocaleDiffers {
                        clips: own.len(),
                        source: source_name.clone(),
                        locale: locale.to_string(),
                    });
                    return Ok(());
                }
                None => {}
            }
        }
        let overlay = self.loaded.contains(&game_bank_path(&file.path));
        let Ok(read) = Bank::read(&m.bytes(file.file)) else {
            return Ok(());
        };
        let buses = self.own_buses(&read.objects);
        let media = read.media.into_iter().filter(|wem| own.contains(&wem.id));
        let media: Vec<Wem> = media.collect();
        let voice = file.role == BankRole::Package || unit.unit.voice;
        // #ADR-0073: a locale WAD's bank of a unit that is not voice goes where the champion
        // WAD's would.
        let locale = locale.filter(|_| voice);
        let layer = match voice {
            true => Layer::Voice(locale.map(str::to_string)),
            false => Layer::Base,
        };
        let character = &m.characters[m.units[self.unit].character];
        let (wad, suffix) = match locale {
            Some(locale) => {
                let wad = m.wads[m.files[file.file].wad].name.clone();
                (wad, format!("_{}", Layout::unit_part(locale)))
            }
            None => (m.wads[character.wad].name.clone(), String::new()),
        };
        let audio_dir = Layout::audio_dir(&self.dir, locale);

        let tag_id = &self.tag_id;
        let (name, events) = if overlay {
            let name = format!("{source_name}_Mod{tag_id}{suffix}");
            (Layout::unit_part(&name), None)
        } else {
            let role = if voice { "VO" } else { "SFX" };
            let nth = match self.own_units {
                0 => String::new(),
                n => (n + 1).to_string(),
            };
            let name = format!("{}_Mod{tag_id}_{role}{nth}{suffix}", self.base_tag);
            let name = Layout::unit_part(&name);
            let Some(events) = self.own_events(unit, &own, (&name, &layer))? else {
                return Ok(());
            };
            self.own_units += 1;
            (name, Some(events))
        };

        // The ids are of the unit's name, which no other mod and no other skin has. The files
        // are named by their role: the directory says whose they are.
        let stem = name.to_ascii_lowercase();
        let role = if voice { "vo" } else { "sfx" };
        let count = self.roles.entry((audio_dir.clone(), role)).or_default();
        *count += 1;
        let file_stem = match *count {
            1 => role.to_string(),
            n => format!("{role}{n}"),
        };
        let mut place = |part: &str| layout.place_bank(&audio_dir, &file_stem, part);
        let (Some(audio_at), Some(events_at)) = (place("audio.bnk"), place("events.bnk")) else {
            return Ok(());
        };
        let mut paths = vec![audio_at.clone(), events_at.clone()];
        let mut made = Vec::new();
        let clips = media.len();
        if file.role == BankRole::Package {
            // The package, and a bank with a header alone beside it (#GAME-BEHAVIOR-0015).
            let Some(package_at) = place("audio.wpk") else {
                return Ok(());
            };
            made.push((audio_at, self.bank(&format!("{stem}_audio"), false)));
            made.push((package_at.clone(), Bank::new_wpk(media)));
            paths.push(package_at);
        } else {
            let mut audio = self.bank(&format!("{stem}_audio"), !buses.is_empty());
            audio.media = media;
            audio.objects = buses;
            made.push((audio_at, audio));
        }
        let mut events_bank = self.bank(&format!("{stem}_events"), true);
        let (carry, names, tag) = match events {
            Some(own) => {
                events_bank.objects = own.cut.objects;
                if layer == Layer::Base && !self.out.tags.contains(&self.mod_tag) {
                    self.out.tags.push(self.mod_tag.clone());
                }
                (own.carry, own.names, Some(self.mod_tag.clone()))
            }
            None => (Carry::Overlay, Vec::new(), None),
        };
        made.push((events_at, events_bank));

        for (path, bank) in made {
            self.banks.push(MadeBank {
                path,
                wad: wad.clone(),
                layer: layer.clone(),
                unit: self.unit,
                bank_unit: name.clone(),
                from: file.file,
                clips: bank.media.len(),
                objects: bank.objects.len(),
                bank,
            });
        }
        self.carried.push(CarriedBank {
            source: source_name.clone(),
            name: name.clone(),
            clips,
            layer: layer.clone(),
            carry,
            replaces: replaces.cloned(),
        });
        self.out.units.push(BankUnit {
            name,
            banks: paths,
            events: names,
            voice,
            layer,
            tag,
            replaces: replaces.cloned(),
        });
        Ok(())
    }
}

impl AudioPlan {
    /// A plan that adds no audio.
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether the plan adds nothing.
    pub fn is_empty(&self) -> bool {
        self.units.is_empty() && self.banks.is_empty()
    }

    /// What a skin unit adds.
    pub fn of(&self, unit: UnitId) -> Option<&UnitAudio> {
        self.units.iter().find(|audio| audio.unit == unit)
    }

    /// Plans the audio of each shipping unit whose banks hold own clips, and raises what is
    /// carried and what is not.
    pub fn plan(
        inputs: &PortInputs<'_>,
        shipping: &Shipping,
        layout: &mut Layout,
        sink: &mut Sink<'_>,
    ) -> Result<Self, PlanError> {
        let m: &Mod = inputs.fixed;
        let mut plan = Self::default();
        let game_buses = Self::game_buses(inputs)?;
        for &unit in &shipping.ships {
            let (Some(sounds), Some(t)) =
                (inputs.facts.sounds.of(unit), inputs.targets.target(unit))
            else {
                continue;
            };
            let character = &m.characters[m.units[unit].character];
            let catalog = inputs.facts.catalogs.of(m.units[unit].character);
            let game = catalog.and_then(|catalog| catalog.skin(t));
            let game = game.map(|skin| &skin.audio);
            // #ADR-0063, at the slot the mod's block was made for only: a block made for
            // another skin names that skin's units, and every unit of the target's is missing
            // from it. Where the game has no block at the target there is none to leave a unit
            // out of.
            let at_source = game.is_some() && t == m.units[unit].source_slot;
            let left_out = match at_source {
                true => sounds.left_out.as_slice(),
                false => &[],
            };
            if !left_out.is_empty() {
                let units = left_out.to_vec();
                sink.raise(AudioUnitsLeftOut { unit, units });
            }
            let left_out: Vec<String> = left_out.iter().map(|left| left.name.clone()).collect();
            let clips = sounds.own();
            if clips == 0 {
                plan.leave_out(unit, left_out);
                continue;
            }
            let header = match game {
                Some(game) => Self::header(inputs, game)?,
                None => None,
            };
            let (Some(game), Some(header)) = (game, header) else {
                let left = vec![NotCarried::NoGameAudio { clips, target: t }];
                sink.raise(AudioNotCarried { unit, left });
                plan.leave_out(unit, left_out);
                continue;
            };
            let paths = game.units.iter().flat_map(|unit| &unit.paths);
            let loaded: BTreeSet<String> = paths.map(|path| path.to_ascii_lowercase()).collect();
            let base_tag = game.tags.first().map_or(&character.spelled, |tag| tag);
            let base_tag = Layout::unit_part(base_tag);
            let tag_id = Layout::tag_id(&m.meta);
            let mut planning = Planning {
                inputs,
                unit,
                sounds,
                game,
                loaded,
                header,
                game_buses: game_buses.as_ref(),
                dir: layout.unit_dir(&character.name, t),
                mod_tag: format!("{base_tag}Mod{tag_id}"),
                tag_id,
                base_tag,
                own_units: 0,
                roles: BTreeMap::new(),
                out: UnitAudio {
                    unit,
                    units: Vec::new(),
                    tags: Vec::new(),
                    renamed: BTreeMap::new(),
                    left_out,
                },
                banks: Vec::new(),
                carried: Vec::new(),
                once: Vec::new(),
                left: Vec::new(),
            };
            for bank_unit in &sounds.units {
                // #MOD-ARTIFACT-0028: a unit of the game's name that loads none of its banks.
                let mine: BTreeSet<String> = bank_unit
                    .unit
                    .paths
                    .iter()
                    .map(|p| game_bank_path(p))
                    .collect();
                let replaces = game.units.iter().find(|held| {
                    let shares = held.paths.iter().any(|p| mine.contains(&game_bank_path(p)));
                    held.name == bank_unit.unit.name && !shares
                });
                let replaces = replaces.map(|held| &held.name);
                let carried = bank_unit
                    .files
                    .iter()
                    .filter(|file| matches!(file.role, BankRole::Media | BankRole::Package));
                for file in carried {
                    planning.file(layout, bank_unit, file, replaces)?;
                }
            }
            let Planning {
                out,
                banks,
                carried,
                once,
                left,
                ..
            } = planning;
            if !carried.is_empty() || !once.is_empty() {
                sink.raise(AudioCarried {
                    unit,
                    banks: carried,
                    once,
                });
            }
            if !left.is_empty() {
                sink.raise(AudioNotCarried { unit, left });
            }
            plan.banks.extend(banks);
            if !out.units.is_empty() || !out.left_out.is_empty() {
                plan.units.push(out);
            }
        }
        Ok(plan)
    }

    /// The ids of the buses the game's init bank defines. None when the game has no init bank.
    fn game_buses(inputs: &PortInputs<'_>) -> Result<Option<BTreeSet<u32>>, PlanError> {
        let init = inputs.env.game().events_bank(path_hash(INIT_BANK))?;
        let buses = |bank: &EventsBank| {
            let buses = bank.objects.iter().filter(|o| o.is_bus());
            buses.map(|o| o.id).collect::<BTreeSet<u32>>()
        };
        Ok(init.map(|bank| buses(&bank)))
    }

    /// The audio of a unit that carries no clip: the game's units it leaves out, if any.
    fn leave_out(&mut self, unit: UnitId, left_out: Vec<String>) {
        if !left_out.is_empty() {
            self.units.push(UnitAudio {
                unit,
                units: Vec::new(),
                tags: Vec::new(),
                renamed: BTreeMap::new(),
                left_out,
            });
        }
    }

    /// The header of the first events bank the game's units at the target load.
    fn header(inputs: &PortInputs<'_>, game: &AudioBlock) -> Result<Option<Vec<u8>>, PlanError> {
        let paths = game.units.iter().flat_map(|unit| &unit.paths);
        let paths = paths.map(|path| path.to_ascii_lowercase());
        for path in paths.filter(|path| path.ends_with("_events.bnk")) {
            if let Some(bank) = inputs.env.game().events_bank(path_hash(&path))? {
                return Ok(Some(bank.header.clone()));
            }
        }
        Ok(None)
    }

    /// Rewrites the event names the carried objects of each shipping unit spell out, token by
    /// token, for the target's audio. A name on a tag the target's block does not hold
    /// becomes the mod's tag where the plan kept that event under it, else the base's tag
    /// where the game's units at the target list that event, else it stays. Gives whether
    /// any object changed.
    pub fn rewrite_literals(
        &self,
        inputs: &PortInputs<'_>,
        shipping: &Shipping,
        reach: &Reach,
        objects: &mut Objects<'_>,
        sink: &mut Sink<'_>,
    ) -> bool {
        let m: &Mod = inputs.fixed;
        let mut any = false;
        for (&unit, reached) in &reach.units {
            if !shipping.ships(unit) {
                continue;
            }
            let catalog = inputs.facts.catalogs.of(m.units[unit].character);
            let game = catalog.and_then(|catalog| catalog.skin(reached.target));
            let Some(game) = game.map(|skin| &skin.audio) else {
                continue;
            };
            let Some(base_tag) = game.tags.first() else {
                continue;
            };
            let sounds = inputs.facts.sounds.of(unit);
            let mod_tags = sounds.map_or(&[][..], |sounds| &sounds.tags);
            let mut premium: BTreeSet<String> = BTreeSet::new();
            for bank_unit in sounds.into_iter().flat_map(|sounds| &sounds.units) {
                premium.extend(premium_tags(mod_tags, &game.tags, &bank_unit.unit.name));
            }
            if premium.is_empty() {
                continue;
            }
            let events = game.units.iter().flat_map(|unit| &unit.events);
            let base_events: BTreeSet<String> =
                events.map(|event| event.to_ascii_lowercase()).collect();
            let renamed = self.of(unit).map(|audio| &audio.renamed);
            // The new name, and whether it is under the mod's tag.
            let to = |name: &str| -> Option<(String, bool)> {
                let lower = name.to_ascii_lowercase();
                if let Some(new) = renamed.and_then(|renamed| renamed.get(&lower)) {
                    return Some((new.clone(), true));
                }
                let base = retag(name, &premium, base_tag)?;
                let known = base_events.contains(&base.to_ascii_lowercase());
                known.then_some((base, false))
            };
            let (mut to_mod, mut to_base) = (0, 0);
            for &(object, fate) in &reached.objects {
                if !matches!(fate, Fate::Graph | Fate::Carried) {
                    continue;
                }
                if !value::any_string(objects.value(object), &|name| to(name).is_some()) {
                    continue;
                }
                let mut held = objects.value(object).clone();
                value::rewrite_strings(&mut held, &mut |name| {
                    let (new, of_mod) = to(name)?;
                    match of_mod {
                        true => to_mod += 1,
                        false => to_base += 1,
                    }
                    Some(new)
                });
                objects.set(object, held);
            }
            if to_mod + to_base > 0 {
                any = true;
                sink.raise(LiteralsRenamed {
                    unit,
                    to_mod,
                    to_base,
                });
            }
        }
        any
    }
}

impl Dump for AudioPlan {
    fn dump(&self, _: &DumpCx<'_>) -> String {
        let mut out = String::new();
        for audio in &self.units {
            let tags = audio.tags.iter().map(|tag| clean(tag).into_owned());
            let tags: Vec<String> = tags.collect();
            let _ = writeln!(out, "audio {} tags [{}]", audio.unit, tags.join(", "));
            for unit in &audio.units {
                let _ = writeln!(
                    out,
                    "   unit {} {} banks={} events={}{}",
                    clean(&unit.name),
                    unit.layer.name(),
                    unit.banks.len(),
                    unit.events.len(),
                    if unit.replaces.is_some() {
                        " stands in"
                    } else {
                        ""
                    }
                );
            }
            for name in &audio.left_out {
                let _ = writeln!(out, "   left out: the game's unit {}", clean(name));
            }
        }
        for bank in &self.banks {
            let _ = writeln!(
                out,
                "bank {} {}/{} clips={} objects={} from {}",
                bank.layer.name(),
                bank.wad,
                bank.path,
                bank.clips,
                bank.objects,
                bank.from
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(tags: &[&str]) -> BTreeSet<String> {
        tags.iter().map(|tag| (*tag).to_string()).collect()
    }

    #[test]
    fn a_units_name_spells_a_tag_the_target_lacks() {
        let tags =
            |list: &[&str]| -> Vec<String> { list.iter().map(|t| (*t).to_string()).collect() };
        let premium = premium_tags(
            &tags(&["Jinx", "JinxSkin04"]),
            &tags(&["Jinx"]),
            "Jinx_Skin04_SFX",
        );
        assert_eq!(premium, set(&["jinxskin04"]));
        let base = premium_tags(&tags(&["Jinx"]), &tags(&["Jinx"]), "Jinx_Base_SFX");
        assert_eq!(base, set(&["jinxbase"]));
    }

    #[test]
    fn an_event_is_renamed_token_by_token() {
        let premium = set(&["jinxskin04"]);
        let renamed = retag("Play_sfx_JinxSkin04_Dance", &premium, "JinxMod0a1b2c3d");
        assert_eq!(renamed.as_deref(), Some("Play_sfx_JinxMod0a1b2c3d_Dance"));
        assert_eq!(retag("Play_sfx_Jinx_Dance", &premium, "X"), None);
    }
}
