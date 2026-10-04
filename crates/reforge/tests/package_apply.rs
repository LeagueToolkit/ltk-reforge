//! The check `apply` run by itself on a port's package: the package as built raises nothing,
//! and each finding is shown on that package with one patch or one chunk changed by hand.
//! The game is `reforge_testkit::skins`.

use std::sync::Arc;

use reforge::diag::{Diagnostics, Stage};
use reforge::env::Env;
use reforge::fixups::TargetTypes;
use reforge::history::NoHistory;
use reforge::model::{ChunkPath, Layer, NoNames, name_hash, path_hash};
use reforge::package::{
    Apply, ApplyFailed, Check, CheckCx, Chunk, ChunkKey, Coverage, Done, FileMissing,
    LinkUnresolved, ManifestUnreadable, NameUnresolved, Overlay, Package, Patch, PatchOf,
    PtchBuilder, RecordSkipped,
};
use reforge::port::RefDangling;
use reforge_testkit::bins::{bin, blob, entry, particle, resolver_with, text};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, skin_bin, skin7};
use reforge_testkit::sounds::{BASE_SFX, BASE_VO, VO_WPK, base_entry, clip, package, voice_game};
use reforge_testkit::{MemoryGame, ModArchive, PortRun};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const OWN_PARTICLE: &str = "Characters/Test/Skins/Skin0/Particles/Test_Own";
const OWN_TEX: &str = "assets/mod/a/own.tex";
const SKIN0_BIN: &str = "data/characters/test/skins/skin0.bin";
const DIR: &str = "mods/synthetic.reforgetests/characters/test/skin0";
/// The name the port carries the mod's particle under.
const CARRIED: &str = "Mods/synthetic.reforgetests/Characters/Test/Skins/Skin0/Particles/Test_Own";

/// The mod's own rig at skin0 with a particle of its own, reached through the resolver,
/// which names `file`.
fn own_mod(file: &str) -> ModArchive {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
        resolver_with(RESOURCES0, &[("Test_Q_Cast", OWN_PARTICLE)]),
        particle(OWN_PARTICLE, file),
    ];
    let mut files = own_files("a");
    files.push((OWN_TEX.into(), blob(OWN_TEX)));
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files)
}

fn path(path: &str) -> ChunkPath {
    ChunkPath::parse(path).unwrap_or_else(|error| panic!("{path} is no path: {error}"))
}

/// The package with the chunks `keeps` takes and each patch as `alter` leaves it.
fn rebuilt(
    package: &Package,
    keeps: impl Fn(&ChunkKey) -> bool,
    alter: impl Fn(&mut Patch),
) -> Package {
    let mut out = Package::new(package.config.clone());
    out.tables = package.tables.clone();
    for (layer, key, chunk) in package.chunks().filter(|(_, key, _)| keeps(key)) {
        out.insert(layer.clone(), key.clone(), chunk.clone());
    }
    for (layer, patch) in package.all_patches() {
        let mut patch = patch.clone();
        alter(&mut patch);
        out.patch(layer.clone(), patch);
    }
    out
}

/// The package without the chunk at a path.
fn without(package: &Package, left_out: &str) -> Package {
    let hash = path_hash(left_out);
    rebuilt(package, |key| key.hash() != hash, |_| {})
}

/// What the check `apply` raises on a package of a run's port.
fn applied(env: &Env, run: &PortRun<'_>, package: &Package) -> Diagnostics {
    let overlay = Overlay::apply(package, env.game()).expect("the game reads");
    let types = TargetTypes::new(None, None);
    let cx = CheckCx::new(package, &types).of_port(env.game(), &overlay, &run.port);
    let mut log = Diagnostics::new();
    Apply.run(&cx, &mut log.at(Stage::Check));
    log
}

/// What the check covered.
fn coverage(log: &Diagnostics) -> Coverage {
    let [done] = log.of::<Done>().collect::<Vec<_>>()[..] else {
        panic!("the check says once what it covered");
    };
    done.0
}

/// A patch of the champion's root bin that sets the name of a skin entry: no unit's patch,
/// so the check walks nothing from it.
fn renaming(run: &PortRun<'_>, target: &str, at: &str, entry_name: &str) -> Patch {
    let mut ptch = PtchBuilder::new(&NoNames);
    ptch.set(
        name_hash(entry_name),
        &[name_hash("championSkinName")],
        text("Renamed"),
    )
    .expect("the field has a name");
    let unit = run.unit("test", 0);
    let character = run.facts.fix.fixed.units[unit].character;
    Patch {
        target: path(target),
        path: path(at),
        records: ptch.len(),
        bytes: ptch.bytes().into(),
        links: Vec::new(),
        of: PatchOf::Root { character },
    }
}

#[test]
fn a_ports_package_as_built_raises_nothing() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &own_mod(OWN_TEX));
    let log = applied(&env, &run, run.package());
    assert_eq!(log.len(), 1, "what the check covered and nothing else");
    // The entry, its graph, its resolver and the particle the resolver names.
    assert_eq!(
        coverage(&log),
        Coverage::Apply {
            patches: 1,
            reached: 4,
            names_known: 0,
            files_known: 0,
            names_game: 0,
            files_game: 0,
        }
    );
}

#[test]
fn a_patch_of_a_bin_the_game_lacks_fails_to_apply() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &own_mod(OWN_TEX));
    let mut package = run.package().clone();
    let target = skin_bin("test", 5);
    let lacking = renaming(&run, &target, "overrides/test-skin5.ptch", SKIN0);
    package.patch(Layer::Base, lacking);
    let log = applied(&env, &run, &package);
    assert_eq!(
        log.of::<ApplyFailed>().collect::<Vec<_>>(),
        [&ApplyFailed {
            layer: "base".into(),
            target: path(&target),
            reason: None,
        }]
    );
    // The unit's own patch applies all the same.
    assert!(matches!(coverage(&log), Coverage::Apply { patches: 1, .. }));
}

/// The game with its skin 7 bin as `PROP` version 1 writes it, which the overlay does not
/// take.
fn game_with_an_old_bin() -> MemoryGame {
    let whole = bin(vec![entry("Characters/Test/Skins/Skin7", &skin7())]);
    let mut old = b"PROP".to_vec();
    old.extend(1u32.to_le_bytes());
    // Version 3 holds the count of its links here, and version 1 holds no such list.
    old.extend(&whole[12..]);
    let mut game = skins::game();
    game.insert(&skin_bin("test", 7), old);
    game
}

#[test]
fn a_patch_the_overlay_refuses_fails_with_the_overlays_reason() {
    let env = env(game_with_an_old_bin(), NoHistory);
    let run = PortRun::of(&env, &own_mod(OWN_TEX));
    let mut package = run.package().clone();
    let target = skin_bin("test", 7);
    let entry_name = "Characters/Test/Skins/Skin7";
    let refused = renaming(&run, &target, "overrides/test-skin7.ptch", entry_name);
    package.patch(Layer::Base, refused);
    let log = applied(&env, &run, &package);
    let [failed] = log.of::<ApplyFailed>().collect::<Vec<_>>()[..] else {
        panic!("one patch does not apply");
    };
    assert_eq!(
        (failed.layer.as_str(), failed.target.as_str()),
        ("base", target.as_str())
    );
    assert!(failed.reason.as_ref().is_some_and(|why| !why.is_empty()));
}

#[test]
fn a_record_the_overlay_skips_is_raised_with_the_overlays_words() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &own_mod(OWN_TEX));
    let mut package = run.package().clone();
    // A record on an entry the patched bin does not hold.
    let lost = "Characters/Test/Skins/Skin99";
    let skipping = renaming(&run, SKIN0_BIN, "overrides/test-lost.ptch", lost);
    package.patch(Layer::Base, skipping);
    let log = applied(&env, &run, &package);
    let [skipped] = log.of::<RecordSkipped>().collect::<Vec<_>>()[..] else {
        panic!("the records of one patch are skipped");
    };
    assert_eq!(
        (skipped.layer.as_str(), skipped.target.as_str()),
        ("base", SKIN0_BIN)
    );
    let [record] = &skipped.records[..] else {
        panic!("one record is skipped: {:?}", skipped.records);
    };
    assert!(record.starts_with("OverrideRecordSkipped"), "{record}");
    assert_eq!(log.of::<ApplyFailed>().count(), 0);
}

#[test]
fn a_link_a_patch_adds_that_leads_to_no_bin_is_unresolved() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &own_mod(OWN_TEX));
    let missing = "mods/synthetic.reforgetests/characters/test/skin0/missing.bin";
    let package = rebuilt(
        run.package(),
        |_| true,
        |patch| patch.links.push(path(missing)),
    );
    let log = applied(&env, &run, &package);
    assert_eq!(
        log.of::<LinkUnresolved>().collect::<Vec<_>>(),
        [&LinkUnresolved {
            target: path(SKIN0_BIN),
            links: vec![missing.to_string()],
        }]
    );
    assert_eq!(log.of::<NameUnresolved>().count(), 0);
}

#[test]
fn a_name_the_patched_entry_reaches_that_no_loaded_bin_holds_is_unresolved() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &own_mod(OWN_TEX));
    // The unit's bin of carried objects, with no object in it: the resolver names the
    // particle, and nothing the patched bin loads holds it.
    let vfx = format!("{DIR}/vfx.bin");
    let mut package = run.package().clone();
    let emptied = Chunk {
        bytes: Arc::from(bin(vec![])),
        origin: run.chunk(&vfx).origin.clone(),
    };
    let key = package.chunks().map(|(_, key, _)| key.clone());
    let key = key.into_iter().find(|key| key.path.as_str() == vfx);
    package.insert(Layer::Base, key.expect("the unit's bin"), emptied);
    let log = applied(&env, &run, &package);
    assert_eq!(
        log.of::<NameUnresolved>().collect::<Vec<_>>(),
        [&NameUnresolved {
            unit: run.unit("test", 0),
            names: vec![name_hash(CARRIED)],
        }]
    );
    assert_eq!(log.of::<LinkUnresolved>().count(), 0);
    assert_eq!(log.of::<FileMissing>().count(), 0);
}

#[test]
fn a_file_a_carried_object_names_that_is_neither_shipped_nor_the_games_is_missing() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &own_mod(OWN_TEX));
    let texture = format!("{DIR}/vfx/own/own.tex");
    assert!(run.ships(&texture), "{}", run.dump());
    let log = applied(&env, &run, &without(run.package(), &texture));
    assert_eq!(
        log.of::<FileMissing>().collect::<Vec<_>>(),
        [&FileMissing {
            unit: run.unit("test", 0),
            files: vec![path_hash(&texture)],
        }]
    );
    assert_eq!(log.of::<NameUnresolved>().count(), 0);
}

#[test]
fn a_manifest_that_does_not_load_is_raised_and_none_of_its_layers_patches_is_applied() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &own_mod(OWN_TEX));
    // The overlay takes an override file by its `.ptch` extension.
    let package = rebuilt(
        run.package(),
        |_| true,
        |patch| patch.path = path("overrides/test-skin0.bin"),
    );
    let log = applied(&env, &run, &package);
    let [unreadable] = log.of::<ManifestUnreadable>().collect::<Vec<_>>()[..] else {
        panic!("one manifest does not load");
    };
    assert_eq!(unreadable.layer, "base");
    assert!(!unreadable.reason.is_empty());
    assert!(matches!(coverage(&log), Coverage::Apply { patches: 0, .. }));
}

/// The files and names a port left unresolved in the mod already, as the check counts them.
fn known(log: &Diagnostics) -> (usize, usize) {
    match coverage(log) {
        Coverage::Apply {
            names_known,
            files_known,
            ..
        } => (names_known, files_known),
        other => panic!("the apply check covered {other:?}"),
    }
}

#[test]
fn a_loose_end_of_the_mods_own_is_counted_apart_and_not_raised() {
    let env = env(skins::game(), NoHistory);
    // The particle names a file the mod does not ship and the game does not hold.
    let run = PortRun::of(&env, &own_mod("assets/mod/a/lost.tex"));
    assert_eq!(run.found::<RefDangling>().len(), 1, "{}", run.dump());
    let log = applied(&env, &run, run.package());
    assert_eq!(known(&log), (0, 1));
    assert_eq!(log.of::<FileMissing>().count(), 0);
}

/// #GAME-BEHAVIOR-0009: a `.troy` path with no file there is no loose end where the game
/// holds the `.troybin` beside it.
#[test]
fn a_troy_path_is_there_when_the_game_holds_the_troybin_beside_it() {
    let troy = "data/particles/test_glacial_idle.troy";
    let troybin = "data/particles/test_glacial_idle.troybin";
    let port = |game: MemoryGame| {
        let env = env(game, NoHistory);
        let run = PortRun::of(&env, &own_mod(troy));
        let log = applied(&env, &run, run.package());
        assert_eq!(log.of::<FileMissing>().count(), 0, "{}", run.dump());
        // The plan finds no file at the path in either game.
        assert_eq!(run.found::<RefDangling>().len(), 1, "{}", run.dump());
        known(&log)
    };
    assert_eq!(port(skins::game()), (0, 1));
    let mut game = skins::game();
    game.insert(troybin, blob(troybin));
    assert_eq!(port(game), (0, 0));
}

#[test]
fn a_bank_a_voice_layers_block_names_that_ships_in_no_layer_is_missing() {
    let env = env(voice_game(), NoHistory);
    // The base's model and audio block, its voice package with clip 3 replaced.
    let skin = bin(vec![base_entry(&[BASE_SFX, BASE_VO])]);
    let voice = package(vec![clip(3, 7), clip(4, 4)]);
    let archive = ModArchive::new()
        .file(skins::WAD, &skin_bin("test", 0), skin)
        .file(skins::WAD, VO_WPK, voice);
    let run = PortRun::of(&env, &archive);
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
    let made = format!("{DIR}/audio/vo_audio.wpk");
    assert!(run.ships(&made), "{}", run.dump());

    let log = applied(&env, &run, &without(run.package(), &made));
    assert_eq!(
        log.of::<FileMissing>().collect::<Vec<_>>(),
        [&FileMissing {
            unit: run.unit("test", 0),
            files: vec![path_hash(&made)],
        }]
    );
}
