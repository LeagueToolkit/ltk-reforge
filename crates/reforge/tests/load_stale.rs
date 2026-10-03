//! Stale copies (#MOD-ARTIFACT-0004): a file at a game path holding the bytes the game shipped
//! there at an earlier build is no override, unless the options keep it or the WAD restores a
//! model from before its champion's rework. The game is `reforge_testkit::skins`. Its history
//! here has two builds, and at the first the game held other bytes at the base texture.

use reforge::env::Env;
use reforge::history::History;
use reforge::load::{
    LoadError, LoadOptions, Restoration, ReworkTable, StaleFiles, Staleness, TakenFromGame,
};
use reforge::model::entity::{FileSource, FileUse};
use reforge_testkit::bins::{bin, entry};
use reforge_testkit::skins::{self, BASE_MESH, BASE_TEX, WAD, base_skin, skin_bin};
use reforge_testkit::{BrokenHistory, LoadRun, MemoryHistory, ModArchive};

const OLD_TEX: &[u8] = b"the base texture as the game shipped it at 16.18";
const OTHER: &str = "assets/characters/test/skins/skin03/test_skin03_old.tex";
const OTHER_TEX: &[u8] = b"skin 3's texture as the game shipped it at 16.18";
const OLD_MODEL: &str = "assets/characters/test/skins/base/test_old.skn";
const NEW_MODEL: &str = "assets/characters/test/skins/base/test_rework.skn";
const OWN_MESH: &[u8] = b"the mod's own mesh";

/// 16.18 and 16.19. The game shipped `OLD_TEX` at the base texture's path and `OTHER_TEX` at
/// skin 3's at 16.18, an old model's mesh at 16.18 only, and a reworked one from 16.19.
fn history() -> MemoryHistory {
    MemoryHistory::new(&["16.18.1", "16.19.1"])
        .with_content(BASE_TEX, OLD_TEX, 0..=0)
        .with_path(BASE_TEX, 0..=1)
        .with_content(OTHER, OTHER_TEX, 0..=0)
        .with_path(OLD_MODEL, 0..=0)
        .with_path(NEW_MODEL, 1..=1)
}

fn env_with(history: impl History + 'static) -> Env {
    Env::builder()
        .game(skins::game())
        .history(Box::new(history))
        .build()
        .expect("an environment")
}

/// `test` reworked at 16.19, and no file fixup.
fn options() -> LoadOptions {
    LoadOptions::new()
        .without_fixups()
        .reworks(ReworkTable::new([("test", "16.19")]))
}

/// A binless mod: its own mesh, and `texture` at the base texture's path.
fn binless(texture: &[u8]) -> ModArchive {
    ModArchive::new()
        .file(WAD, BASE_MESH, OWN_MESH)
        .file(WAD, BASE_TEX, texture)
}

#[test]
fn a_stale_copy_is_no_override() {
    let env = env_with(history());
    let run = LoadRun::of(&env, options(), &binless(OLD_TEX));

    let (texture, node) = run.file(BASE_TEX);
    assert_eq!(node.used, FileUse::Stale, "{}", run.dump());
    let stale = run.only::<StaleFiles>();
    assert_eq!(
        (stale.files.as_slice(), stale.bytes),
        ([texture].as_slice(), OLD_TEX.len() as u64)
    );
    // The mesh, the mod's own, is the one override, and the base's bin is taken for it.
    let taken = run.only::<TakenFromGame>();
    assert_eq!((taken.overridden, taken.stale), (1, 1));
    assert_eq!(run.file(BASE_MESH).1.used, FileUse::Referenced);
}

#[test]
fn the_options_keep_stale_copies() {
    let env = env_with(history());
    let run = LoadRun::of(&env, options().keep_stale(true), &binless(OLD_TEX));
    assert_eq!(run.file(BASE_TEX).1.used, FileUse::Referenced);
    assert_eq!(run.log.of::<StaleFiles>().count(), 0);
    assert_eq!(run.only::<TakenFromGame>().overridden, 2);
}

#[test]
fn bytes_the_game_held_under_another_path_are_the_authors_choice() {
    let env = env_with(history());
    let run = LoadRun::of(&env, options(), &binless(OTHER_TEX));
    assert_eq!(run.file(BASE_TEX).1.used, FileUse::Referenced);
}

#[test]
fn without_a_history_nothing_is_known_to_be_stale() {
    let env = Env::builder()
        .game(skins::game())
        .build()
        .expect("an environment");
    let run = LoadRun::of(&env, options(), &binless(OLD_TEX));
    assert_eq!(run.file(BASE_TEX).1.used, FileUse::Referenced);
    assert!(!run.only::<TakenFromGame>().history);
}

#[test]
fn a_stale_copy_is_stale_whatever_names_it() {
    // #ADR-0023: the mod's own skin bin names the base texture, and the copy is still stale.
    let archive = ModArchive::new()
        .file(
            WAD,
            &skin_bin("test", 0),
            bin(vec![entry("Characters/Test/Skins/Skin0", &base_skin())]),
        )
        .file(WAD, BASE_TEX, OLD_TEX);
    let env = env_with(history());
    let run = LoadRun::of(&env, options(), &archive);
    assert_eq!(run.file(BASE_TEX).1.used, FileUse::Stale);
}

#[test]
fn the_games_own_bytes_at_their_path_are_not_stale() {
    let archive = ModArchive::new()
        .file(
            WAD,
            &skin_bin("test", 0),
            bin(vec![entry("Characters/Test/Skins/Skin0", &base_skin())]),
        )
        .file(WAD, BASE_MESH, reforge_testkit::bins::blob(BASE_MESH));
    let history = history().with_content(BASE_MESH, &reforge_testkit::bins::blob(BASE_MESH), 0..=1);
    let env = env_with(history);
    let run = LoadRun::of(&env, options(), &archive);
    assert_eq!(run.file(BASE_MESH).1.used, FileUse::Referenced);
}

#[test]
fn an_old_model_restored_keeps_its_copies_of_old_bytes() {
    // The old model's mesh beside a file only the rework has: made after the rework.
    let archive = binless(OLD_TEX)
        .file(WAD, OLD_MODEL, b"the old model".to_vec())
        .file(
            WAD,
            NEW_MODEL,
            b"the mod's file at the reworked model's path".to_vec(),
        );
    let env = env_with(history());
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    let restoration = run.only::<Restoration>();
    assert_eq!(
        (restoration.champion.as_str(), restoration.patch.as_str()),
        ("test", "16.19")
    );
    assert_eq!((restoration.older, restoration.newer), (1, 1));
    assert!(m.restorations.contains(&restoration.wad));
    assert_eq!(run.file(BASE_TEX).1.used, FileUse::Referenced);
    assert_eq!(run.log.of::<StaleFiles>().count(), 0);
    // The game's bin taken for the mod is not the archive's file: it counts on neither side.
    assert_eq!(run.file(&skin_bin("test", 0)).1.source, FileSource::Game);
}

#[test]
fn work_from_before_the_rework_is_no_restoration() {
    // Files at paths the rework dropped, and none at a path only the rework has.
    let archive = binless(OLD_TEX).file(WAD, OLD_MODEL, b"the old model".to_vec());
    let env = env_with(history());
    let run = LoadRun::of(&env, options(), &archive);
    assert_eq!(run.log.of::<Restoration>().count(), 0);
    assert_eq!(run.file(BASE_TEX).1.used, FileUse::Stale);
}

#[test]
fn a_champion_with_no_rework_is_judged_by_the_history_alone() {
    let archive = binless(OLD_TEX)
        .file(WAD, OLD_MODEL, b"the old model".to_vec())
        .file(WAD, NEW_MODEL, b"a newer file".to_vec());
    let env = env_with(history());
    let options = LoadOptions::new().without_fixups();
    let run = LoadRun::of(&env, options, &archive);
    assert_eq!(run.log.of::<Restoration>().count(), 0);
    assert_eq!(run.file(BASE_TEX).1.used, FileUse::Stale);
}

#[test]
fn the_judge_answers_for_one_file() {
    let env = env_with(history());
    let run = LoadRun::of(&env, options(), &binless(OLD_TEX));
    let judge = Staleness::new(&env, &options());
    let stale = |path: &str| {
        judge
            .of(run.module(), run.file(path).0)
            .expect("the history answers")
    };
    assert!(stale(BASE_TEX));
    assert!(!stale(BASE_MESH));
}

#[test]
fn a_history_that_does_not_answer_ends_the_load_with_a_fault() {
    let env = env_with(BrokenHistory(history()));
    let run = LoadRun::of(&env, options(), &binless(OLD_TEX));
    assert!(matches!(run.result, Err(LoadError::History(_))));
    assert!(run.log.halted().is_none(), "a fault is no diagnostic");
}

#[test]
fn the_standard_table_holds_the_reworks_of_the_record() {
    // #GAME-CHANGE-0001: each champion with the patch its new model shipped in.
    let table = ReworkTable::standard();
    let rows = [
        ("ahri", "13.3"),
        ("caitlyn", "11.23"),
        ("drmundo", "11.12"),
        ("fiddlesticks", "10.7"),
        ("jax", "13.20"),
        ("leblanc", "15.7"),
        ("leesin", "14.9"),
        ("mordekaiser", "9.12"),
        ("pantheon", "9.16"),
        ("shyvana", "16.6"),
        ("skarner", "14.7"),
        ("teemo", "14.20"),
        ("udyr", "12.16"),
        ("viktor", "14.24"),
        ("volibear", "10.11"),
        ("xinzhao", "15.17"),
    ];
    for (champion, patch) in rows {
        assert_eq!(table.patch(champion), Some(patch), "{champion}");
    }
    assert_eq!(table, ReworkTable::new(rows));
    assert_eq!(table.patch("Pantheon"), Some("9.16"), "in any case");
    assert_eq!(table.patch("vi"), None, "a re-export is no rework");
}
