//! What a package holds (#ADR-0039): each reason a file of the mod stays behind, counted in
//! the builder's finding, and each way a file ships. A repair is built by `InPlaceBuilder`, a
//! port by `PortBuilder`. The game is `reforge_testkit::skins`, with a build before it in the
//! history.

use ltk_meta::BinObject;
use reforge::env::Env;
use reforge::fixups::{FixupSet, Rule};
use reforge::history::NoHistory;
use reforge::model::path_hash;
use reforge::package::{InPlace, LeftBehind, LeftOut, Origin, Ported};
use reforge::port::FileVerdict;
use reforge_testkit::bins::{Skin, bin, blob, entry, object, particle, text};
use reforge_testkit::facts::{archive, env as port_env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::packs::{DdsPixels, dds};
use reforge_testkit::skins::{self, BASE_PARTICLE_TEX, BASE_TEX, S3_TEX, WAD, base_skin, skin_bin};
use reforge_testkit::sounds::{BASE_SFX, SFX_AUDIO, audio_game, base_entry, clip, media_bank};
use reforge_testkit::{MemoryHistory, ModArchive, PackRun, PortRun};

const ENTRY: &str = "Characters/Test/Skins/Skin0";
const GRAPH: &str = "Characters/Test/Animations/Skin0";
const RESOURCES: &str = "Characters/Test/Skins/Skin0/Resources";
const SKIN0_BIN: &str = "data/characters/test/skins/skin0.bin";
/// A texture at a path of the mod's own.
const OWN_TEX: &str = "assets/mod/a/own.tex";
/// A texture at a path the game shipped at the first build of the history, and not since.
const FORMER_TEX: &str = "assets/characters/test/skins/base/test_tx_cm.dds";
/// The base texture as the game shipped it at the first build of the history.
const OLD_BYTES: &[u8] = b"the base texture at 16.18";

/// The counts of these reasons, one file each.
fn left(reasons: &[LeftOut]) -> LeftBehind {
    let mut left = LeftBehind::default();
    for reason in reasons {
        left.count(*reason);
    }
    left
}

/// The game with a build before it: the game shipped a path it does not ship, and other
/// bytes at the base texture's path.
fn env() -> Env {
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"])
        .with_path(FORMER_TEX, 0..=0)
        .with_content(BASE_TEX, OLD_BYTES, 0..=0);
    Env::builder()
        .game(skins::game())
        .history(Box::new(history))
        .build()
        .expect("an environment")
}

/// A repair of the base skin's bin, holding these objects beside its entry, and one more
/// file. No fixup runs.
fn repair<'a>(env: &'a Env, extra: Vec<BinObject>, path: &str, bytes: &[u8]) -> PackRun<'a> {
    let mut objects = vec![entry(ENTRY, &base_skin())];
    objects.extend(extra);
    let archive = ModArchive::new()
        .file(WAD, SKIN0_BIN, bin(objects))
        .file(WAD, path, bytes);
    PackRun::of(env, &FixupSet::empty(), &archive)
}

#[test]
fn a_file_nothing_loads_or_names_stays_behind_as_unused() {
    let env = env();
    let run = repair(&env, vec![], OWN_TEX, &blob(OWN_TEX));
    assert!(!run.ships(OWN_TEX));
    let found = run.only::<InPlace>();
    assert_eq!(
        found.left,
        left(&[LeftOut::Unused]),
        "{:#?}",
        run.messages()
    );
    assert_eq!(found.kept, 1);
}

#[test]
fn a_file_at_a_former_path_that_nothing_names_stays_behind_as_at_a_former_path() {
    let env = env();
    let run = repair(&env, vec![], FORMER_TEX, &blob(FORMER_TEX));
    assert!(!run.ships(FORMER_TEX));
    let found = run.only::<InPlace>();
    assert_eq!(
        found.left,
        left(&[LeftOut::FormerPath]),
        "{:#?}",
        run.messages()
    );
}

#[test]
fn the_games_older_bytes_at_the_games_path_stay_behind_as_stale() {
    let env = env();
    let run = repair(&env, vec![], BASE_TEX, OLD_BYTES);
    assert!(!run.ships(BASE_TEX));
    let found = run.only::<InPlace>();
    assert_eq!(found.left, left(&[LeftOut::Stale]), "{:#?}", run.messages());
}

#[test]
fn a_file_only_a_value_a_fixup_removed_named_stays_behind_as_orphaned() {
    let env = env();
    let material = object("M", "Material", vec![("oldTexture", text(OWN_TEX))]);
    let archive = ModArchive::new()
        .file(
            WAD,
            SKIN0_BIN,
            bin(vec![entry(ENTRY, &base_skin()), material]),
        )
        .file(WAD, OWN_TEX, blob(OWN_TEX));
    let set = FixupSet::empty().with(Rule::remove_fields("Material", ["oldTexture"]).id("no-old"));
    let run = PackRun::of(&env, &set, &archive);
    assert!(!run.ships(OWN_TEX));
    let found = run.only::<InPlace>();
    assert_eq!(
        found.left,
        left(&[LeftOut::Orphaned]),
        "{:#?}",
        run.messages()
    );

    // With the value left in, the bin names the file and it ships.
    let kept = PackRun::of(&env, &FixupSet::empty(), &archive);
    assert!(kept.ships(OWN_TEX));
    assert_eq!(kept.only::<InPlace>().left, left(&[]));
}

#[test]
fn a_bin_the_client_loads_and_a_file_it_names_ship() {
    let env = env();
    let run = repair(&env, vec![particle("P", OWN_TEX)], OWN_TEX, &blob(OWN_TEX));
    assert!(matches!(run.chunk(SKIN0_BIN).origin, Origin::Kept(_)));
    assert_eq!(&*run.chunk_bytes(OWN_TEX), blob(OWN_TEX).as_slice());
    let found = run.only::<InPlace>();
    assert_eq!((found.kept, found.left), (2, left(&[])));
}

#[test]
fn a_file_at_a_path_the_game_ships_ships_though_no_bin_of_the_mod_names_it() {
    let env = env();
    // Skin 3's texture: the game's own bins read the path, and the mod's bin does not.
    let run = repair(&env, vec![], S3_TEX, b"the mod's own texture for skin 3");
    assert_eq!(
        &*run.chunk_bytes(S3_TEX),
        b"the mod's own texture for skin 3"
    );
    assert_eq!(run.only::<InPlace>().left, left(&[]));
}

#[test]
fn a_file_a_file_fixup_made_ships_beside_the_file_it_was_made_from() {
    let env = env();
    // The base particle's texture at the `.dds` path of an older client, which the mod's
    // bin names: `load` makes the file at the `.tex` path the game reads.
    let old = BASE_PARTICLE_TEX.replace(".tex", ".dds");
    let texture = dds(DdsPixels::Dxt3, (8, 8), 1);
    let archive = ModArchive::new()
        .file(
            WAD,
            SKIN0_BIN,
            bin(vec![entry(ENTRY, &base_skin()), particle("P", &old)]),
        )
        .file(WAD, &old, texture.clone());
    let run = PackRun::of(&env, &FixupSet::standard(), &archive);
    assert_eq!(&*run.chunk_bytes(BASE_PARTICLE_TEX), texture.as_slice());
    assert!(run.ships(&old));
    let found = run.only::<InPlace>();
    assert_eq!(
        (found.made, found.left),
        (1, left(&[])),
        "{:#?}",
        run.messages()
    );
}

/// A port of the mod's own model at skin0, its entry naming `texture`, the model's texture
/// at that path with these bytes.
fn port<'a>(env: &'a Env, texture: &str, bytes: Vec<u8>) -> PortRun<'a> {
    let skin = Skin {
        texture: texture.into(),
        ..own_skin("a", GRAPH, RESOURCES)
    };
    let mut files = own_files("a");
    files.retain(|(path, _)| !path.ends_with(".tex"));
    files.push((texture.to_string(), bytes));
    files.push(mod_skin(
        "test",
        0,
        ENTRY,
        &skin,
        vec![own_graph("a", GRAPH)],
    ));
    PortRun::of(env, &archive(&files))
}

#[test]
fn a_ports_own_skin_bin_stays_behind_as_not_carried() {
    let env = port_env(skins::game(), NoHistory);
    let run = port(&env, OWN_TEX, blob(OWN_TEX));
    // Its entry goes into the patch of the game's bin, and its graph into a bin made.
    assert!(!run.ships(SKIN0_BIN), "{}", run.dump());
    let found = run.only::<Ported>();
    assert_eq!(found.left, left(&[LeftOut::NotCarried]), "{}", run.dump());
    // Every other file is named by a ported object with the mod's own bytes, and ships.
    assert_eq!(found.files, 5, "{}", run.dump());
}

#[test]
fn a_file_with_the_games_bytes_at_the_games_path_stays_behind_as_the_games() {
    let env = port_env(skins::game(), NoHistory);
    let run = port(&env, BASE_TEX, blob(BASE_TEX));
    // The entry names the game's file where the game has it, and nothing ships there.
    let unit = run.unit("test", 0);
    let verdicts = &run.port.refs.of(unit).expect("the unit's files").refs;
    let at = path_hash(BASE_TEX);
    assert_eq!(verdicts.get(&at), Some(&FileVerdict::Game { path: at }));
    assert!(!run.ships(BASE_TEX), "{}", run.dump());
    assert_eq!(
        run.only::<Ported>().left,
        left(&[LeftOut::NotCarried, LeftOut::Game]),
        "{}",
        run.dump()
    );
}

#[test]
fn a_bank_whose_own_clips_ship_in_a_bank_made_stays_behind_as_recut() {
    let env = port_env(audio_game(), NoHistory);
    // Clip 1 as the game has it, clip 2 replaced, clip 9 new.
    let clips = vec![clip(1, 1), clip(2, 7), clip(9, 9)];
    let media = media_bank("test_base_sfx_audio", 134, clips);
    let skin = bin(vec![base_entry(&[BASE_SFX])]);
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), skin)
        .file(WAD, SFX_AUDIO, media);
    let run = PortRun::of(&env, &archive);
    let found = run.only::<Ported>();
    assert!(found.banks > 0, "{}", run.dump());
    assert_eq!(
        found.left,
        left(&[LeftOut::NotCarried, LeftOut::Recut]),
        "{}",
        run.dump()
    );
    // The bank at the game's path is the mod's, cut: no chunk of the package is at it.
    assert!(!run.ships(SFX_AUDIO), "{}", run.dump());
}
