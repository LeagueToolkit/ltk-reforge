//! The plan of a port over synthetic mods: which units ship, what each entry reaches, where
//! each file is found, the names moved to the target and the names and places under the
//! namespace. The game is `reforge_testkit::skins`.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyValueEnum};
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::entity::ObjectId;
use reforge::model::{Slot, WadHash, name_hash, path_hash, value};
use reforge::port::{
    Carried, Fate, FileVerdict, GameCarried, Left, LinkDangling, PlanError, Policy, PortOptions,
    RefDangling, RefUnmounted, Retargeted, UnitsLeft, Unreached, WorkKind, WorkLeft,
};
use reforge_testkit::bins::{
    Skin, bin, blob, entry, graph, particle, resolver, resolver_with, text, with_link,
};
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::packs::{DdsPixels, dds};
use reforge_testkit::plans::{
    champion_and_unbound_pet, old_model_and_pet, paste_and_own_skin3, reworked,
};
use reforge_testkit::skins::{
    self, BASE_PARTICLE, BASE_PARTICLE_TEX, BASE_TEX, S3_TEX, WAD, base_skin, skin_bin,
};
use reforge_testkit::{
    BrokenGame, BrokenHistory, FactRun, MemoryHistory, ModArchive, PlanRun, PortRun, rigs,
};

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
    // A name nothing holds is held at no other skin either.
    assert!(run.only::<LinkDangling>().elsewhere.is_empty());
}

#[test]
fn a_dangling_name_the_bins_of_another_skin_hold_is_named_with_that_skin() {
    // The mod's rig at skin0 links the graph only the game's skin 3 bins hold.
    let graph3 = "Characters/Test/Animations/Skin3";
    let files = own_mod(&[("fx", graph3)], vec![]);
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let dangling = run.only::<LinkDangling>();
    assert_eq!(dangling.names, [name_hash(graph3)], "{}", run.dump());
    let held = &dangling.elsewhere[0];
    assert_eq!(
        (held.name, held.slots.as_slice()),
        (name_hash(graph3), &[Slot(3)][..])
    );
    // A copy brings the graph and names its clips; nothing it links is missing.
    let brought = dangling.brought;
    let counts = (brought.objects, brought.files, brought.unresolved);
    assert_eq!(counts, (1, skins::S3_CLIPS.len(), 0));
}

#[test]
fn an_entry_linking_a_resolver_nothing_loads_reaches_the_resolver_under_the_slots_name() {
    // #ADR-0056: the entry links the resolver of the skin it was pasted from.
    let pasted = "Characters/Test/Skins/Skin9/Resources";
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, pasted)),
        own_graph("a", GRAPH0),
        resolver_with(RESOURCES0, &[("Test_Q_Cast", OWN_PARTICLE)]),
        particle(OWN_PARTICLE, OWN_TEX),
    ];
    let mut files = own_files("a");
    files.push((OWN_TEX.into(), blob(OWN_TEX)));
    files.push((skin_bin("test", 0), bin(objects)));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    assert_eq!(
        fate(&run, 0, RESOURCES0),
        Some(Fate::Resolver),
        "{}",
        run.dump()
    );
    assert_eq!(fate(&run, 0, OWN_PARTICLE), Some(Fate::Carried));
    let reach = run.port.reach.of(run.unit("test", 0)).expect("reached");
    assert!(reach.dangling.is_empty() && reach.leftovers.is_empty());
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

#[test]
fn a_stale_copy_is_named_at_the_games_path_unless_the_options_keep_it() {
    // #MOD-ARTIFACT-0004: at the base texture's path, the bytes the game shipped there at
    // the build before.
    let old: &[u8] = b"the base texture as the game shipped it at 16.18";
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"])
        .with_content(BASE_TEX, old, 0..=0)
        .with_path(BASE_TEX, 0..=1);
    let env = env(skins::game(), history);
    let mut files = own_mod(&[("fx", "P/Stale")], vec![particle("P/Stale", BASE_TEX)]);
    files.push((BASE_TEX.into(), old.to_vec()));
    let plan = |options: PortOptions| {
        let facts = FactRun::of(&env, &archive(&files));
        let targets = Policy::default().decide(&facts.facts);
        PlanRun::decided(&env, facts, targets, options)
    };
    let verdict = |run: &PlanRun<'_>| {
        let refs = run.port().refs.of(run.unit("test", 0)).expect("refs");
        refs.refs.get(&path_hash(BASE_TEX)).cloned()
    };

    let run = plan(PortOptions::default());
    let path = path_hash(BASE_TEX);
    assert_eq!(
        verdict(&run),
        Some(FileVerdict::Game { path }),
        "{}",
        run.dump()
    );
    let kept = PortOptions {
        keep_stale: true,
        ..PortOptions::default()
    };
    let run = plan(kept);
    assert!(
        matches!(verdict(&run), Some(FileVerdict::Own { .. })),
        "{}",
        run.dump()
    );
}

/// A game where another champion's WAD holds a texture, with or without the champion's own
/// (#GAME-BEHAVIOR-0007).
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
    // #GAME-BEHAVIOR-0006: the pet's entry links a particle only the champion's skin bin
    // holds, loaded beside it.
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
fn a_repath_tools_tag_is_dropped_from_the_name_a_file_ships_under() {
    // #MOD-ARTIFACT-0026: the tag says the skin folder a tool took the file from.
    let tagged = "assets/mod/a/glow.skins_test_skin3.tex";
    let mut files = own_mod(
        &[("fx", OWN_PARTICLE)],
        vec![particle(OWN_PARTICLE, tagged)],
    );
    files.push((tagged.into(), blob(tagged)));
    let env = env(skins::game(), NoHistory);
    let run = PlanRun::of(&env, &archive(&files));

    let m = &run.facts.fix.fixed;
    let mut shipped = run.port().files.iter();
    let place = shipped.find(|(file, _)| m.files[**file].hash == path_hash(tagged));
    let place = place.map(|(_, place)| place.as_str());
    let expected = format!("mods/{NS}/characters/test/skin0/vfx/own/glow.tex");
    assert_eq!(place, Some(expected.as_str()), "{}", run.dump());
}

#[test]
fn a_file_no_table_names_ships_under_a_name_that_is_not_its_hash() {
    // #MOD-ARTIFACT-0007: a manager reads sixteen hex digits as the chunk's hash.
    let nameless = WadHash(0x0123_4567_89ab_cdef);
    let fx = BinObject::builder(
        name_hash(OWN_PARTICLE),
        name_hash("VfxSystemDefinitionData"),
    )
    .property(name_hash("particleName"), text(OWN_PARTICLE))
    .property(
        name_hash("texture"),
        PropertyValueEnum::WadChunkLink(values::WadChunkLink::new(nameless)),
    )
    .build();
    let mut files = own_mod(&[("fx", OWN_PARTICLE)], vec![fx]);
    files.push((
        format!("{:016x}.tex", nameless.0),
        b"TEX\0nameless".to_vec(),
    ));
    let env = env(skins::game(), NoHistory);
    let run = PlanRun::of(&env, &archive(&files));

    let m = &run.facts.fix.fixed;
    let mut shipped = run.port().files.iter();
    let place = shipped.find(|(file, _)| m.files[**file].hash == nameless);
    let place = place.map(|(_, place)| place.as_str());
    let expected = format!("mods/{NS}/characters/test/skin0/vfx/own/unnamed-0123456789abcdef.tex");
    assert_eq!(place, Some(expected.as_str()), "{}", run.dump());
}

#[test]
fn a_file_named_by_sixteen_hex_digits_in_a_folder_ships_under_another_name() {
    let hexed = "assets/mod/a/0123456789abcdef.tex";
    let mut files = own_mod(&[("fx", OWN_PARTICLE)], vec![particle(OWN_PARTICLE, hexed)]);
    files.push((hexed.into(), blob(hexed)));
    let env = env(skins::game(), NoHistory);
    let run = PlanRun::of(&env, &archive(&files));

    let m = &run.facts.fix.fixed;
    let mut shipped = run.port().files.iter();
    let place = shipped.find(|(file, _)| m.files[**file].hash == path_hash(hexed));
    let place = place.map(|(_, place)| place.as_str());
    let expected = format!("mods/{NS}/characters/test/skin0/vfx/own/0123456789abcdef-2.tex");
    assert_eq!(place, Some(expected.as_str()), "{}", run.dump());
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
fn a_unit_whose_slot_another_unit_took_leaves_superseded() {
    let env = env(skins::game(), NoHistory);
    let run = PlanRun::of(&env, &archive(&paste_and_own_skin3()));
    let written = run.unit("test", 3);
    assert_eq!(run.port().shipping.ships, [written], "{}", run.dump());
    let by = Some(written);
    assert_eq!(run.left("test", 0), Some(Left::Superseded { by }));
    assert_eq!(run.only::<UnitsLeft>().units.len(), 1);
}

#[test]
fn a_unit_made_before_the_rework_leaves_with_its_sub_character() {
    let (history, table) = reworked();
    let env = env(skins::game(), history);
    let facts = FactRun::with_reworks(&env, &archive(&old_model_and_pet()), &table);
    let targets = Policy::default().decide(&facts.facts);
    let run = PlanRun::decided(&env, facts, targets, PortOptions::default());
    assert!(run.port().shipping.ships.is_empty(), "{}", run.dump());
    assert_eq!(run.left("test", 0), Some(Left::PreRework));
    assert_eq!(run.left("testpet", 0), Some(Left::PreRework));
    // One finding per character.
    assert_eq!(run.found::<UnitsLeft>().len(), 2);
    assert!(run.port().units.is_empty());
}

#[test]
fn a_sub_character_left_to_the_games_base_leaves_the_port() {
    let env = env(skins::game(), NoHistory);
    let run = PlanRun::of(&env, &archive(&champion_and_unbound_pet()));
    let champion = run.unit("test", 0);
    assert_eq!(run.port().shipping.ships, [champion], "{}", run.dump());
    assert_eq!(run.left("testpet", 0), Some(Left::SubLeft));
}

#[test]
fn a_sub_character_shipped_as_the_game_has_it_leaves_as_equal_to_the_games() {
    // The mod's rig at skin0, and the sub-character's skin bin byte for byte the game's.
    let mut files = own_mod(&[], vec![]);
    files.push((skin_bin("testpet", 0), skins::pet_bin()));
    let env = env(skins::game(), NoHistory);
    let run = PlanRun::of(&env, &archive(&files));
    let champion = run.unit("test", 0);
    assert_eq!(run.port().shipping.ships, [champion], "{}", run.dump());
    assert_eq!(run.left("testpet", 0), Some(Left::EqualToGame));
    // It is no copy and was never weighed.
    let pet = run.unit("testpet", 0);
    assert!(!run.targets.decisions.contains_key(&pet));
}

#[test]
fn a_unit_the_targets_hold_no_decision_for_leaves_with_no_target() {
    let env = env(skins::game(), NoHistory);
    let facts = FactRun::of(&env, &archive(&own_mod(&[], vec![])));
    // Every policy decides for every representative unit: targets that lack one are a
    // caller's own.
    let mut targets = Policy::default().decide(&facts.facts);
    let unit = facts.unit("test", 0);
    assert!(targets.decisions.remove(&unit).is_some());
    let run = PlanRun::decided(&env, facts, targets, PortOptions::default());
    assert!(run.port().shipping.ships.is_empty(), "{}", run.dump());
    assert_eq!(run.left("test", 0), Some(Left::NoTarget));
    let left = run.only::<UnitsLeft>();
    assert_eq!(left.units[0].why, Left::NoTarget);
}

/// An environment of the synthetic game in which the chunk at `path` does not read.
fn env_broken_at(path: &str) -> Env {
    let game = BrokenGame::at(skins::game(), path);
    let sizes = game.sizes();
    let env = Env::builder().game(game).sizes(sizes);
    env.build().expect("a game was given")
}

#[test]
fn a_game_bin_that_does_not_read_ends_the_plan_with_a_fault() {
    let good = env(skins::game(), NoHistory);
    let facts = FactRun::of(&good, &archive(&own_mod(&[], vec![])));
    let targets = Policy::default().decide(&facts.facts);
    // The plan compares what the entry reaches with the game's skin bin at the target.
    let broken = env_broken_at(&skin_bin("test", 0));
    let run = PlanRun::decided(&broken, facts, targets, PortOptions::default());
    assert!(
        matches!(run.port, Err(PlanError::Game(_))),
        "{}",
        run.dump()
    );
    assert!(run.log.is_empty(), "a fault is no diagnostic");
}

#[test]
fn a_history_that_does_not_answer_ends_the_plan_with_a_fault() {
    // A particle naming a file nothing holds: the plan asks the history whether the game
    // ever shipped the path.
    let files = own_mod(
        &[("fx", "P/Nowhere")],
        vec![particle("P/Nowhere", "assets/mod/a/nowhere.tex")],
    );
    let good = env(skins::game(), NoHistory);
    let facts = FactRun::of(&good, &archive(&files));
    let targets = Policy::default().decide(&facts.facts);
    let history = BrokenHistory(MemoryHistory::new(&["16.19.1"]));
    let broken = env(skins::game(), history);
    let run = PlanRun::decided(&broken, facts, targets, PortOptions::default());
    assert!(
        matches!(run.port, Err(PlanError::History(_))),
        "{}",
        run.dump()
    );
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
fn units_of_one_rank_on_one_slot_leave_the_port_and_the_champion_ships() {
    // #ADR-0076: two pets on models of their own at slots 1 and 2 follow the champion's paste
    // of skin 3 there.
    let mut files = vec![mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    for slot in [1, 2] {
        let prefix = format!("pet{slot}");
        files.extend(own_files(&prefix));
        let pet = own_skin(
            &prefix,
            &format!("Characters/TestPet/Animations/Skin{slot}"),
            &format!("Characters/TestPet/Skins/Skin{slot}/Resources"),
        );
        let entry = format!("Characters/TestPet/Skins/Skin{slot}");
        files.push(mod_skin("testpet", slot, &entry, &pet, vec![]));
    }
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    assert_eq!(
        run.port.shipping.ships,
        [run.unit("test", 0)],
        "{}",
        run.dump()
    );
    for slot in [1, 2] {
        let left = &run.port.shipping.left[&run.unit("testpet", slot)];
        assert_eq!(left.why, Left::Conflict);
    }
}

#[test]
fn what_a_unit_that_leaves_reaches_is_not_reported() {
    // #ADR-0072: the game's own skin 7 bin, with an object nothing reaches.
    let mut files = own_mod(&[], vec![]);
    files.push((
        skin_bin("test", 7),
        bin(vec![
            entry("Characters/Test/Skins/Skin7", &skins::skin7()),
            resolver("Characters/Test/Skins/Skin7/Resources"),
            particle("Stray/Particle", "assets/mod/a/stray.tex"),
        ]),
    ));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let left = run.unit("test", 7);
    assert_eq!(
        run.port.shipping.left[&left].why,
        Left::GameSkin,
        "{}",
        run.dump()
    );
    assert!(!run.port.reach.of(left).expect("found").leftovers.is_empty());
    let unreached: Vec<&Unreached> = run.found();
    assert!(
        unreached.iter().all(|found| found.unit != left),
        "{unreached:?}"
    );
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

/// The mod's own rig `prefix` at a slot of the champion: its skin bin, with its graph.
fn own_rig_at(slot: u16, prefix: &str) -> (String, Vec<u8>) {
    let graph = format!("Characters/Test/Animations/Skin{slot}");
    let resources = format!("Characters/Test/Skins/Skin{slot}/Resources");
    let skin = own_skin(prefix, &graph, &resources);
    let entry = format!("Characters/Test/Skins/Skin{slot}");
    mod_skin("test", slot, &entry, &skin, vec![own_graph(prefix, &graph)])
}

/// The mod's rig at skin0, and another rig of the mod's at skin 3 whose mesh, skeleton and
/// texture are files of their kinds.
fn rigs_at_skin0_and_skin3() -> Vec<(String, Vec<u8>)> {
    let mut files = own_mod(&[], vec![]);
    let body: &[[f32; 3]] = &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let of_its_kind = |path: &str| match path.rsplit('.').next() {
        Some("skn") => Some(rigs::mesh(&[("body", body)], 0)),
        Some("skl") => Some(rigs::skeleton(&["Root", "Spine"])),
        Some("tex") => Some(dds(DdsPixels::Dxt1, (4, 4), 1)),
        _ => None,
    };
    let own = own_files("b").into_iter().map(|(path, bytes)| {
        let bytes = of_its_kind(&path).unwrap_or(bytes);
        (path, bytes)
    });
    files.extend(own);
    files.push(own_rig_at(3, "b"));
    files
}

fn sorted(mut units: Vec<reforge::model::entity::UnitId>) -> Vec<reforge::model::entity::UnitId> {
    units.sort();
    units
}

#[test]
fn a_unit_at_a_slot_the_port_does_not_change_leaves_with_its_own_work_named() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&rigs_at_skin0_and_skin3()));

    let (target, other) = (run.unit("test", 0), run.unit("test", 3));
    assert_eq!(run.port.shipping.ships, [target], "{}", run.dump());
    assert_eq!(
        run.port.shipping.left[&other].why,
        Left::OffTarget { target }
    );
    let work = run.only::<WorkLeft>();
    assert_eq!(work.target, target);
    let row = work.units[0];
    let found = (row.unit, row.slot, row.kind);
    assert_eq!(found, (other, Slot(3), WorkKind::SlotWork));
    // Its mesh, skeleton, texture and two clips are bytes the unit at skin0 does not ship.
    let files = (row.models, row.skeletons, row.textures, row.files);
    assert_eq!(files, (1, 1, 1, 2));
}

#[test]
fn the_options_that_keep_other_slots_ship_every_unit_at_its_own_slot() {
    let env = env(skins::game(), NoHistory);
    let options = PortOptions {
        keep_other_slots: true,
        ..PortOptions::default()
    };
    let archive = archive(&rigs_at_skin0_and_skin3());
    let run = PortRun::under(&env, &archive, Policy::default(), options);

    let ships = sorted(vec![run.unit("test", 0), run.unit("test", 3)]);
    let shipped = sorted(run.port.shipping.ships.clone());
    assert_eq!(shipped, ships, "{}", run.dump());
    assert!(run.found::<WorkLeft>().is_empty());
}

#[test]
fn own_objects_alone_at_a_slot_the_port_does_not_change_are_not_supported() {
    // The game's skin 3 with a particle of the mod's on the game's texture.
    let own = "Characters/Test/Skins/Skin3/Particles/Test_Own";
    let skin3 = with_link(
        entry("Characters/Test/Skins/Skin3", &skins::skin3()),
        "fx",
        own,
    );
    let mut files = own_mod(&[], vec![]);
    files.push((skin_bin("test", 3), bin(vec![skin3, particle(own, S3_TEX)])));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let (target, other) = (run.unit("test", 0), run.unit("test", 3));
    let left = &run.port.shipping.left[&other];
    assert_eq!(left.why, Left::OffTarget { target }, "{}", run.dump());
    let row = run.only::<WorkLeft>().units[0];
    assert_eq!(row.kind, WorkKind::Unsupported);
    assert_eq!((row.models, row.skeletons, row.textures), (0, 0, 0));
    assert!(row.objects > 0, "{row:?}");
}

#[test]
fn with_no_skin0_unit_every_unit_ships_at_its_own_slot() {
    // The mod's rigs at skins 3 and 7 and none at skin0, a case #ADR-0082 leaves open.
    let mut files = own_files("a");
    files.extend(own_files("b"));
    files.push(own_rig_at(3, "a"));
    files.push(own_rig_at(7, "b"));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let ships = sorted(vec![run.unit("test", 3), run.unit("test", 7)]);
    let shipped = sorted(run.port.shipping.ships.clone());
    assert_eq!(shipped, ships, "{}", run.dump());
}

#[test]
fn the_games_own_skin0_leaves_and_the_mods_work_at_another_slot_ships() {
    // The game's skin0 bin as the game ships it, beside the mod's rig at skin 3: the target is
    // chosen among the author's units.
    let mut files = own_files("a");
    files.push(own_rig_at(3, "a"));
    let game_skin0 = bin(vec![
        entry(SKIN0, &skins::base_skin()),
        graph(GRAPH0, skins::BASE_CLIPS),
        resolver(RESOURCES0),
        particle(BASE_PARTICLE, BASE_PARTICLE_TEX),
    ]);
    files.push((skin_bin("test", 0), game_skin0));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let work = run.unit("test", 3);
    assert_eq!(run.port.shipping.ships, [work], "{}", run.dump());
    let left = &run.port.shipping.left[&run.unit("test", 0)];
    assert_eq!(left.why, Left::GameSkin);
}

#[test]
fn a_sub_characters_unit_at_another_slot_leaves_with_its_champions() {
    // The champion's rigs at skin0 and skin 3, and models of the pet's own at both.
    let mut files = rigs_at_skin0_and_skin3();
    for (slot, prefix) in [(0, "p"), (3, "q")] {
        files.extend(own_files(prefix));
        let pet = own_skin(
            prefix,
            &format!("Characters/TestPet/Animations/Skin{slot}"),
            &format!("Characters/TestPet/Skins/Skin{slot}/Resources"),
        );
        let entry = format!("Characters/TestPet/Skins/Skin{slot}");
        files.push(mod_skin("testpet", slot, &entry, &pet, vec![]));
    }
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let target = run.unit("test", 0);
    let ships = sorted(vec![target, run.unit("testpet", 0)]);
    let shipped = sorted(run.port.shipping.ships.clone());
    assert_eq!(shipped, ships, "{}", run.dump());
    let left = &run.port.shipping.left[&run.unit("testpet", 3)];
    assert_eq!(left.why, Left::OffTarget { target });
}
