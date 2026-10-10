//! The audio of a unit: the copies the archive ships of the banks its audio block names, what
//! each holds, the locale of its WAD, and whose each clip is. The game is
//! `reforge_testkit::sounds`.

use reforge::diag::Diag;
use reforge::facts::{BankFile, BankUnreadable, ClipOrigin, Facts, FactsFound, UnitSounds};
use reforge::history::{NoHistory, Position};
use reforge::model::{BankClip, BankRole};
use reforge_testkit::bins::{bin, blob, entry, graph, resolver};
use reforge_testkit::facts::env;
use reforge_testkit::skins::{self, BASE_CLIPS, WAD, skin_bin};
use reforge_testkit::sounds::{
    BASE_SFX, BASE_VO, SFX_AUDIO, SFX_EVENTS, Unit, VO_AUDIO, VO_WPK, audio_game, base_entry, clip,
    event, events_bank, media_bank, package, play, sound, stub, voice_game,
};
use reforge_testkit::{FactRun, MemoryHistory, ModArchive};
use reforge_wwise::{CLIENT_VERSION, Wem, id_of};

const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const LOCALE_WAD: &str = "Test.en_US.wad.client";

/// A mod of the base's model in one WAD, whose entry declares `units`, with these banks
/// beside its skin bin.
fn mod_in(wad: &str, units: &[Unit<'_>], banks: &[(&str, Vec<u8>)]) -> ModArchive {
    let objects = vec![
        base_entry(units),
        graph(GRAPH0, BASE_CLIPS),
        resolver(RESOURCES0),
    ];
    let archive = ModArchive::new().file(wad, &skin_bin("test", 0), bin(objects));
    archive.files(wad, banks)
}

/// The mod that ships a media bank at the base's path: clip 1 as the game has it, clip 2
/// replaced, clip 9 new, and the game's clip 1 again under the id 5.
fn over_the_base() -> ModArchive {
    let moved = Wem {
        id: 5,
        bytes: clip(1, 1).bytes,
    };
    let clips = vec![clip(1, 1), clip(2, 7), clip(9, 9), moved];
    let media = media_bank("test_base_sfx_audio", CLIENT_VERSION, clips);
    mod_in(WAD, &[BASE_SFX], &[(SFX_AUDIO, media)])
}

fn sounds<'a>(run: &'a FactRun<'_>) -> &'a UnitSounds {
    let sounds = run.facts.sounds.of(run.unit("test", 0));
    sounds.unwrap_or_else(|| panic!("the unit has audio\n{}", run.dump()))
}

fn origins(file: &BankFile) -> Vec<(u32, ClipOrigin)> {
    let clips = file.clips.iter();
    clips.map(|clip| (clip.id, clip.origin)).collect()
}

#[test]
fn each_copy_of_a_bank_is_sorted_by_what_it_holds() {
    let env = env(voice_game(), NoHistory);
    let stem = "test_base_sfx_events";
    let dance = vec![
        sound(10, 9, 0),
        play(20, 10, stem),
        event("Play_sfx_Test_Dance", &[20]),
    ];
    let media = media_bank("test_base_sfx_audio", CLIENT_VERSION, vec![clip(9, 9)]);
    let banks = [
        (SFX_AUDIO, media),
        (SFX_EVENTS, events_bank(stem, dance)),
        (VO_AUDIO, stub("test_base_vo_audio")),
        (VO_WPK, package(vec![clip(3, 3)])),
    ];
    let run = FactRun::of(&env, &mod_in(WAD, &[BASE_SFX, BASE_VO], &banks));
    let sounds = sounds(&run);
    assert_eq!(sounds.tags, ["Test"]);
    let held = |unit: usize| -> Vec<(BankRole, usize, usize)> {
        let files = sounds.units[unit].files.iter();
        files
            .map(|file| (file.role, file.objects, file.clips.len()))
            .collect()
    };
    assert_eq!(
        held(0),
        [(BankRole::Media, 0, 1), (BankRole::Events, 3, 0)],
        "{}",
        run.dump()
    );
    // The voice unit names an events bank too, which the mod does not ship: it has no copy.
    assert_eq!(held(1), [(BankRole::Stub, 0, 0), (BankRole::Package, 0, 1)]);
    let media = &sounds.units[0].files[0];
    assert_eq!(
        (media.path.as_str(), media.version, media.bank_id),
        (SFX_AUDIO, CLIENT_VERSION, id_of("test_base_sfx_audio"))
    );
    assert!(media.locale.is_none() && media.unreadable.is_none());
    assert!(sounds.units[1].unit.voice && !sounds.units[0].unit.voice);
}

#[test]
fn a_unit_whose_entry_holds_no_audio_block_has_no_sounds() {
    let env = env(voice_game(), NoHistory);
    let objects = vec![entry("Characters/Test/Skins/Skin0", &skins::base_skin())];
    let archive = ModArchive::new().file(WAD, &skin_bin("test", 0), bin(objects));
    let run = FactRun::of(&env, &archive);
    assert!(run.facts.sounds.of(run.unit("test", 0)).is_none());
    assert_eq!(run.facts.sounds.iter().count(), 0);
}

#[test]
fn the_sounds_a_snapshot_keeps_read_back_with_every_clip_the_report_only_counts() {
    let env = env(audio_game(), NoHistory);
    let run = FactRun::of(&env, &over_the_base());
    let json = serde_json::to_string(&run.facts.found()).expect("the facts write");
    let found: FactsFound = serde_json::from_str(&json).expect("the facts read");
    let reopened = Facts::reopen(&run.fix.fixed, &env, found).expect("the game answers");
    let unit = run.unit("test", 0);
    assert_eq!(reopened.sounds.of(unit), Some(sounds(&run)));
    assert_eq!(sounds(&run).units[0].files[0].clips.len(), 4);
}

#[test]
fn without_a_history_a_clip_is_the_games_where_its_bank_at_the_path_holds_it_under_the_id() {
    let env = env(audio_game(), NoHistory);
    let run = FactRun::of(&env, &over_the_base());
    let file = &sounds(&run).units[0].files[0];
    assert_eq!(
        origins(file),
        [
            (1, ClipOrigin::GameBank),
            (2, ClipOrigin::Own),
            (9, ClipOrigin::Own),
            (5, ClipOrigin::Own),
        ],
        "{}",
        run.dump()
    );
    assert_eq!(file.own_ids().into_iter().collect::<Vec<_>>(), [2, 5, 9]);
    assert_eq!((file.own(), sounds(&run).own()), (3, 3));
}

#[test]
fn with_a_history_a_clip_is_the_games_where_any_build_shipped_its_bytes() {
    // The game shipped the mod's clip 2 from the second build on, and its clip 1 under any
    // id. The history does not hold clip 9.
    let replaced = BankClip::of(2, &clip(2, 7).bytes);
    let installed = BankClip::of(1, &clip(1, 1).bytes);
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"])
        .with_clip(replaced.key, 1..=1)
        .with_clip(installed.key, 0..=1);
    let env = env(audio_game(), history);
    let run = FactRun::of(&env, &over_the_base());
    let file = &sounds(&run).units[0].files[0];
    let (first, second) = (Position(0), Position(1));
    assert_eq!(
        origins(file),
        [
            (1, ClipOrigin::Game { first }),
            (2, ClipOrigin::Game { first: second }),
            (9, ClipOrigin::Own),
            (5, ClipOrigin::Game { first }),
        ],
        "{}",
        run.dump()
    );
    assert_eq!(file.own_ids().into_iter().collect::<Vec<_>>(), [9]);

    // The install's bank is not asked: a clip it holds that the history lacks is the mod's.
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]);
    let env = reforge_testkit::facts::env(audio_game(), history);
    let run = FactRun::of(&env, &over_the_base());
    let file = &sounds(&run).units[0].files[0];
    assert_eq!(origins(file)[0], (1, ClipOrigin::Own));
}

#[test]
fn a_bank_that_does_not_read_is_reported_and_its_clips_are_not_judged() {
    let env = env(audio_game(), NoHistory);
    let banks = [
        (SFX_AUDIO, blob("no bank")),
        (SFX_EVENTS, events_bank("test_base_sfx_events", Vec::new())),
    ];
    let run = FactRun::of(&env, &mod_in(WAD, &[BASE_SFX], &banks));
    let files = &sounds(&run).units[0].files;
    let reason = files[0].unreadable.clone();
    assert!(reason.is_some(), "{}", run.dump());
    assert!(files[0].clips.is_empty() && files[1].unreadable.is_none());

    let unreadable: Vec<BankUnreadable> = run.found();
    assert_eq!(unreadable.len(), 1);
    assert_eq!(unreadable[0].unit, run.unit("test", 0));
    let banks: Vec<_> = unreadable[0].banks.iter().collect();
    assert_eq!(banks.len(), 1);
    assert_eq!(
        (banks[0].file, Some(&banks[0].reason)),
        (files[0].file, reason.as_ref())
    );
    assert_eq!(BankUnreadable::CODE.name, "audio.bank-unreadable");
}

#[test]
fn a_bank_shipped_in_two_wads_is_two_copies_each_with_its_wads_locale() {
    let env = env(voice_game(), NoHistory);
    let archive = mod_in(WAD, &[BASE_VO], &[(VO_WPK, package(vec![clip(3, 3)]))]);
    let localized = package(vec![clip(3, 3), clip(4, 8)]);
    let archive = archive.file(LOCALE_WAD, VO_WPK, localized);
    let run = FactRun::of(&env, &archive);
    let files = &sounds(&run).units[0].files;
    let copies: Vec<(Option<&str>, usize)> = files
        .iter()
        .map(|file| (file.locale.as_deref(), file.own()))
        .collect();
    // In the order of the mod's WADs, which is by name.
    assert_eq!(copies, [(Some("en_us"), 1), (None, 0)], "{}", run.dump());
}

// #MOD-ARTIFACT-0010: what the game localizes keeps its locale.
#[test]
fn in_a_mod_packed_into_a_locale_wad_only_what_the_game_localizes_keeps_the_locale() {
    // A package the game ships in the locale's WAD, at a path outside the locale's folder.
    let lines = "assets/sounds/wwise2016/vo/characters/test/skins/base/test_base_lines.wpk";
    let mut game = voice_game();
    game.insert_in(lines, package(vec![clip(5, 5)]), &[LOCALE_WAD]);
    let env = env(game, NoHistory);
    let voice = Unit {
        banks: &[VO_WPK, lines],
        ..BASE_VO
    };
    let media = media_bank("test_base_sfx_audio", CLIENT_VERSION, vec![clip(9, 9)]);
    let banks = [
        (SFX_AUDIO, media),
        (VO_WPK, package(vec![clip(3, 3)])),
        (lines, package(vec![clip(5, 5)])),
    ];
    let run = FactRun::of(&env, &mod_in(LOCALE_WAD, &[BASE_SFX, voice], &banks));
    let locale = |unit: usize, file: usize| {
        let file = &sounds(&run).units[unit].files[file];
        file.locale.as_deref()
    };
    assert_eq!(locale(0, 0), None, "{}", run.dump());
    assert_eq!(locale(1, 0), Some("en_us"), "under the locale's folder");
    assert_eq!(locale(1, 1), Some("en_us"), "in the game's locale WAD");
}
