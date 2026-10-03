//! Binless mods (#MOD-ARTIFACT-0013): a champion WAD with no skin bin gets the game's bins of
//! the skins that name the files it overrides. The game is `reforge_testkit::skins`, changed
//! where a test says so.

use reforge::env::Env;
use reforge::load::{LoadError, LoadOptions, TakenFromGame, TakenSkin};
use reforge::model::entity::{FileKind, FileSource, FileUse, LinkTo};
use reforge::model::{Slot, path_hash};
use reforge_testkit::bins::{bin, blob, entry, graph, linked_bin, particle, resolver};
use reforge_testkit::skins::{
    self, BASE_CLIPS, BASE_MESH, BASE_TEX, S3_TEX, WAD, base_skin, skin_bin, skin3, skin7, skin9,
};
use reforge_testkit::{LoadRun, MemoryGame, MemoryHistory, ModArchive};

fn env_of(game: MemoryGame) -> Env {
    Env::builder().game(game).build().expect("an environment")
}

fn skin(character: &str, slot: u16) -> TakenSkin {
    TakenSkin {
        character: character.into(),
        slot: Slot(slot),
    }
}

/// The slots of the units of `test`.
fn slots(run: &LoadRun<'_>) -> Vec<u16> {
    run.module()
        .units
        .values()
        .map(|unit| unit.source_slot.0)
        .collect()
}

#[test]
fn a_binless_mod_takes_the_games_bin_of_the_skin_its_files_belong_to() {
    // No skin bin: the base's mesh and texture, the mod's own bytes at the game's paths.
    let archive = ModArchive::new()
        .file(WAD, BASE_MESH, b"the mod's own mesh".to_vec())
        .file(WAD, BASE_TEX, b"the mod's own texture".to_vec());
    let env = env_of(skins::game());
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let m = run.module();

    let taken = run.only::<TakenFromGame>();
    assert_eq!(taken.skins, [skin("test", 0)]);
    assert_eq!((taken.overridden, taken.unclaimed.len()), (2, 0));
    assert_eq!(
        (taken.former.len(), taken.never, taken.covered, taken.stale),
        (0, 0, 0, 0)
    );
    assert_eq!(slots(&run), [0]);

    let (bin, node) = run.file(&skin_bin("test", 0));
    assert_eq!(taken.bins, [bin]);
    assert_eq!(
        (node.source, node.kind, node.used),
        (FileSource::Game, FileKind::Bin, FileUse::Loaded)
    );
    assert_eq!(m.units[run.unit("test", 0)].file, bin);
    assert!(m.units[run.unit("test", 0)].entry.is_some());
    assert_eq!(run.file(BASE_MESH).1.used, FileUse::Referenced);
    assert_eq!(run.file(BASE_TEX).1.used, FileUse::Referenced);
}

#[test]
fn a_shared_bin_the_mod_ships_claims_the_skins_linking_it() {
    // The game's skin0 links a bin shared between skins. The mod ships only its own copy.
    const SHARED: &str = "data/characters/test/test_multi_skins_skin0_skins_skin3.bin";
    const R_TEX: &str = "assets/characters/test/skins/base/particles/r.tex";
    const R: &str = "Characters/Test/Skins/Skin0/Particles/Test_Base_R";
    let mut game = skins::game();
    let skin0 = vec![
        entry("Characters/Test/Skins/Skin0", &base_skin()),
        graph("Characters/Test/Animations/Skin0", BASE_CLIPS),
        resolver("Characters/Test/Skins/Skin0/Resources"),
    ];
    let links = ["DATA/Characters/Test/Test_Multi_Skins_Skin0_Skins_Skin3.bin"];
    game.insert(&skin_bin("test", 0), linked_bin(skin0, &links));
    game.insert(SHARED, bin(vec![particle(R, R_TEX)]));
    game.insert(R_TEX, blob(R_TEX));
    let archive = ModArchive::new().file(WAD, SHARED, bin(vec![particle(R, BASE_TEX)]));
    let env = env_of(game);
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let m = run.module();

    assert_eq!(run.only::<TakenFromGame>().skins, [skin("test", 0)]);
    assert_eq!(slots(&run), [0]);
    // Only the mod's copy of the shared bin, and the client loads it through the game's link.
    let copies: Vec<_> = m
        .files
        .values()
        .filter(|f| f.hash == path_hash(SHARED))
        .collect();
    assert_eq!(copies.len(), 1, "{}", run.dump());
    assert_eq!(
        (copies[0].source, copies[0].used),
        (FileSource::Archive, FileUse::Loaded)
    );
    let unit = &m.units[run.unit("test", 0)];
    assert_eq!(unit.loads.len(), 2);
    let skin_bin = unit.bin.expect("the game's skin bin reads");
    assert_eq!(
        m.bins[skin_bin].links[0].to,
        LinkTo::Mod(run.file(SHARED).0)
    );
}

#[test]
fn a_linked_bin_of_the_game_that_names_an_override_is_taken_with_the_skin_bin() {
    const ANIMATIONS: &str = "data/characters/test/animations/skin0.bin";
    let mut game = skins::game();
    let skin0 = vec![
        entry("Characters/Test/Skins/Skin0", &base_skin()),
        resolver("Characters/Test/Skins/Skin0/Resources"),
    ];
    let links = [
        "DATA/Characters/Test/Animations/Skin0.bin",
        "DATA/Characters/Test/Test.bin",
    ];
    game.insert(&skin_bin("test", 0), linked_bin(skin0, &links));
    game.insert(
        ANIMATIONS,
        bin(vec![graph("Characters/Test/Animations/Skin0", BASE_CLIPS)]),
    );
    game.insert(
        "data/characters/test/test.bin",
        bin(vec![particle("Characters/Test/Spells/Q", BASE_CLIPS[0])]),
    );
    let archive = ModArchive::new().file(WAD, BASE_CLIPS[0], b"the mod's own idle".to_vec());
    let env = env_of(game);
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let m = run.module();

    // The skin bin always, the animation bin for the clip it names, and never the root bin.
    let taken = run.only::<TakenFromGame>();
    assert_eq!(
        taken.bins,
        [run.file(&skin_bin("test", 0)).0, run.file(ANIMATIONS).0]
    );
    assert_eq!(m.units[run.unit("test", 0)].loads.len(), 2);
    assert!(
        m.files
            .values()
            .all(|f| f.hash != path_hash("data/characters/test/test.bin"))
    );
}

#[test]
fn a_file_in_a_skins_folder_goes_with_that_skin() {
    // Skin 3 and its chroma, skin 9, both name skin 3's texture: it is skin 3's.
    let archive = ModArchive::new().file(WAD, S3_TEX, b"a repainted skin 3".to_vec());
    let env = env_of(skins::game());
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert_eq!(run.only::<TakenFromGame>().skins, [skin("test", 3)]);
    assert_eq!(slots(&run), [3]);
}

#[test]
fn a_base_file_only_a_premium_skin_names_is_claimed_by_no_skin() {
    // A base mod is not spread onto a premium skin.
    const ONLY3: &str = "assets/characters/test/skins/base/particles/only3.tex";
    let mut game = skins::game();
    let objects = vec![
        entry("Characters/Test/Skins/Skin3", &skin3()),
        particle("Characters/Test/Skins/Skin3/Particles/P", ONLY3),
    ];
    game.insert(&skin_bin("test", 3), bin(objects));
    game.insert(ONLY3, blob(ONLY3));
    let archive = ModArchive::new().file(WAD, ONLY3, b"repainted".to_vec());
    let env = env_of(game);
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);

    assert_eq!(run.log.of::<TakenFromGame>().count(), 0);
    assert!(matches!(run.result, Err(LoadError::Halted(_))));
    assert_eq!(run.codes(), ["wad.not-carried", "archive.no-skin"]);
}

/// The game with a HUD icon, in no skin's folder, which the given skins of `test` name.
fn game_with_icon(icon: &str, named_by: &[u16]) -> MemoryGame {
    let mut game = skins::game();
    for &slot in named_by {
        let skin = match slot {
            0 => base_skin(),
            3 => skin3(),
            7 => skin7(),
            _ => skin9(),
        };
        let objects = vec![
            entry(&format!("Characters/Test/Skins/Skin{slot}"), &skin),
            particle(&format!("Characters/Test/Skins/Skin{slot}/Icon"), icon),
        ];
        game.insert(&skin_bin("test", slot), bin(objects));
    }
    game.insert(icon, blob(icon));
    game
}

#[test]
fn a_file_in_no_skins_folder_goes_with_the_base_when_the_base_names_it() {
    const ICON: &str = "assets/characters/test/hud/test_circle.tex";
    let archive = ModArchive::new().file(WAD, ICON, b"a new icon".to_vec());
    let env = env_of(game_with_icon(ICON, &[0, 7, 9]));
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert_eq!(run.only::<TakenFromGame>().skins, [skin("test", 0)]);
}

#[test]
fn a_file_in_no_skins_folder_goes_with_the_lowest_skin_naming_it_that_is_no_chroma() {
    const ICON: &str = "assets/characters/test/hud/test_circle.tex";
    let archive = ModArchive::new().file(WAD, ICON, b"a new icon".to_vec());
    let env = env_of(game_with_icon(ICON, &[7, 9]));
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert_eq!(run.only::<TakenFromGame>().skins, [skin("test", 7)]);

    // Named by a chroma alone, it goes with the chroma.
    let env = env_of(game_with_icon(ICON, &[9]));
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert_eq!(run.only::<TakenFromGame>().skins, [skin("test", 9)]);
}

#[test]
fn files_the_game_does_not_ship_are_counted_by_what_the_history_and_the_mod_say() {
    const DROPPED: &str = "assets/characters/test/skins/base/test_cape.tex";
    const OWN: &str = "assets/characters/test/skins/base/notes.tex";
    // The `.dds` twin of the base texture, which the mod ships as `.tex` too.
    const TWIN: &str = "assets/characters/test/skins/base/test.dds";
    let archive = ModArchive::new()
        .file(WAD, BASE_TEX, b"the mod's own texture".to_vec())
        .file(
            WAD,
            TWIN,
            b"the same texture in the dropped format".to_vec(),
        )
        .file(WAD, DROPPED, blob(DROPPED))
        .file(WAD, OWN, blob(OWN));
    let game = skins::game;

    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]).with_path(DROPPED, 0..=0);
    let env = Env::builder()
        .game(game())
        .history(Box::new(history))
        .build()
        .expect("an env");
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let taken = run.only::<TakenFromGame>();
    assert_eq!(taken.former, [path_hash(DROPPED)]);
    assert_eq!((taken.never, taken.covered, taken.history), (1, 1, true));
    assert_eq!(run.file(DROPPED).1.used, FileUse::FormerPath);

    // Without a history a former path is not told from one never the game's.
    let env = env_of(game());
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let taken = run.only::<TakenFromGame>();
    assert_eq!(
        (
            taken.former.len(),
            taken.never,
            taken.covered,
            taken.history
        ),
        (2, 0, 1, false)
    );

    // With no file fixup nothing says where the twin's asset is.
    let run = LoadRun::of(&env, LoadOptions::new().without_fixups(), &archive);
    assert_eq!(run.only::<TakenFromGame>().covered, 0);
}

#[test]
fn a_file_a_fixup_made_another_from_is_counted_through_the_file_made() {
    // An older binless mod: the base texture as `.dds`, where the game ships `.tex`.
    const OLD: &str = "assets/characters/test/skins/base/test.dds";
    let dds = [b"DDS ".as_slice(), &[0u8; 124]].concat();
    let archive = ModArchive::new().file(WAD, OLD, dds);
    let env = env_of(skins::game());
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);

    let taken = run.only::<TakenFromGame>();
    assert_eq!(
        (taken.overridden, taken.former.len(), taken.covered),
        (1, 0, 0)
    );
    let (old, _) = run.file(OLD);
    let (_, made) = run.file(BASE_TEX);
    assert!(matches!(made.source, FileSource::Made { from, .. } if from == old));
    assert_eq!(made.used, FileUse::Referenced);
    assert_eq!(run.codes()[0], "file.format-converted");
}

#[test]
fn files_equal_to_the_games_override_nothing() {
    let archive = ModArchive::new().file(WAD, BASE_MESH, blob(BASE_MESH));
    let env = env_of(skins::game());
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert_eq!(run.codes(), ["wad.not-carried", "archive.no-skin"]);
}

#[test]
fn a_wad_with_a_skin_bin_takes_none_of_the_games() {
    let archive = ModArchive::new()
        .file(
            WAD,
            &skin_bin("test", 3),
            bin(vec![entry("Characters/Test/Skins/Skin3", &skin3())]),
        )
        .file(WAD, BASE_MESH, b"the mod's own mesh".to_vec());
    let env = env_of(skins::game());
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert_eq!(run.log.of::<TakenFromGame>().count(), 0);
    assert_eq!(slots(&run), [3]);
    assert_eq!(run.file(BASE_MESH).1.used, FileUse::Override);
}

#[test]
fn a_locale_wad_and_a_wad_of_no_champion_take_none() {
    let archive = ModArchive::new()
        .file(
            "Test.en_US.wad.client",
            BASE_MESH,
            b"the mod's own mesh".to_vec(),
        )
        .file(
            "Shared.wad.client",
            BASE_TEX,
            b"the mod's own texture".to_vec(),
        );
    let env = env_of(skins::game());
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert_eq!(run.log.of::<TakenFromGame>().count(), 0);
    assert!(matches!(run.result, Err(LoadError::Halted(_))));
}
