//! The audio of a port's package over synthetic mods: the banks made as chunks, the audio
//! block of the patched skin entry, the patch of a voice layer applied over the base layer's
//! result, and the package written. The game is `reforge_testkit::sounds`.

use std::collections::BTreeMap;

use ltk_meta::BinObject;
use reforge::diag::{Diagnostics, Stage};
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::{AudioBlock, ChunkPath, Layer, SkinEntry, name_hash, path_hash};
use reforge::package::{
    Coverage, Done, MANIFEST_FILE, Origin, Package, PackageWriter, PortBuilder, Ported, ProjectDir,
};
use reforge::port::{AudioNotCarried, BankUnit, NotCarried, Port, UnitAudio};
use reforge_testkit::bins::{Skin, bin, entry, graph, resolver};
use reforge_testkit::facts::{archive, env, mod_skin, own_files};
use reforge_testkit::skins::{self, BASE_CLIPS, WAD, skin_bin};
use reforge_testkit::sounds::{
    BASE_SFX, BASE_VO, SFX_AUDIO, SFX_EVENTS, Unit, VO_WPK, audio_game, base_entry, clip, event,
    events_bank, media_bank, package, play, sfx, sound, voice_game, with_audio,
};
use reforge_testkit::{ModArchive, PortRun};
use reforge_wwise::CLIENT_VERSION;

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const SKIN0_BIN: &str = "data/characters/test/skins/skin0.bin";
const LOCALE_WAD: &str = "Test.en_US.wad.client";
const DIR: &str = "mods/synthetic.reforgetests/characters/test/skin0/audio";

fn skin0(entry: BinObject) -> Vec<u8> {
    let graph = graph("Characters/Test/Animations/Skin0", BASE_CLIPS);
    let resources = resolver("Characters/Test/Skins/Skin0/Resources");
    bin(vec![entry, graph, resources])
}

/// A mod of the base's model whose entry holds `units`, with these files beside its skin bin.
fn mod_of(units: &[Unit<'_>], files: &[(&str, Vec<u8>)]) -> ModArchive {
    let archive = ModArchive::new().file(WAD, &skin_bin("test", 0), skin0(base_entry(units)));
    archive.files(WAD, files)
}

/// The audio block of the skin entry in a bin's objects.
fn block(objects: &[BinObject]) -> AudioBlock {
    let entry = objects.iter().find(|o| o.path_hash == name_hash(SKIN0));
    SkinEntry::read(entry.expect("the skin entry")).audio()
}

fn names(block: &AudioBlock) -> Vec<&str> {
    block.units.iter().map(|unit| unit.name.as_str()).collect()
}

#[test]
fn the_banks_ship_and_the_patch_adds_their_unit_after_the_games() {
    let env = env(audio_game(), NoHistory);
    let clips = vec![clip(1, 1), clip(2, 7), clip(9, 9)];
    let media = media_bank("test_base_sfx_audio", 134, clips);
    let run = PortRun::of(&env, &mod_of(&[BASE_SFX], &[(SFX_AUDIO, media)]));
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());

    let unit = &run.port.audio.units[0].units[0];
    for path in &unit.banks {
        let chunk = run.chunk(path.as_str());
        assert!(matches!(chunk.origin, Origin::Bank(_)), "{}", run.dump());
        assert!(
            run.package()
                .tables
                .paths()
                .any(|held| held == path.as_str())
        );
    }
    // The game's unit stays, the port's comes after it, and no tag is added.
    let applied = run.applied(&Layer::Base, SKIN0_BIN);
    let patched = block(&applied.objects);
    assert_eq!(names(&patched), ["Test_Base_SFX", unit.name.as_str()]);
    assert_eq!(patched.tags, ["Test"]);
    let ours = &patched.units[1];
    let banks: Vec<&str> = unit.banks.iter().map(|path| path.as_str()).collect();
    assert_eq!(ours.paths, banks);
    assert!(!ours.voice && ours.events.is_empty());
    assert_eq!(patched.units[0].paths, [SFX_AUDIO, SFX_EVENTS]);

    let ported = run.only::<Ported>();
    assert_eq!((ported.banks, ported.left.recut), (2, 1));
    let read_back = run
        .found::<Done>()
        .into_iter()
        .any(|done| matches!(done.0, Coverage::RoundTrip { banks: 2, .. }));
    assert!(read_back, "{:#?}", run.messages());
}

/// #MOD-ARTIFACT-0028: a unit of the game's name on banks of the mod's own stands in for
/// the game's.
#[test]
fn a_unit_that_stands_in_takes_the_games_unit_out_of_the_block() {
    let stem = "test_base_sfx_events";
    let mut game = audio_game();
    let objects = vec![
        sound(10, 1, 0),
        play(20, 10, stem),
        event("Play_sfx_Test_Dance", &[20]),
    ];
    game.insert(SFX_EVENTS, events_bank(stem, objects));
    let env = env(game, NoHistory);
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
    let own = "added_sfx_events";
    let objects = vec![sound(10, 77, 0), play(20, 10, own), event(squirt, &[20])];
    let media = media_bank("added_sfx_audio", CLIENT_VERSION, vec![clip(77, 7)]);
    let files = [
        (audio_at.as_str(), media),
        (events_at.as_str(), events_bank(own, objects)),
    ];
    let run = PortRun::of(&env, &mod_of(&[unit], &files));
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
    let made = &run.port.audio.units[0].units[0];
    let patched = block(&run.applied(&Layer::Base, SKIN0_BIN).objects);
    // The record holds the mod's unit and not the game's of that name, nor its banks.
    assert_eq!(names(&patched), [made.name.as_str()]);
    assert_eq!(patched.units[0].events, [squirt]);
    // The base's tag is no premium one: no event is renamed, and the mod's tag is added
    // for the hooks composed under it.
    assert_eq!(patched.tags.len(), 2);
}

/// A mod of the base's model and audio block whose voice package, clip 3 replaced, ships in
/// a WAD.
fn voice_mod(wad: &str) -> ModArchive {
    let voice = package(vec![clip(3, 7), clip(4, 4)]);
    mod_of(&[BASE_SFX, BASE_VO], &[]).file(wad, VO_WPK, voice)
}

#[test]
fn a_voice_layers_patch_sets_the_block_over_the_base_layers_result() {
    let env = env(voice_game(), NoHistory);
    let run = PortRun::of(&env, &voice_mod(WAD));
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
    let layer = Layer::Voice(None);
    let unit = &run.port.audio.units[0].units[0];

    // The banks are chunks of the layer, and the layer has a patch of its own on the bin.
    let package = run.package();
    let chunks: Vec<&str> = package
        .layer(&layer)
        .map(|(key, _)| key.path.as_str())
        .collect();
    let expected = ["vo_audio.bnk", "vo_audio.wpk", "vo_events.bnk"].map(|n| format!("{DIR}/{n}"));
    assert_eq!(chunks, expected);
    let patch = run.patch(&layer, SKIN0_BIN);
    assert_eq!(patch.path.as_str(), "overrides/test-skin0-voice.ptch");
    assert!(patch.links.is_empty());
    let manifest = package.manifest(&layer).expect("the layer's manifest");
    assert!(manifest.contains("test-skin0-voice.ptch"), "{manifest}");

    // With the layer off the game's block stands. With it on, the voice unit comes after
    // the game's.
    let base = block(&run.applied(&Layer::Base, SKIN0_BIN).objects);
    assert_eq!(names(&base), ["Test_Base_SFX", "Test_Base_VO"]);
    let voiced = block(&run.applied(&layer, SKIN0_BIN).objects);
    assert_eq!(
        names(&voiced),
        ["Test_Base_SFX", "Test_Base_VO", unit.name.as_str()]
    );
    assert!(voiced.units[2].voice);

    // On disk: the layer's directory holds its manifest, its patch and its banks.
    let out = tempfile::tempdir().expect("a directory");
    let out = camino::Utf8Path::from_path(out.path()).expect("a UTF-8 path");
    let mut log = Diagnostics::new();
    let writer: &dyn PackageWriter = &ProjectDir::new(out);
    let written = package.write(writer, "{}\n", &mut log.at(Stage::Write));
    written.expect("the package writes");
    let content = out.join(&package.config.name).join("content").join("voice");
    assert!(content.join(MANIFEST_FILE).is_file());
    let patch = content.join("overrides").join("test-skin0-voice.ptch");
    assert!(patch.is_file());
    assert!(content.join(WAD).join(&expected[1]).is_file());
}

#[test]
fn a_locales_voice_ships_in_the_locales_wad_and_layer() {
    let env = env(voice_game(), NoHistory);
    let run = PortRun::of(&env, &voice_mod(LOCALE_WAD));
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
    let layer = Layer::Voice(Some("en_us".into()));
    let package = run.package();
    let chunks: Vec<_> = package.layer(&layer).collect();
    assert_eq!(chunks.len(), 3, "{}", run.dump());
    let in_locale = |path: &str| path.starts_with(&format!("{DIR}/en_us/vo_"));
    assert!(
        chunks
            .iter()
            .all(|(key, _)| key.wad.as_str() == LOCALE_WAD && in_locale(key.path.as_str()))
    );
    let patch = run.patch(&layer, SKIN0_BIN);
    assert_eq!(patch.path.as_str(), "overrides/test-skin0-voice-en-us.ptch");
    let voiced = block(&run.applied(&layer, SKIN0_BIN).objects);
    assert!(voiced.units[2].name.ends_with("_en_us"));
}

/// What `PortBuilder` raises and builds from a run's mod and a plan, against a game.
fn built(run: &PortRun<'_>, port: &Port, game: &Env) -> (Diagnostics, Package) {
    let mut log = Diagnostics::new();
    let speller = run.facts.fix.speller();
    let builder = PortBuilder::new(&run.facts.fix.fixed, port, game, &speller);
    let package = builder.build(&mut log.at(Stage::Build));
    let package = package.unwrap_or_else(|error| panic!("no package: {error}\n{}", run.dump()));
    (log, package)
}

#[test]
fn a_games_entry_with_no_bank_unit_gives_no_model_for_the_ports_units() {
    let planned = env(audio_game(), NoHistory);
    let clips = vec![clip(1, 1), clip(2, 7), clip(9, 9)];
    let media = media_bank("test_base_sfx_audio", 134, clips);
    let run = PortRun::of(&planned, &mod_of(&[BASE_SFX], &[(SFX_AUDIO, media)]));
    assert_eq!(run.only::<Ported>().banks, 2, "{}", run.dump());

    // The plan adds a unit only where the game's entry has one, so the package is built
    // here against a game whose entry has no audio block.
    let silent = env(skins::game(), NoHistory);
    let (log, package) = built(&run, &run.port, &silent);
    let left = vec![NotCarried::NoUnitToModel { layer: Layer::Base }];
    assert_eq!(
        log.of::<AudioNotCarried>().collect::<Vec<_>>(),
        [&AudioNotCarried {
            unit: run.unit("test", 0),
            left,
        }]
    );
    // A bank ships only with the record that names its unit.
    let mut origins = package.chunks().map(|(_, _, chunk)| &chunk.origin);
    assert!(!origins.any(|origin| matches!(origin, Origin::Bank(_))));
    let ported = log.of::<Ported>().next().expect("the build's finding");
    assert_eq!((ported.banks, ported.left.recut), (0, 0));
}

#[test]
fn a_skin_bin_written_whole_takes_no_voice_layers_patch() {
    // The game's pet has a voice unit at its one skin. The mod's pet follows the champion to
    // skin 3, where the game ships no bin of it, so its bin is written whole.
    let pet_voice = Unit {
        name: "TestPet_Base_VO",
        ..BASE_VO
    };
    let pet_entry = "Characters/TestPet/Skins/Skin0";
    let voiced = with_audio(
        entry(pet_entry, &skins::pet_skin()),
        &["TestPet"],
        &[pet_voice],
    );
    let mut game = voice_game();
    game.insert(&skin_bin("testpet", 0), bin(vec![voiced]));
    let env = env(game, NoHistory);
    let pet = Skin {
        mesh: "assets/mod/pet/mesh.skn".into(),
        skeleton: "assets/mod/pet/skeleton.skl".into(),
        texture: "assets/mod/pet/body.tex".into(),
        ..skins::pet_skin()
    };
    let mut files = own_files("pet");
    let champion = "Characters/Test/Skins/Skin0";
    files.push(mod_skin("test", 0, champion, &skins::skin3(), vec![]));
    files.push(mod_skin("testpet", 0, pet_entry, &pet, vec![]));
    let run = PortRun::of(&env, &archive(&files));
    let unit = run.unit("testpet", 0);
    let target = "data/characters/testpet/skins/skin3.bin";
    assert!(
        matches!(run.chunk(target).origin, Origin::Made(_)),
        "{}",
        run.dump()
    );
    assert!(run.port.audio.is_empty(), "{}", run.dump());

    // The plan carries no audio to a slot the game has no skin of, so the voice unit is put
    // in the plan by hand.
    let made = "mods/synthetic.reforgetests/characters/testpet/skin3/audio/vo_audio.wpk";
    let mut port = run.port.clone();
    port.audio.units.push(UnitAudio {
        unit,
        units: vec![BankUnit {
            name: "TestPet_Mod_VO".into(),
            banks: vec![ChunkPath::parse(made).expect("a path")],
            events: Vec::new(),
            voice: true,
            layer: Layer::Voice(None),
            tag: None,
            replaces: None,
        }],
        tags: Vec::new(),
        renamed: BTreeMap::new(),
    });
    let (log, package) = built(&run, &port, &env);
    assert_eq!(
        log.of::<AudioNotCarried>().collect::<Vec<_>>(),
        [&AudioNotCarried {
            unit,
            left: vec![NotCarried::NewBin],
        }]
    );
    assert!(package.patches(&Layer::Voice(None)).is_empty());
    assert!(matches!(
        package
            .by_hash(path_hash(target))
            .map(|chunk| &chunk.origin),
        Some(Origin::Made(_))
    ));
}

#[test]
fn two_runs_of_a_mod_with_sounds_give_the_same_package_and_report() {
    let env = env(voice_game(), NoHistory);
    let archive = voice_mod(WAD);
    let (first, second) = (PortRun::of(&env, &archive), PortRun::of(&env, &archive));
    assert_eq!(first.package(), second.package());
    assert_eq!(first.report(), second.report());
}
