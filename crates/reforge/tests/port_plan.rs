//! The plan of a port over synthetic mods: which units ship, what each entry reaches, where
//! each file is found, the names moved to the target and the names and places under the
//! namespace. The game is `reforge_testkit::skins`.

use reforge::history::NoHistory;
use reforge::model::entity::ObjectId;
use reforge::model::{Slot, name_hash, path_hash, value};
use reforge::port::{
    Carried, Fate, FileVerdict, GameCarried, Left, LinkDangling, Policy, PortOptions, RefDangling,
    RefUnmounted, Retargeted, UnitsLeft, Unreached,
};
use reforge_testkit::bins::{Skin, bin, blob, entry, graph, particle, resolver, with_link};
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::skins::{
    self, BASE_PARTICLE, BASE_PARTICLE_TEX, BASE_TEX, S3_TEX, WAD, base_skin, skin_bin,
};
use reforge_testkit::{ModArchive, PortRun};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const OWN_PARTICLE: &str = "Characters/Test/Skins/Skin0/Particles/Test_Own";
const OWN_TEX: &str = "assets/mod/a/own.tex";
/// The namespace of the synthetic archive: its name and its author.
const NS: &str = "synthetic.reforgetests";

/// The mod's own rig at skin0, its entry with these links, and these objects beside it.
fn own_mod(links: &[(&str, &str)], extra: Vec<ltk_meta::BinObject>) -> Vec<(String, Vec<u8>)> {
    let mut e = entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0));
    for (field, target) in links {
        e = with_link(e, field, target);
    }
    let mut objects = vec![e, own_graph("a", GRAPH0), resolver(RESOURCES0)];
    objects.extend(extra);
    let mut files = own_files("a");
    files.push((skin_bin("test", 0), bin(objects)));
    files
}

fn fate(run: &PortRun<'_>, slot: u16, name: &str) -> Option<Fate> {
    let reach = run.port.reach.of(run.unit("test", slot)).expect("reached");
    let found = reach.objects.iter().find(|(object, _)| {
        let m = &run.facts.fix.fixed;
        m.objects[*object].entry == name_hash(name)
    });
    found.map(|(_, fate)| *fate)
}

fn verdict(run: &PortRun<'_>, character: &str, slot: u16, path: &str) -> Option<FileVerdict> {
    let refs = run.port.refs.of(run.unit(character, slot)).expect("refs");
    refs.refs.get(&path_hash(path)).cloned()
}

#[test]
fn reach_sorts_what_the_entry_reaches() {
    // The entry links the base particle, shipped byte for byte as the game has it, a
    // particle of the mod's own, and a name nothing holds. One more object is linked by
    // nothing.
    let gone = "Characters/Test/Skins/Skin0/Particles/Gone";
    let links = [
        ("fxBase", BASE_PARTICLE),
        ("fxOwn", OWN_PARTICLE),
        ("fxGone", gone),
    ];
    let leftover = "Characters/Test/Skins/Skin0/Particles/Leftover";
    let mut files = own_mod(
        &links,
        vec![
            particle(BASE_PARTICLE, BASE_PARTICLE_TEX),
            particle(OWN_PARTICLE, OWN_TEX),
            particle(leftover, "assets/mod/a/leftover.tex"),
        ],
    );
    files.push((OWN_TEX.into(), blob(OWN_TEX)));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    assert_eq!(fate(&run, 0, SKIN0), Some(Fate::Entry), "{}", run.dump());
    assert_eq!(fate(&run, 0, RESOURCES0), Some(Fate::Resolver));
    assert_eq!(fate(&run, 0, GRAPH0), Some(Fate::Graph));
    assert_eq!(fate(&run, 0, BASE_PARTICLE), Some(Fate::Game));
    assert_eq!(fate(&run, 0, OWN_PARTICLE), Some(Fate::Carried));
    assert_eq!(fate(&run, 0, leftover), None);
    let unit = run.unit("test", 0);
    let reach = run.port.reach.of(unit).expect("reached");
    assert_eq!(reach.target, Slot(0));
    assert!(reach.dangling.contains(&name_hash(gone)));
    assert_eq!(reach.leftovers.len(), 1);
    assert_eq!(run.only::<LinkDangling>().names, [name_hash(gone)]);
    assert_eq!(run.only::<Unreached>().objects, 1);

    assert!(matches!(
        verdict(&run, "test", 0, OWN_TEX),
        Some(FileVerdict::Own { .. })
    ));
    assert!(matches!(
        verdict(&run, "test", 0, "assets/mod/a/mesh.skn"),
        Some(FileVerdict::Own { .. })
    ));
    // The base particle is the game's own object: its files are not the port's.
    assert_eq!(verdict(&run, "test", 0, BASE_PARTICLE_TEX), None);
    // The leftover is not carried, so neither is what it names.
    assert_eq!(verdict(&run, "test", 0, "assets/mod/a/leftover.tex"), None);
}

#[test]
fn a_file_is_the_games_where_the_game_holds_its_bytes_and_ships_otherwise() {
    // Particles naming a game file the mod does not ship, a game file the mod ships
    // unchanged, the game's bytes under a path of the mod's, and a file nothing holds.
    let links = [
        ("a", "P/GameOnly"),
        ("b", "P/GameSame"),
        ("c", "P/Copied"),
        ("d", "P/Nowhere"),
    ];
    let mut files = own_mod(
        &links,
        vec![
            particle("P/GameOnly", BASE_TEX),
            particle("P/GameSame", S3_TEX),
            particle("P/Copied", "assets/mod/a/copied.tex"),
            particle("P/Nowhere", "assets/mod/a/nowhere.tex"),
        ],
    );
    files.push((S3_TEX.into(), blob(S3_TEX)));
    files.push(("assets/mod/a/copied.tex".into(), blob(S3_TEX)));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let game = |path: &str| {
        Some(FileVerdict::Game {
            path: path_hash(path),
        })
    };
    assert_eq!(
        verdict(&run, "test", 0, BASE_TEX),
        game(BASE_TEX),
        "{}",
        run.dump()
    );
    assert_eq!(verdict(&run, "test", 0, S3_TEX), game(S3_TEX));
    assert_eq!(
        verdict(&run, "test", 0, "assets/mod/a/copied.tex"),
        game(S3_TEX)
    );
    assert_eq!(
        verdict(&run, "test", 0, "assets/mod/a/nowhere.tex"),
        Some(FileVerdict::Dangling)
    );
    assert_eq!(
        run.only::<RefDangling>().files,
        [path_hash("assets/mod/a/nowhere.tex")]
    );
    // The copy is not shipped: the particle names the game's file.
    assert!(!run.port.files.keys().any(|file| {
        run.facts.fix.fixed.files[*file].hash == path_hash("assets/mod/a/copied.tex")
    }));
    let copied = run.port.object(run.object("P/Copied")).expect("carried");
    let named: Vec<_> = value::file_refs(copied)
        .iter()
        .map(value::FileRef::hash)
        .collect();
    assert_eq!(named, [path_hash(S3_TEX)]);
    assert_eq!(run.only::<Carried>().repointed, 1);
}

/// A game where another champion's WAD holds a texture, with or without the champion's own.
fn verdict_of_borrowed(wads: &[&str]) -> Option<FileVerdict> {
    let borrowed = "assets/characters/other/skins/base/other.tex";
    let mut game = skins::game();
    game.insert_in(borrowed, blob(borrowed), wads);
    let env = env(game, NoHistory);
    let files = own_mod(
        &[("fx", "P/Borrowed")],
        vec![particle("P/Borrowed", borrowed)],
    );
    let run = PortRun::of(&env, &archive(&files));
    let found = verdict(&run, "test", 0, borrowed);
    let unmounted = matches!(found, Some(FileVerdict::Unmounted { .. }));
    assert_eq!(run.found::<RefUnmounted>().len(), usize::from(unmounted));
    found
}

#[test]
fn a_file_another_champions_wad_holds_is_the_games_when_the_champions_holds_it_too() {
    let borrowed = path_hash("assets/characters/other/skins/base/other.tex");
    assert_eq!(
        verdict_of_borrowed(&["Other.wad.client", WAD]),
        Some(FileVerdict::Game { path: borrowed })
    );
    assert_eq!(
        verdict_of_borrowed(&["Global.wad.client"]),
        Some(FileVerdict::Game { path: borrowed })
    );
}

#[test]
fn a_file_only_another_champions_wad_holds_is_unmounted() {
    let found = verdict_of_borrowed(&["Other.wad.client"]);
    let Some(FileVerdict::Unmounted { wad }) = found else {
        panic!("not unmounted: {found:?}");
    };
    assert_eq!(wad.as_str(), "Other.wad.client");
}

#[test]
fn a_sub_character_reaches_its_champions_objects() {
    // The pet's entry links a particle only the champion's skin bin holds, loaded beside it.
    let shared = "Characters/Test/Skins/Skin0/Particles/Shared";
    let champion = with_link(entry(SKIN0, &base_skin()), "fx", shared);
    let pet = with_link(
        entry("Characters/TestPet/Skins/Skin0", &skins::pet_skin()),
        "fx",
        shared,
    );
    let files = [
        (
            skin_bin("test", 0),
            bin(vec![champion, particle(shared, "assets/mod/a/shared.tex")]),
        ),
        (skin_bin("testpet", 0), bin(vec![pet])),
        ("assets/mod/a/shared.tex".to_string(), blob("shared")),
    ];
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let pet = run
        .port
        .reach
        .of(run.unit("testpet", 0))
        .expect("the pet is reached");
    assert!(pet.champion.contains(&name_hash(shared)), "{}", run.dump());
    assert!(!pet.dangling.contains(&name_hash(shared)));
    assert_eq!(fate(&run, 0, shared), Some(Fate::Carried));
    // The pet's link follows the name the champion's unit gives the particle.
    let carried = format!("Mods/{NS}/Characters/Test/Skins/Skin0/Particles/Shared");
    let pet_entry = run.object("Characters/TestPet/Skins/Skin0");
    let pet_entry = run
        .port
        .object(pet_entry)
        .expect("the pet's entry is ported");
    assert!(value::links(pet_entry).contains(&name_hash(&carried)));
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
}

#[test]
fn a_copy_leaves_the_port() {
    // The same skin bin at slots 0 and 7.
    let mut files = own_mod(&[], vec![]);
    let bin0 = files.last().expect("the skin bin").1.clone();
    files.push((skin_bin("test", 7), bin0));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let (kept, copy) = (run.unit("test", 0), run.unit("test", 7));
    assert_eq!(run.port.shipping.ships, [kept], "{}", run.dump());
    let left = run.port.shipping.left.get(&copy).expect("the copy leaves");
    assert_eq!(left.why, Left::Copy { of: kept });
    // The unit holds the diagnostic that says why.
    let finding = run.only::<UnitsLeft>();
    assert_eq!(finding.units.len(), 1);
    let raised = run.log.get(left.diagnostic).expect("the diagnostic");
    assert_eq!(raised.downcast_ref::<UnitsLeft>(), Some(finding));
    assert_eq!(run.port.units.len(), 1);
}

#[test]
fn the_games_own_skin_beside_the_mods_leaves_the_port() {
    // The mod's rig at skin0, and the game's own skin 7 bin as the game ships it.
    let mut files = own_mod(&[], vec![]);
    files.push((
        skin_bin("test", 7),
        bin(vec![
            entry("Characters/Test/Skins/Skin7", &skins::skin7()),
            resolver("Characters/Test/Skins/Skin7/Resources"),
        ]),
    ));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let shipped = run.unit("test", 0);
    assert_eq!(run.port.shipping.ships, [shipped], "{}", run.dump());
    let left = &run.port.shipping.left[&run.unit("test", 7)];
    assert_eq!(left.why, Left::GameSkin);
}

#[test]
fn a_chroma_that_is_its_parents_work_folds_into_it() {
    // The mod's rig at skin 3, and at skin 9, a chroma of 3, the same rig on the game's
    // texture.
    let skin = |slot: u16, texture: Option<&str>| {
        let own = own_skin(
            "a",
            &format!("Characters/Test/Animations/Skin{slot}"),
            &format!("Characters/Test/Skins/Skin{slot}/Resources"),
        );
        let skin = Skin {
            texture: texture.map_or(own.texture.clone(), str::to_string),
            ..own
        };
        mod_skin(
            "test",
            slot,
            &format!("Characters/Test/Skins/Skin{slot}"),
            &skin,
            vec![own_graph(
                "a",
                &format!("Characters/Test/Animations/Skin{slot}"),
            )],
        )
    };
    let mut files = own_files("a");
    files.push(skin(3, None));
    files.push(skin(9, Some(S3_TEX)));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let (parent, chroma) = (run.unit("test", 3), run.unit("test", 9));
    assert_eq!(run.port.shipping.ships, [parent], "{}", run.dump());
    let left = &run.port.shipping.left[&chroma];
    assert_eq!(left.why, Left::ChromaFolded { into: parent });
}

#[test]
fn a_unit_that_goes_to_another_slot_takes_its_names() {
    // A paste of skin 3 at skin0 with a particle of its own: it fits only skin 3.
    let pasted = with_link(entry(SKIN0, &skins::skin3()), "fx", OWN_PARTICLE);
    let files = [
        (
            skin_bin("test", 0),
            bin(vec![pasted, particle(OWN_PARTICLE, OWN_TEX)]),
        ),
        (OWN_TEX.to_string(), blob(OWN_TEX)),
    ];
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let unit = run.unit("test", 0);
    let retargeted = run.only::<Retargeted>();
    assert_eq!(
        (retargeted.from, retargeted.to),
        (Slot(0), Slot(3)),
        "{}",
        run.dump()
    );
    let port = run.port.unit(unit).expect("the unit ships");
    assert_eq!(port.target, Slot(3));
    assert_eq!(port.dir, format!("mods/{NS}/characters/test/skin3"));
    // The entry takes the target's name. The mod's objects stay as they were.
    let ported = run.port.object(port.entry).expect("the entry is ported");
    assert_eq!(ported.path_hash, name_hash("Characters/Test/Skins/Skin3"));
    let m = &run.facts.fix.fixed;
    assert_eq!(m.objects[port.entry].value.path_hash, name_hash(SKIN0));
    // The carried particle is named and placed under the target's directory.
    let carried: Vec<ObjectId> = port.carried.clone();
    assert_eq!(carried, [run.object(OWN_PARTICLE)]);
    let particle = run.port.object(carried[0]).expect("carried");
    let name = format!("Mods/{NS}/Characters/Test/Skins/Skin3/Particles/Test_Own");
    assert_eq!(particle.path_hash, name_hash(&name));
    // The name is kept for the package's hash table, the character as the run spells it.
    let made = &run.port.layout.made.entries;
    assert!(made.iter().any(|made| made.eq_ignore_ascii_case(&name)));
    assert!(value::links(ported).contains(&name_hash(&name)));
    let place = format!("mods/{NS}/characters/test/skin3/vfx/own/own.tex");
    let places: Vec<&str> = run
        .port
        .files
        .values()
        .map(|place| place.as_str())
        .collect();
    assert_eq!(places, [place.as_str()]);
    let named: Vec<_> = value::file_refs(particle)
        .iter()
        .map(value::FileRef::hash)
        .collect();
    assert_eq!(named, [path_hash(&place)]);
}

#[test]
fn a_game_object_naming_an_overridden_file_is_carried() {
    // The game's base particle, unchanged, beside the mod's own bytes at the path of the
    // texture it names.
    let mut files = own_mod(
        &[("fx", BASE_PARTICLE)],
        vec![particle(BASE_PARTICLE, BASE_PARTICLE_TEX)],
    );
    files.push((BASE_PARTICLE_TEX.into(), blob("the mod's own idle texture")));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    assert_eq!(
        fate(&run, 0, BASE_PARTICLE),
        Some(Fate::Carried),
        "{}",
        run.dump()
    );
    assert_eq!(run.only::<GameCarried>().objects, 1);
    assert!(matches!(
        verdict(&run, "test", 0, BASE_PARTICLE_TEX),
        Some(FileVerdict::Own { .. })
    ));
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
}

#[test]
fn the_games_own_skin_alone_stays_a_unit() {
    // The game's own skin 7 bin alone: nothing of the mod's.
    let files = [(
        skin_bin("test", 7),
        bin(vec![
            entry("Characters/Test/Skins/Skin7", &skins::skin7()),
            graph("Characters/Test/Animations/Skin0", skins::BASE_CLIPS),
        ]),
    )];
    let env = env(skins::game(), NoHistory);
    let archive: ModArchive = archive(&files);
    let run = PortRun::under(&env, &archive, Policy::default(), PortOptions::default());
    // One unit alone stays a unit, and its port holds no record and no file.
    assert_eq!(run.port.shipping.ships.len(), 1, "{}", run.dump());
    assert!(run.port.files.is_empty());
}
