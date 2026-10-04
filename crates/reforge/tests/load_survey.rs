//! The survey: whose WAD each WAD is, the characters of a champion's WAD, one unit per skin bin.
//! The game is `reforge_testkit::skins`.

use reforge::env::Env;
use reforge::load::{
    ChampionAmbiguous, ChampionGuessed, EqualToGame, LoadError, LoadOptions, LocalePacked,
    LocaleWad, NoSkin, SeveralChampions, WadDuplicate, WadDuplicateDiffers, WadNotCarried,
};
use reforge::model::Slot;
use reforge::model::entity::{CharKind, WadId, WadRole};
use reforge_testkit::bins::{Skin, bin, blob, entry};
use reforge_testkit::skins::{self, BASE_TEX, WAD, base_skin, skin_bin, skin3};
use reforge_testkit::{LoadRun, MemoryGame, ModArchive};

fn env_of(game: MemoryGame) -> Env {
    Env::builder().game(game).build().expect("an environment")
}

fn env() -> Env {
    env_of(skins::game())
}

/// No file fixup: the survey is by path.
fn options() -> LoadOptions {
    LoadOptions::new().without_fixups()
}

/// A skin bin of `test` at a slot, holding the skin's entry under the slot's own name.
fn test_bin(slot: u16, skin: &Skin) -> (String, Vec<u8>) {
    let name = format!("Characters/Test/Skins/Skin{slot}");
    (skin_bin("test", slot), bin(vec![entry(&name, skin)]))
}

/// A skin entry of a character the game does not know, at slot 0.
fn other_bin() -> (String, Vec<u8>) {
    let name = "Characters/Other/Skins/Skin0";
    (skin_bin("other", 0), bin(vec![entry(name, &base_skin())]))
}

/// The game with a second champion, `other`, in a WAD of its own name.
fn game_with_other() -> MemoryGame {
    let mut game = skins::game();
    let (path, bytes) = other_bin();
    game.insert_in(&path, bytes, &["Other.wad.client"]);
    game
}

fn role(run: &LoadRun<'_>, wad: usize) -> WadRole {
    run.module().wads[WadId::new(wad)].role.clone()
}

fn champion(name: &str) -> WadRole {
    WadRole::Champion {
        champion: name.into(),
    }
}

#[test]
fn a_champion_wad_gets_its_character_and_a_unit_per_skin_bin() {
    let (base, base_bytes) = test_bin(0, &base_skin());
    let (three, three_bytes) = test_bin(3, &skin3());
    let archive = ModArchive::new()
        .file(WAD, &three, three_bytes)
        .file(WAD, &base, base_bytes)
        .file(WAD, "data/characters/test/test.bin", bin(vec![]));
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    assert_eq!(role(&run, 0), champion("test"));
    assert_eq!(m.characters.len(), 1);
    let (id, test) = m.characters.iter().next().expect("the champion");
    assert_eq!(test.name, "test");
    assert_eq!(test.kind, CharKind::Champion);
    assert_eq!(
        test.root_bin,
        Some(run.file("data/characters/test/test.bin").0)
    );
    let slots: Vec<Slot> = test.units.iter().map(|&u| m.units[u].source_slot).collect();
    assert_eq!(slots, [Slot(0), Slot(3)], "units by slot");
    assert_eq!(m.groups.len(), 1);
    assert_eq!(
        (m.groups[0].champion, &m.groups[0].characters),
        (id, &vec![id])
    );
    assert_eq!(m.champion_of(id), id);
}

#[test]
fn a_wad_named_for_no_champion_is_taken_as_the_champion_whose_skin_bins_it_ships() {
    // #MOD-ARTIFACT-0011: a WAD under the mod's name.
    let (path, bytes) = test_bin(0, &base_skin());
    let archive = ModArchive::new()
        .file("Demon Test.wad.client", &path, bytes)
        .file("Demon Test.wad.client", BASE_TEX, blob("recoloured"));
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);

    let guessed = run.only::<ChampionGuessed>();
    assert_eq!(guessed.champion, "test");
    assert_eq!(role(&run, 0), champion("test"));
    assert_eq!(
        run.module().wads[guessed.wad].name.as_str(),
        "Demon Test.wad.client"
    );
    assert_eq!(run.module().units.len(), 1);
}

#[test]
fn a_wad_whose_skin_bins_are_of_no_champion_of_the_game_is_not_guessed() {
    let (path, bytes) = other_bin();
    let archive = ModArchive::new()
        .file("Remix.wad.client", &path, bytes)
        .file("Remix.wad.client", BASE_TEX, blob("recoloured"));
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);

    assert!(matches!(run.result, Err(LoadError::Halted(_))));
    assert_eq!(run.codes(), ["wad.not-carried", "archive.no-skin"]);
    assert_eq!(run.only::<NoSkin>().wads, 1);
    assert_eq!(
        run.log.halted().map(|h| h.0),
        run.log.iter().last().map(|d| d.id)
    );
}

#[test]
fn a_wad_shipping_only_a_sub_characters_bins_is_not_guessed() {
    // #MOD-ARTIFACT-0011: the game keeps `testpet` in the champion's WAD, so it is no champion.
    let archive = ModArchive::new().file("Pet.wad.client", &skin_bin("testpet", 0), bin(vec![]));
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    assert_eq!(run.codes(), ["wad.not-carried", "archive.no-skin"]);
}

#[test]
fn a_wad_named_for_no_champion_with_the_skin_bins_of_several_is_taken_as_none() {
    // #MOD-ARTIFACT-0011: the bins' paths name two champions.
    let (test, test_bytes) = test_bin(0, &base_skin());
    let (other, other_bytes) = other_bin();
    let archive = ModArchive::new()
        .file("Pack.wad.client", &test, test_bytes)
        .file("Pack.wad.client", &other, other_bytes);
    let env = env_of(game_with_other());
    let run = LoadRun::of(&env, options(), &archive);

    assert_eq!(run.only::<ChampionAmbiguous>().champions, ["other", "test"]);
    assert_eq!(
        run.codes(),
        [
            "wad.champion-ambiguous",
            "wad.not-carried",
            "archive.no-skin"
        ]
    );
}

#[test]
fn a_guess_does_not_take_a_champion_whose_own_wad_is_carried() {
    let (path, bytes) = test_bin(0, &base_skin());
    let archive = ModArchive::new()
        .file("Extras.wad.client", &path, bytes.clone())
        .file(WAD, &path, bytes);
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);

    assert_eq!(run.log.of::<ChampionGuessed>().count(), 0);
    assert_eq!(role(&run, 0), WadRole::Other);
    assert_eq!(role(&run, 1), champion("test"));
    assert_eq!(run.only::<WadNotCarried>().wad, WadId::new(0));
}

#[test]
fn of_two_wads_for_one_champion_the_one_the_game_spells_is_carried() {
    // #MOD-ARTIFACT-0012: every path of the `.wad` is in the `.wad.client`.
    let (path, bytes) = test_bin(0, &base_skin());
    let archive = ModArchive::new()
        .file("Test.wad", &path, bytes.clone())
        .file(WAD, &path, bytes);
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);

    // WADs are in lowercase name order: `test.wad`, then `test.wad.client`.
    let duplicate = run.only::<WadDuplicate>();
    assert_eq!(
        (duplicate.wad, duplicate.carried),
        (WadId::new(0), WadId::new(1))
    );
    let message = run
        .messages()
        .into_iter()
        .find(|m| m.starts_with("wad.duplicate"));
    let message = message.expect("the finding prints");
    assert!(message.contains("beside Test.wad.client;"), "{message}");
    assert_eq!(role(&run, 0), WadRole::Other);
    assert_eq!(role(&run, 1), champion("test"));
    assert_eq!(run.module().units.len(), 1);
}

#[test]
fn a_second_wad_with_files_the_carried_one_lacks_is_a_warning() {
    let (path, bytes) = test_bin(0, &base_skin());
    let archive = ModArchive::new()
        .file("Test.wad", &path, bytes.clone())
        .file("Test.wad", BASE_TEX, blob("only in the second WAD"))
        .file(WAD, &path, bytes);
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);

    // #MOD-ARTIFACT-0012: the `.wad` holds a file at a path the `.wad.client` lacks.
    let differs = run.only::<WadDuplicateDiffers>();
    assert_eq!(
        (differs.wad, differs.carried, differs.champion.as_str()),
        (WadId::new(0), WadId::new(1), "test")
    );
    assert_eq!(role(&run, 0), WadRole::Other);
    assert_eq!(differs.files.len(), 1);
    assert_eq!(
        run.module().files[differs.files[0]].hash,
        reforge::model::path_hash(BASE_TEX)
    );
    assert_eq!(run.log.of::<WadDuplicate>().count(), 0);
}

#[test]
fn of_two_wads_neither_spelled_as_the_game_spells_it_the_first_is_carried() {
    let (path, bytes) = test_bin(0, &base_skin());
    let archive = ModArchive::new()
        .file("Test.wad", &path, bytes.clone())
        .file("Test.wad.mobile", &path, bytes);
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    assert_eq!(run.only::<WadDuplicate>().carried, WadId::new(0));
}

#[test]
fn a_locale_wad_beside_its_champions_keeps_its_locale() {
    let (path, bytes) = test_bin(0, &base_skin());
    let voice =
        "assets/sounds/wwise2016/vo/en_us/characters/test/skins/base/test_base_vo_audio.wpk";
    let archive = ModArchive::new().file(WAD, &path, bytes).file(
        "Test.en_US.wad.client",
        voice,
        blob("voice"),
    );
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);

    let locale = run.only::<LocaleWad>();
    assert_eq!(
        (locale.champion.as_str(), locale.locale.as_str()),
        ("test", "en_us")
    );
    let expected = WadRole::Locale {
        champion: "test".into(),
        locale: "en_us".into(),
    };
    assert_eq!(run.module().wads[locale.wad].role, expected);
    assert_eq!(run.module().groups.len(), 1);
}

#[test]
fn a_mod_packed_into_a_locale_wad_is_the_champions() {
    // #MOD-ARTIFACT-0010: the champion's skin bin in a locale WAD, and no champion WAD.
    let (path, bytes) = test_bin(0, &base_skin());
    let archive = ModArchive::new().file("Test.en_US.wad.client", &path, bytes);
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);

    let packed = run.only::<LocalePacked>();
    assert_eq!(
        (packed.champion.as_str(), packed.locale.as_str()),
        ("test", "en_us")
    );
    let wad = &run.module().wads[packed.wad];
    assert_eq!(wad.role, champion("test"));
    assert_eq!(wad.packed_locale.as_deref(), Some("en_us"));
    assert_eq!(run.module().units.len(), 1);
    assert_eq!(run.log.of::<LocaleWad>().count(), 0);
}

#[test]
fn a_locale_wad_with_skin_bins_beside_the_champions_wad_is_a_locale_wad() {
    // #MOD-ARTIFACT-0010: beside its champion's WAD a locale WAD is no artifact.
    let (path, bytes) = test_bin(0, &base_skin());
    let archive = ModArchive::new().file(WAD, &path, bytes.clone()).file(
        "Test.en_US.wad.client",
        &path,
        bytes,
    );
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    assert_eq!(
        run.codes()
            .iter()
            .filter(|c| c.starts_with("wad."))
            .collect::<Vec<_>>(),
        [&"wad.locale"]
    );
    assert_eq!(run.module().units.len(), 1);
}

#[test]
fn a_sub_character_is_found_by_the_paths_the_archive_spells() {
    let (path, bytes) = test_bin(0, &base_skin());
    let pet = entry("Characters/TestPet/Skins/Skin0", &skins::pet_skin());
    let archive = ModArchive::new().file(WAD, &path, bytes).file(
        WAD,
        &skin_bin("testpet", 0),
        bin(vec![pet]),
    );
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    let names: Vec<&str> = m.characters.values().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["test", "testpet"], "the champion first");
    let (champion, _) = m.characters.iter().next().expect("the champion");
    let (pet, node) = m.characters.iter().nth(1).expect("the sub-character");
    assert_eq!(node.kind, CharKind::Sub { champion });
    assert_eq!(m.champion_of(pet), champion);
    assert_eq!(m.groups[0].characters, [champion, pet]);
    assert!(m.equal_to_game.is_empty());
    assert_eq!(m.unit_name(run.unit("testpet", 0)), "testpet skin0");
}

#[test]
fn a_sub_character_whose_skin_bins_are_the_games_is_left_to_the_game() {
    let (path, bytes) = test_bin(0, &base_skin());
    let archive = ModArchive::new().file(WAD, &path, bytes).file(
        WAD,
        &skin_bin("testpet", 0),
        skins::pet_bin(),
    );
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    let equal = run.only::<EqualToGame>();
    assert_eq!(m.characters[equal.character].name, "testpet");
    assert_eq!(equal.units, [run.unit("testpet", 0)]);
    assert!(m.equal_to_game.contains(&equal.character));
    // The unit stays, loaded as any: a later stage leaves it out.
    assert!(m.units[run.unit("testpet", 0)].entry.is_some());
}

#[test]
fn several_champion_wads_are_several_groups() {
    let (test, test_bytes) = test_bin(0, &base_skin());
    let (other, other_bytes) = other_bin();
    let archive = ModArchive::new()
        .file(WAD, &test, test_bytes)
        .file("Other.wad.client", &other, other_bytes)
        .file("Map11.wad.client", "data/maps/x.bin", bin(vec![]));
    let env = env_of(game_with_other());
    let run = LoadRun::of(&env, options(), &archive);

    // WADs in lowercase name order: map11, other, test.
    let m = run.module();
    let several = run.only::<SeveralChampions>();
    let names: Vec<&str> = several
        .champions
        .iter()
        .map(|&c| m.characters[c].name.as_str())
        .collect();
    assert_eq!(names, ["other", "test"]);
    let message = run
        .messages()
        .into_iter()
        .find(|m| m.starts_with("wad.several-champions"));
    assert_eq!(
        message.as_deref(),
        Some("wad.several-champions 2 champion WADs: other, test")
    );
    assert_eq!(run.only::<WadNotCarried>().wad, WadId::new(0));
    assert_eq!(run.module().groups.len(), 2);
    assert_eq!(role(&run, 1), champion("other"));
}

#[test]
fn a_mod_for_a_premium_slot_only_is_a_skin_mod() {
    let (path, bytes) = test_bin(3, &skin3());
    let archive = ModArchive::new().file(WAD, &path, bytes);
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let unit = run.unit("test", 3);
    assert_eq!(run.module().units[unit].entry_name_slot, Some(Slot(3)));
}

#[test]
fn a_packed_wad_is_surveyed_by_its_hashes_and_its_skin_bin_gets_its_path() {
    let (path, bytes) = test_bin(0, &base_skin());
    let archive = ModArchive::new().packed(WAD, &[(path.as_str(), bytes.as_slice())]);
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let (_, file) = run.file(&path);
    assert_eq!(file.path.as_ref().map(|p| p.as_str()), Some(path.as_str()));
    assert_eq!(run.module().units.len(), 1);
}
