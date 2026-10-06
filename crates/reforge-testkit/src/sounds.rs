//! Sound banks and audio blocks for tests of a port's audio, and the synthetic game of
//! [`crate::skins`] with an audio block on its base skin: one effects unit with a media bank
//! (clips 1 and 2) and an events bank, and for the voice tests a voice unit with a package
//! (clips 3 and 4).

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyKind, PropertyValueEnum};
use reforge::model::known::field;
use reforge::model::name_hash;
use reforge_wwise::{ACTION_PLAY, Bank, CLIENT_VERSION, Object, Wem, id_of, kind};

use crate::bins::{bin, embedded, entry, flag, graph, particle, resolver, text};
use crate::game::MemoryGame;
use crate::skins::{self, BASE_CLIPS, BASE_PARTICLE, BASE_PARTICLE_TEX, base_skin, skin_bin};

/// The folder of the champion's effects banks, skin by skin.
const SFX: &str = "assets/sounds/wwise2016/sfx/characters/test/skins";

/// The base skin's effects media bank.
pub const SFX_AUDIO: &str =
    "assets/sounds/wwise2016/sfx/characters/test/skins/base/test_base_sfx_audio.bnk";
/// The base skin's effects events bank.
pub const SFX_EVENTS: &str =
    "assets/sounds/wwise2016/sfx/characters/test/skins/base/test_base_sfx_events.bnk";
/// The base skin's voice stub.
pub const VO_AUDIO: &str =
    "assets/sounds/wwise2016/vo/en_us/characters/test/skins/base/test_base_vo_audio.bnk";
/// The base skin's voice events bank.
pub const VO_EVENTS: &str =
    "assets/sounds/wwise2016/vo/en_us/characters/test/skins/base/test_base_vo_events.bnk";
/// The base skin's voice package.
pub const VO_WPK: &str =
    "assets/sounds/wwise2016/vo/en_us/characters/test/skins/base/test_base_vo_audio.wpk";

/// The path of an effects bank of a skin's folder: `sfx("skin03", "test_skin03_sfx_audio")`.
pub fn sfx(folder: &str, stem: &str) -> String {
    format!("{SFX}/{folder}/{stem}.bnk")
}

/// The header of a bank named `stem`, of the version the client loads.
pub fn header(stem: &str) -> Vec<u8> {
    let mut header = CLIENT_VERSION.to_le_bytes().to_vec();
    header.extend(id_of(stem).to_le_bytes());
    header.extend(0u32.to_le_bytes());
    header.extend(16u16.to_le_bytes());
    header.extend(0u16.to_le_bytes());
    header
}

/// A clip of an id, its bytes filled with one value.
pub fn clip(id: u32, fill: u8) -> Wem {
    Wem {
        id,
        bytes: vec![fill; 40 + id as usize],
    }
}

/// A media bank named `stem`, of a version, holding these clips.
pub fn media_bank(stem: &str, version: u32, clips: Vec<Wem>) -> Vec<u8> {
    let mut header = header(stem);
    header[..4].copy_from_slice(&version.to_le_bytes());
    let mut bank = Bank::new_bnk(header, false);
    bank.media = clips;
    bank.write()
}

/// A media bank named `stem`, of the version the client loads, holding these clips and these
/// objects beside them.
pub fn media_bank_with(stem: &str, clips: Vec<Wem>, objects: Vec<Object>) -> Vec<u8> {
    let mut bank = Bank::new_bnk(header(stem), true);
    bank.media = clips;
    bank.objects = objects;
    bank.write()
}

/// A bus named `name` under the bus of an id, 0 for none.
pub fn bus(name: &str, parent: u32) -> Object {
    Object {
        kind: kind::BUS,
        id: id_of(name),
        body: parent.to_le_bytes().to_vec(),
    }
}

/// The game's init bank, defining buses of these names.
pub fn init_bank(buses: &[&str]) -> Vec<u8> {
    events_bank("init", buses.iter().map(|name| bus(name, 0)).collect())
}

/// A voice package of these clips.
pub fn package(clips: Vec<Wem>) -> Vec<u8> {
    Bank::new_wpk(clips).write()
}

/// A bank named `stem` with a header alone.
pub fn stub(stem: &str) -> Vec<u8> {
    Bank::new_bnk(header(stem), false).write()
}

/// An events bank named `stem` holding these objects.
pub fn events_bank(stem: &str, objects: Vec<Object>) -> Vec<u8> {
    let mut bank = Bank::new_bnk(header(stem), true);
    bank.objects = objects;
    bank.write()
}

/// A node's base parameters up to its parent, as the client's version writes them.
fn node_head(parent: u32) -> Vec<u8> {
    let mut body = vec![0, 0, 0, 0, 0];
    body.extend(0u32.to_le_bytes());
    body.extend(parent.to_le_bytes());
    body
}

/// A random container under a parent, 0 for none.
pub fn container(id: u32, parent: u32) -> Object {
    Object {
        kind: kind::RANDOM,
        id,
        body: node_head(parent),
    }
}

/// A sound that plays a clip, under a parent, 0 for none.
pub fn sound(id: u32, clip: u32, parent: u32) -> Object {
    let mut body = 0x0004_0001u32.to_le_bytes().to_vec();
    body.push(0);
    body.extend(clip.to_le_bytes());
    body.extend(40u32.to_le_bytes());
    body.push(0);
    body.extend(node_head(parent));
    Object {
        kind: kind::SOUND,
        id,
        body,
    }
}

/// A play action on a target, which it expects in the bank named `bank`.
pub fn play(id: u32, target: u32, bank: &str) -> Object {
    let mut body = ((u16::from(ACTION_PLAY) << 8) | 3).to_le_bytes().to_vec();
    body.extend(target.to_le_bytes());
    body.extend([0, 0, 0, 4]);
    body.extend(id_of(bank).to_le_bytes());
    Object {
        kind: kind::ACTION,
        id,
        body,
    }
}

/// An event of a name that runs these actions.
pub fn event(name: &str, actions: &[u32]) -> Object {
    Object::event(id_of(name), actions, CLIENT_VERSION)
}

/// A bank unit of an audio block.
#[derive(Debug, Clone, Copy)]
pub struct Unit<'a> {
    /// Its name.
    pub name: &'a str,
    /// The paths of its banks.
    pub banks: &'a [&'a str],
    /// The names of its events.
    pub events: &'a [&'a str],
    /// Whether it is voice.
    pub voice: bool,
}

fn strings(items: &[&str]) -> PropertyValueEnum {
    let items = items.iter().map(|item| text(item)).collect();
    let list = values::Container::new(PropertyKind::String, items)
        .unwrap_or_else(|error| panic!("a list of strings does not build: {error}"));
    PropertyValueEnum::Container(list)
}

/// A skin entry with an audio block of these tags and units.
pub fn with_audio(mut entry: BinObject, tags: &[&str], units: &[Unit<'_>]) -> BinObject {
    let unit = |unit: &Unit<'_>| {
        let fields = vec![
            (field::BANK_UNIT_NAME, text(unit.name)),
            (field::BANK_PATH, strings(unit.banks)),
            (field::BANK_UNIT_EVENTS, strings(unit.events)),
            (field::VOICE_OVER, flag(unit.voice)),
        ];
        embedded("BankUnit", fields)
    };
    let units = units.iter().map(unit).collect();
    let units = values::Container::new(PropertyKind::Embedded, units)
        .unwrap_or_else(|error| panic!("a list of bank units does not build: {error}"));
    let units = PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(units));
    let block = vec![
        (field::TAG_EVENT_LIST, strings(tags)),
        (field::BANK_UNITS, units),
    ];
    let block = embedded("SkinAudioProperties", block);
    let name = name_hash(field::SKIN_AUDIO_PROPERTIES);
    entry.properties.insert(name, block);
    entry
}

/// The effects unit of the game's base skin.
pub const BASE_SFX: Unit<'static> = Unit {
    name: "Test_Base_SFX",
    banks: &[SFX_AUDIO, SFX_EVENTS],
    events: &["Play_sfx_Test_Dance"],
    voice: false,
};

/// The voice unit of the game's base skin.
pub const BASE_VO: Unit<'static> = Unit {
    name: "Test_Base_VO",
    banks: &[VO_AUDIO, VO_EVENTS, VO_WPK],
    events: &["Play_vo_Test_Joke"],
    voice: true,
};

/// The base skin's entry with an audio block of the tag `Test` and these units.
pub fn base_entry(units: &[Unit<'_>]) -> BinObject {
    let entry = entry("Characters/Test/Skins/Skin0", &base_skin());
    with_audio(entry, &["Test"], units)
}

/// The game's base skin bin around an entry.
fn base_bin(entry: BinObject) -> Vec<u8> {
    bin(vec![
        entry,
        graph("Characters/Test/Animations/Skin0", BASE_CLIPS),
        resolver("Characters/Test/Skins/Skin0/Resources"),
        particle(BASE_PARTICLE, BASE_PARTICLE_TEX),
    ])
}

/// The game of [`crate::skins`] with an effects unit on its base skin: a media bank of clips
/// 1 and 2, and an events bank with no object.
pub fn audio_game() -> MemoryGame {
    let mut game = skins::game();
    game.insert(&skin_bin("test", 0), base_bin(base_entry(&[BASE_SFX])));
    let clips = vec![clip(1, 1), clip(2, 2)];
    let media = media_bank("test_base_sfx_audio", CLIENT_VERSION, clips);
    game.insert(SFX_AUDIO, media);
    game.insert(SFX_EVENTS, events_bank("test_base_sfx_events", Vec::new()));
    game
}

/// [`audio_game`] with a voice unit too: a stub, an events bank with no object, and a package
/// of clips 3 and 4.
pub fn voice_game() -> MemoryGame {
    let mut game = audio_game();
    let entry = base_entry(&[BASE_SFX, BASE_VO]);
    game.insert(&skin_bin("test", 0), base_bin(entry));
    game.insert(VO_AUDIO, stub("test_base_vo_audio"));
    game.insert(VO_EVENTS, events_bank("test_base_vo_events", Vec::new()));
    game.insert(VO_WPK, package(vec![clip(3, 3), clip(4, 4)]));
    game
}
