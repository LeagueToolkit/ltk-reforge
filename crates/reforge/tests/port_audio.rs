//! The audio of a port over synthetic mods: whose the clips of a shipped bank are, a unit of
//! the own clips after the game's, a unit with events of its own, a bank the author added, a
//! unit that stands in for the game's, and the voice layers. The game is
//! `reforge_testkit::sounds`.

use ltk_meta::BinObject;
use reforge::env::Env;
use reforge::facts::ClipOrigin;
use reforge::history::{NoHistory, Position};
use reforge::model::{BankClip, BankRole, Layer, Slot, name_hash, value};
use reforge::port::{
    AudioCarried, AudioNotCarried, Carry, LiteralsRenamed, NotCarried, Policy, PortOptions,
};
use reforge_testkit::bins::{bin, graph, resolver, text};
use reforge_testkit::facts::env;
use reforge_testkit::skins::{self, BASE_CLIPS, WAD, skin_bin};
use reforge_testkit::sounds::{
    BASE_SFX, BASE_VO, SFX_AUDIO, SFX_EVENTS, Unit, VO_WPK, audio_game, base_entry, clip,
    container, event, events_bank, media_bank, package, play, sfx, sound, voice_game, with_audio,
};
use reforge_testkit::{MemoryGame, MemoryHistory, ModArchive, PlanRun, PortRun};
use reforge_wwise::{Bank, CLIENT_VERSION, id_of, kind};

const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const DANCE: &str = "Play_sfx_TestSkin03_Dance";
const IDLE: &str = "Play_sfx_TestSkin03_Idle";
const BASE_DANCE: &str = "Play_sfx_Test_Dance";
const LOCALE_WAD: &str = "Test.en_US.wad.client";
const DIR: &str = "mods/synthetic.reforgetests/characters/test/skin0/audio";

/// The mod's skin bin around an entry. Its graph names two of skin 3's events literally, as
/// an animation's sound event does.
fn skin0(entry: BinObject) -> Vec<u8> {
    let mut graph = graph(GRAPH0, BASE_CLIPS);
    let fields = &mut graph.properties;
    fields.insert(name_hash("danceSound"), text(DANCE));
    fields.insert(name_hash("idleSound"), text(IDLE));
    let resources = resolver("Characters/Test/Skins/Skin0/Resources");
    bin(vec![entry, graph, resources])
}

/// A mod of the base's model whose entry holds `units`, with these files beside its skin bin.
fn mod_of(tags: &[&str], units: &[Unit<'_>], files: &[(&str, Vec<u8>)]) -> ModArchive {
    let entry = with_audio(base_entry(&[]), tags, units);
    let archive = ModArchive::new().file(WAD, &skin_bin("test", 0), skin0(entry));
    archive.files(WAD, files)
}

/// The mod that ships a media bank at the base's path: clip 1 as the game has it, clip 2
/// replaced, clip 9 new.
fn over_the_base(version: u32) -> ModArchive {
    let clips = vec![clip(1, 1), clip(2, 7), clip(9, 9)];
    let media = media_bank("test_base_sfx_audio", version, clips);
    mod_of(&["Test"], &[BASE_SFX], &[(SFX_AUDIO, media)])
}

fn port<'a>(env: &'a Env, archive: &ModArchive) -> PortRun<'a> {
    PortRun::of(env, archive)
}

fn read(run: &PortRun<'_>, path: &str) -> Bank {
    Bank::read(&run.chunk(path).bytes).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn clip_ids(bank: &Bank) -> Vec<u32> {
    bank.media.iter().map(|wem| wem.id).collect()
}

#[test]
fn without_a_history_a_clip_of_the_games_bank_at_the_path_is_the_games() {
    let env = env(audio_game(), NoHistory);
    let run = port(&env, &over_the_base(134));
    let unit = run.unit("test", 0);
    let sounds = run.facts.facts.sounds.of(unit).expect("the unit's audio");
    assert_eq!(sounds.tags, ["Test"]);
    let file = &sounds.units[0].files[0];
    assert_eq!((file.role, file.version), (BankRole::Media, 134));
    let origin = |id: u32| file.clips.iter().find(|c| c.id == id).map(|c| c.origin);
    assert_eq!(origin(1), Some(ClipOrigin::GameBank));
    assert_eq!(origin(2), Some(ClipOrigin::Own));
    assert_eq!(origin(9), Some(ClipOrigin::Own));
    assert_eq!(file.own_ids().into_iter().collect::<Vec<_>>(), [2, 9]);
}

#[test]
fn with_a_history_a_clip_the_game_ever_shipped_is_the_games() {
    // Clip 2 as the mod has it is bytes the game shipped at an earlier build.
    let old = BankClip::of(2, &clip(2, 7).bytes);
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]).with_clip(old.key, 0..=0);
    let env = env(audio_game(), history);
    let run = port(&env, &over_the_base(134));
    let sounds = run
        .facts
        .facts
        .sounds
        .of(run.unit("test", 0))
        .expect("audio");
    let file = &sounds.units[0].files[0];
    let origin = |id: u32| file.clips.iter().find(|c| c.id == id).map(|c| c.origin);
    let first = Position(0);
    assert_eq!(origin(2), Some(ClipOrigin::Game { first }));
    // The history does not hold the install's own clip here, so it is the mod's.
    assert_eq!(origin(1), Some(ClipOrigin::Own));
    assert_eq!(origin(9), Some(ClipOrigin::Own));
}

#[test]
fn own_clips_at_the_bases_path_go_in_a_unit_after_the_games() {
    let env = env(audio_game(), NoHistory);
    let run = port(&env, &over_the_base(134));
    let audio = run.port.audio.of(run.unit("test", 0)).expect("planned");
    assert_eq!(audio.units.len(), 1, "{}", run.dump());
    let unit = &audio.units[0];
    assert!(unit.name.starts_with("Test_Base_SFX_Mod"), "{}", unit.name);
    assert!(audio.tags.is_empty(), "the game's events play the clips");
    assert_eq!((&unit.layer, unit.voice), (&Layer::Base, false));
    let paths: Vec<&str> = unit.banks.iter().map(|path| path.as_str()).collect();
    let (audio_at, events_at) = (
        format!("{DIR}/sfx_audio.bnk"),
        format!("{DIR}/sfx_events.bnk"),
    );
    assert_eq!(paths, [audio_at.as_str(), events_at.as_str()]);

    // #GAME-BEHAVIOR-0012: the clips keep the game's ids, so the game's events play them.
    // The media bank holds the own clips, at the client's version, under the id of the
    // unit's name. The events bank holds a hierarchy with no object, in a file named for
    // its role (#GAME-BEHAVIOR-0013).
    let media = read(&run, &audio_at);
    assert_eq!(media.version, CLIENT_VERSION);
    assert_eq!(clip_ids(&media), [2, 9]);
    let stem = unit.name.to_ascii_lowercase();
    assert_eq!(media.id(), id_of(&format!("{stem}_audio")));
    let events = read(&run, &events_at);
    assert!(events.has_hirc() && events.objects.is_empty());
    assert_eq!(events.id(), id_of(&format!("{stem}_events")));

    let carried = run.only::<AudioCarried>();
    assert_eq!(carried.banks.len(), 1);
    assert_eq!(carried.banks[0].carry, Carry::Overlay);
    assert_eq!(carried.banks[0].clips, 2);
    assert_eq!(carried.banks[0].replaces, None);
    assert!(run.found::<AudioNotCarried>().is_empty());
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
}

#[test]
fn own_clips_for_a_target_with_no_audio_block_are_not_carried() {
    // The game's base skin has no audio block, so no events bank gives a header.
    let env = env(skins::game(), NoHistory);
    let run = PlanRun::of(&env, &over_the_base(134));
    assert!(run.port().audio.is_empty(), "{}", run.dump());
    // No bank of the game's is at the path, so each of the three clips is the mod's own.
    let expected = NotCarried::NoGameAudio {
        clips: 3,
        target: Slot(0),
    };
    assert_eq!(run.only::<AudioNotCarried>().left, [expected]);
    assert!(run.found::<AudioCarried>().is_empty());
}

#[test]
fn a_mod_of_game_clips_only_carries_nothing() {
    let env = env(audio_game(), NoHistory);
    let clips = vec![clip(1, 1), clip(2, 2)];
    let media = media_bank("test_base_sfx_audio", CLIENT_VERSION, clips);
    let run = port(&env, &mod_of(&["Test"], &[BASE_SFX], &[(SFX_AUDIO, media)]));
    assert!(run.port.audio.is_empty(), "{}", run.dump());
    assert!(run.found::<AudioCarried>().is_empty());
    assert!(run.found::<AudioNotCarried>().is_empty());
}

/// Skin 3's unit as a mod pastes it onto the base.
fn skin3_unit<'a>(banks: &'a [&'a str]) -> Unit<'a> {
    Unit {
        name: "Test_Skin03_SFX",
        banks,
        events: &[DANCE, IDLE],
        voice: false,
    }
}

#[test]
fn own_clips_at_a_premium_path_without_an_events_bank_are_reported() {
    let env = env(audio_game(), NoHistory);
    let at = sfx("skin03", "test_skin03_sfx_audio");
    let media = media_bank("test_skin03_sfx_audio", CLIENT_VERSION, vec![clip(9, 9)]);
    let banks = [at.as_str()];
    let archive = mod_of(&["Test"], &[skin3_unit(&banks)], &[(&at, media)]);
    let run = port(&env, &archive);
    assert!(run.port.audio.is_empty(), "{}", run.dump());
    let left = &run.only::<AudioNotCarried>().left;
    let expected = NotCarried::NoEventsBank {
        clips: 1,
        source: "Test_Skin03_SFX".into(),
    };
    assert_eq!(left, &[expected]);
}

/// The game with skin 3's banks: clips 5 and 6, and an events bank whose dance plays a
/// container of clips 9 and 5 and whose idle plays clip 6.
fn skin3_game() -> (MemoryGame, String, String) {
    let mut game = audio_game();
    let audio_at = sfx("skin03", "test_skin03_sfx_audio");
    let events_at = sfx("skin03", "test_skin03_sfx_events");
    let clips = vec![clip(5, 5), clip(6, 6)];
    let media = media_bank("test_skin03_sfx_audio", CLIENT_VERSION, clips);
    game.insert(&audio_at, media);
    let stem = "test_skin03_sfx_events";
    let objects = vec![
        container(50, 0),
        sound(10, 9, 50),
        sound(11, 5, 50),
        sound(12, 6, 0),
        play(20, 50, stem),
        play(21, 12, stem),
        event(DANCE, &[20]),
        event(IDLE, &[21]),
    ];
    game.insert(&events_at, events_bank(stem, objects));
    (game, audio_at, events_at)
}

#[test]
fn own_clips_at_a_premium_path_get_a_unit_of_their_own() {
    let (game, audio_at, events_at) = skin3_game();
    let env = env(game, NoHistory);
    // The mod on the base's model, its audio block skin 3's, clip 9 of its media bank its own.
    let clips = vec![clip(5, 5), clip(9, 9)];
    let media = media_bank("test_skin03_sfx_audio", CLIENT_VERSION, clips);
    let banks = [audio_at.as_str(), events_at.as_str()];
    let unit = skin3_unit(&banks);
    let archive = mod_of(&["TestSkin03"], &[unit], &[(&audio_at, media)]);
    let run = port(&env, &archive);

    let audio = run.port.audio.of(run.unit("test", 0)).expect("planned");
    // #GAME-BEHAVIOR-0011: the mod's tag goes after the game's, and the kept event is named
    // under it.
    assert_eq!(audio.tags.len(), 1, "{}", run.dump());
    let tag = &audio.tags[0];
    assert!(tag.starts_with("TestMod"), "{tag}");
    let unit = &audio.units[0];
    assert!(unit.name.starts_with("Test_Mod") && unit.name.ends_with("_SFX"));
    assert_eq!(unit.tag.as_ref(), Some(tag));
    // The dance reaches clip 9 and is renamed under the mod's tag. The idle reaches none.
    let dance = DANCE.replace("TestSkin03", tag);
    assert_eq!(unit.events, std::slice::from_ref(&dance));
    assert_eq!(audio.renamed.get(&DANCE.to_ascii_lowercase()), Some(&dance));

    let media = read(&run, &format!("{DIR}/sfx_audio.bnk"));
    assert_eq!(clip_ids(&media), [9]);
    let events = read(&run, &format!("{DIR}/sfx_events.bnk"));
    let kept: Vec<u32> = events
        .objects
        .iter()
        .filter(|object| object.kind == kind::EVENT)
        .map(|object| object.id)
        .collect();
    assert_eq!(kept, [id_of(&dance)]);
    // #GAME-BEHAVIOR-0017: every node stays. The idle's action goes.
    assert!(events.object(12).is_some() && events.object(50).is_some());
    assert!(events.object(11).is_some() && events.object(21).is_none());
    // #GAME-BEHAVIOR-0014: the dance's action names the new bank by its header's id.
    let action = events.object(20).expect("the dance's action");
    assert_eq!(action.play_bank(), Some(events.id()));
    let stem = unit.name.to_ascii_lowercase();
    assert_eq!(events.id(), id_of(&format!("{stem}_events")));

    let carried = &run.only::<AudioCarried>().banks[0];
    let expected = Carry::Own {
        tag: tag.clone(),
        events: 1,
        from_mod: false,
        shared: 0,
        silent: 1,
        unnamed: Vec::new(),
    };
    assert_eq!(carried.carry, expected);

    // The graph's literal names: the dance follows the renamed event. The idle, which the
    // mod's unit dropped and the base does not list, stays.
    let graph = run
        .port
        .object(run.object(GRAPH0))
        .expect("the graph ships");
    let named = |field: &str| value::get(graph, &[field]).and_then(value::as_str);
    assert_eq!(named("danceSound"), Some(dance.as_str()));
    assert_eq!(named("idleSound"), Some(IDLE));
    let literals = run.only::<LiteralsRenamed>();
    assert_eq!((literals.to_mod, literals.to_base), (1, 0));
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
}

#[test]
fn a_literal_name_on_a_premium_tag_takes_the_bases_tag_where_the_base_lists_the_event() {
    let env = env(audio_game(), NoHistory);
    // Skin 3's audio block pasted onto the base, with no bank shipped: nothing is carried,
    // and the port's audio is the base's.
    let at = sfx("skin03", "test_skin03_sfx_audio");
    let banks = [at.as_str()];
    let archive = mod_of(&["TestSkin03"], &[skin3_unit(&banks)], &[]);
    let run = PlanRun::of(&env, &archive);
    assert!(run.port().audio.is_empty(), "{}", run.dump());

    let m = &run.facts.fix.fixed;
    let mut objects = m.objects.iter();
    let graph = objects.find(|(_, object)| object.entry == name_hash(GRAPH0));
    let (graph, _) = graph.expect("the mod's graph");
    let graph = run.port().object(graph).expect("the graph ships");
    let named = |field: &str| value::get(graph, &[field]).and_then(value::as_str);
    // The base's unit lists the dance under the base's tag, and no idle.
    assert_eq!(named("danceSound"), Some(BASE_DANCE));
    assert_eq!(named("idleSound"), Some(IDLE));
    let literals = run.only::<LiteralsRenamed>();
    assert_eq!((literals.to_mod, literals.to_base), (0, 1));
}

/// A base events bank whose dance plays `clip`.
fn base_events(clip: u32) -> Vec<u8> {
    let stem = "test_base_sfx_events";
    let objects = vec![
        sound(10, clip, 0),
        play(20, 10, stem),
        event(BASE_DANCE, &[20]),
    ];
    events_bank(stem, objects)
}

/// A mod that adds a media bank of its own, clip 77, to the base's unit. Its copy of the
/// base's events bank plays the clip under the base's event.
fn added_bank(options: PortOptions) -> (Env, ModArchive, PortOptions) {
    let mut game = audio_game();
    game.insert(SFX_EVENTS, base_events(1));
    let added = sfx("base", "added_sfx_audio");
    let banks = [added.as_str(), SFX_EVENTS];
    let unit = Unit {
        banks: &banks,
        events: &[BASE_DANCE],
        ..BASE_SFX
    };
    let media = media_bank("added_sfx_audio", CLIENT_VERSION, vec![clip(77, 7)]);
    let files = [(added.as_str(), media), (SFX_EVENTS, base_events(77))];
    let archive = mod_of(&["Test"], &[unit], &files);
    (env(game, NoHistory), archive, options)
}

#[test]
fn an_added_bank_is_cut_from_the_mods_own_events_bank() {
    let (env, archive, options) = added_bank(PortOptions::default());
    let run = PortRun::under(&env, &archive, Policy::default(), options);
    let audio = run.port.audio.of(run.unit("test", 0)).expect("planned");
    // The base's tag is no premium one: the dance keeps its name, which the game's bank
    // defines too.
    assert_eq!(audio.units[0].events, [BASE_DANCE]);
    assert_eq!(clip_ids(&read(&run, &format!("{DIR}/sfx_audio.bnk"))), [77]);
    let events = read(&run, &format!("{DIR}/sfx_events.bnk"));
    assert!(events.object(id_of(BASE_DANCE)).is_some());
    let action = events.object(20).expect("the dance's action");
    assert_eq!(action.play_bank(), Some(events.id()));
    let carried = &run.only::<AudioCarried>().banks[0];
    assert!(
        matches!(
            carried.carry,
            Carry::Own {
                from_mod: true,
                shared: 1,
                events: 1,
                ..
            }
        ),
        "{carried:?}"
    );
}

#[test]
fn with_the_games_events_only_an_added_bank_is_reported() {
    let options = PortOptions {
        game_events_only: true,
        ..PortOptions::default()
    };
    let (env, archive, options) = added_bank(options);
    let run = PortRun::under(&env, &archive, Policy::default(), options);
    assert!(run.port.audio.is_empty(), "{}", run.dump());
    let expected = NotCarried::NoEvent {
        clips: 1,
        source: "Test_Base_SFX".into(),
    };
    assert_eq!(run.only::<AudioNotCarried>().left, [expected]);
}

#[test]
fn a_unit_of_the_games_name_on_banks_of_its_own_stands_in_for_the_games() {
    let mut game = audio_game();
    game.insert(SFX_EVENTS, base_events(1));
    let env = env(game, NoHistory);
    // #MOD-ARTIFACT-0028: the unit keeps the base unit's name and loads banks of its own, a
    // media bank of clip 77 and an events bank whose own event plays it.
    let (audio_at, events_at) = (
        sfx("base", "added_sfx_audio"),
        sfx("base", "added_sfx_events"),
    );
    let squirt = "Play_sfx_Test_Squirt";
    let banks = [audio_at.as_str(), events_at.as_str()];
    let unit = Unit {
        banks: &banks,
        events: &[squirt],
        ..BASE_SFX
    };
    let stem = "added_sfx_events";
    let objects = vec![sound(10, 77, 0), play(20, 10, stem), event(squirt, &[20])];
    let files = [
        (
            audio_at.as_str(),
            media_bank("added_sfx_audio", CLIENT_VERSION, vec![clip(77, 7)]),
        ),
        (events_at.as_str(), events_bank(stem, objects)),
    ];
    let run = port(&env, &mod_of(&["Test"], &[unit], &files));
    let audio = run.port.audio.of(run.unit("test", 0)).expect("planned");
    let unit = &audio.units[0];
    assert_eq!(unit.replaces.as_deref(), Some("Test_Base_SFX"));
    assert_eq!(unit.events, [squirt]);
    let carried = &run.only::<AudioCarried>().banks[0];
    assert_eq!(carried.replaces.as_deref(), Some("Test_Base_SFX"));
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
}

/// A mod of the base's model and audio block whose voice package, clip 3 replaced, ships in
/// each WAD named.
fn voice_mod(wads: &[&str]) -> ModArchive {
    let entry = base_entry(&[BASE_SFX, BASE_VO]);
    let archive = ModArchive::new().file(WAD, &skin_bin("test", 0), skin0(entry));
    let voice = package(vec![clip(3, 7), clip(4, 4)]);
    let add = |archive: ModArchive, wad: &&str| archive.file(wad, VO_WPK, voice.clone());
    wads.iter().fold(archive, add)
}

#[test]
fn voice_goes_to_a_layer_of_its_own() {
    let env = env(voice_game(), NoHistory);
    let run = port(&env, &voice_mod(&[WAD]));
    let audio = run.port.audio.of(run.unit("test", 0)).expect("planned");
    let unit = &audio.units[0];
    assert!(unit.voice && unit.layer == Layer::Voice(None), "{unit:?}");
    let paths: Vec<&str> = unit.banks.iter().map(|path| path.as_str()).collect();
    let expected = ["vo_audio.bnk", "vo_events.bnk", "vo_audio.wpk"].map(|n| format!("{DIR}/{n}"));
    assert_eq!(paths, expected);
    let made = &run.port.audio.banks;
    assert_eq!(made.len(), 3);
    assert!(made.iter().all(|bank| bank.layer == Layer::Voice(None)));
    assert!(made.iter().all(|bank| bank.wad.as_str() == WAD));
    // The stub holds no clip, and the package the one own clip.
    let stub = made
        .iter()
        .find(|bank| bank.path.as_str().ends_with("vo_audio.bnk"));
    assert_eq!(stub.map(|bank| bank.clips), Some(0));
    assert_eq!(clip_ids(&read(&run, &format!("{DIR}/vo_audio.wpk"))), [3]);
    assert_eq!(
        run.only::<AudioCarried>().banks[0].layer,
        Layer::Voice(None)
    );
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
}

#[test]
fn a_locale_wads_voice_goes_to_the_locales_layer_and_wad() {
    let env = env(voice_game(), NoHistory);
    let run = port(&env, &voice_mod(&[LOCALE_WAD]));
    let audio = run.port.audio.of(run.unit("test", 0)).expect("planned");
    let unit = &audio.units[0];
    let layer = Layer::Voice(Some("en_us".into()));
    assert!(unit.name.ends_with("_en_us"), "{}", unit.name);
    assert_eq!(unit.layer, layer);
    assert_eq!(layer.name(), "voice-en-us");
    let package = format!("{DIR}/en_us/vo_audio.wpk");
    let made = run.port.audio.banks.iter();
    let made: Vec<_> = made.filter(|bank| bank.path.as_str() == package).collect();
    assert_eq!(made.len(), 1, "{}", run.dump());
    assert_eq!(made[0].wad.as_str(), LOCALE_WAD);
    assert!(run.found::<AudioNotCarried>().is_empty());
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
}

#[test]
fn a_locale_copy_of_the_champion_wads_voice_is_carried_once() {
    let env = env(voice_game(), NoHistory);
    let run = port(&env, &voice_mod(&[WAD, LOCALE_WAD]));
    let audio = run.port.audio.of(run.unit("test", 0)).expect("planned");
    assert_eq!(audio.units.len(), 1, "{}", run.dump());
    assert_eq!(audio.units[0].layer, Layer::Voice(None));
    let carried = run.only::<AudioCarried>();
    assert_eq!(carried.once.len(), 1);
    assert_eq!(carried.once[0].locale, "en_us");
}

#[test]
fn a_locale_copy_with_other_clips_is_not_carried() {
    let env = env(voice_game(), NoHistory);
    let other = package(vec![clip(3, 3), clip(4, 8)]);
    let archive = voice_mod(&[WAD]).file(LOCALE_WAD, VO_WPK, other);
    let run = port(&env, &archive);
    let audio = run.port.audio.of(run.unit("test", 0)).expect("planned");
    assert_eq!(audio.units.len(), 1, "{}", run.dump());
    let expected = NotCarried::LocaleDiffers {
        clips: 1,
        source: "Test_Base_VO".into(),
        locale: "en_us".into(),
    };
    assert_eq!(run.only::<AudioNotCarried>().left, [expected]);
}
