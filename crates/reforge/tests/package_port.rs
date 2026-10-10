//! A port's package over synthetic mods: the patch of the game's skin bin, the port's own
//! bins, the files under the namespace, the HUD layer, a skin bin the game does not ship,
//! the patches applied as the overlay applies them, and the package written. The game is
//! `reforge_testkit::skins`.

use std::io::Cursor;

use ltk_meta::property::values;
use ltk_meta::{Bin, BinObject, PropertyKind, PropertyValueEnum};
use reforge::diag::Diagnostics;
use reforge::diag::Stage;
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::load::Fault;
use reforge::model::known::field;
use reforge::model::{ContentId, SkinEntry, Slot, name_hash, path_hash, value};
use reforge::package::{
    BinWrite, BuildError, Coverage, Done, EntryWhole, FieldKept, Layer, MANIFEST_FILE, Made,
    Modpkg, NewBin, NewBinFailed, NotWritten, Origin, PackageWriter, PatchOf, PortBuilder, Ported,
    ProjectConfig, ProjectDir, ResolverFrozen, UnitPorted,
};
use reforge::port::{HudCarried, Policy, Port, PortOptions, PortValues};
use reforge::store::MemoryStore;
use reforge_testkit::bins::{
    Skin, bin, blob, entry, graph, particle, resolver, resolver_with, text, with_link,
};
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, BASE_CLIPS, BASE_PARTICLE, BASE_PARTICLE_TEX, WAD, skin_bin};
use reforge_testkit::{BrokenGame, FactRun, MemoryGame, MemoryHistory, PortRun};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const OWN_PARTICLE: &str = "Characters/Test/Skins/Skin0/Particles/Test_Own";
const OWN_TEX: &str = "assets/mod/a/own.tex";
const NS: &str = "synthetic.reforgetests";
const SKIN0_BIN: &str = "data/characters/test/skins/skin0.bin";

/// The mod's own rig at skin0 with a particle of its own, reached through the resolver.
fn own_mod() -> Vec<(String, Vec<u8>)> {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
        resolver_with(RESOURCES0, &[("Test_Q_Cast", OWN_PARTICLE)]),
        particle(OWN_PARTICLE, OWN_TEX),
    ];
    let mut files = own_files("a");
    files.push((OWN_TEX.into(), blob(OWN_TEX)));
    files.push((skin_bin("test", 0), bin(objects)));
    files
}

fn object_of<'a>(objects: &'a [BinObject], name: &str) -> &'a BinObject {
    let found = objects.iter().find(|o| o.path_hash == name_hash(name));
    found.unwrap_or_else(|| panic!("no object {name}"))
}

#[test]
fn a_port_is_linked_applied_and_written() {
    let env = env(skins::game(), NoHistory);
    // The run spells the resolver's key: the name the game plays the effect by names the
    // folder of its files.
    let mut facts = FactRun::of(&env, &archive(&own_mod()));
    facts.fix.names.learn_hash("Test_Q_Cast");
    let run = PortRun::from_facts(&env, facts, Policy::default(), PortOptions::default());
    // Nothing the overlay would refuse, nothing newly unresolved, every value typed.
    assert!(
        run.check_errors().is_empty(),
        "{:#?}\n{}",
        run.check_errors(),
        run.dump()
    );

    // One patch on the game's skin bin, which links the unit's own bins.
    let package = run.package();
    let patch = run.patch(&Layer::Base, SKIN0_BIN);
    assert_eq!(patch.path.as_str(), "overrides/test-skin0.ptch");
    let unit = run.unit("test", 0);
    assert!(matches!(patch.of, PatchOf::Skin { unit: of, .. } if of == unit));
    let dir = format!("mods/{NS}/characters/test/skin0");
    let links: Vec<&str> = patch.links.iter().map(|link| link.as_str()).collect();
    assert_eq!(
        links,
        [format!("{dir}/vfx.bin"), format!("{dir}/animations.bin")]
    );
    let manifest = package.manifest(&Layer::Base).expect("a manifest");
    assert!(
        manifest.contains("target: \"data/characters/test/skins/skin0.bin\""),
        "{manifest}"
    );

    // The particle is in the unit's bin under a name of its own, its texture where the port
    // ships it.
    let vfx = run.chunk(&format!("{dir}/vfx.bin"));
    assert!(matches!(vfx.origin, Origin::Made(_)));
    let vfx = run.package().bytes(vfx);
    let made = Bin::from_reader(&mut Cursor::new(&vfx[..])).expect("a bin");
    let carried = format!("Mods/{NS}/Characters/Test/Skins/Skin0/Particles/Test_Own");
    let particle = made
        .objects
        .get(&name_hash(&carried))
        .expect("the particle is renamed");
    let texture = format!("{dir}/vfx/q_cast/own.tex");
    let named: Vec<_> = value::file_refs(particle)
        .iter()
        .map(value::FileRef::hash)
        .collect();
    assert_eq!(named, [path_hash(&texture)], "{}", run.dump());
    assert_eq!(&run.chunk_bytes(&texture)[..], blob(OWN_TEX));
    assert!(package.tables.paths().any(|path| path == texture));
    assert!(
        package
            .tables
            .entries()
            .any(|name| name.eq_ignore_ascii_case(&carried))
    );

    // Applied to the game's bin: the entry names the mod's model, the resolver keeps the
    // game's name and holds the mod's key, and the game's own particle stays.
    let applied = run.applied(&Layer::Base, SKIN0_BIN);
    assert!(applied.skipped.is_empty(), "{:?}", applied.skipped);
    let patched = SkinEntry::read(object_of(&applied.objects, SKIN0));
    let mesh = patched.mesh().expect("a mesh").hash();
    assert_eq!(mesh, path_hash(&format!("{dir}/files/mesh.skn")));
    assert!(run.ships(&format!("{dir}/files/mesh.skn")));
    let resolver = object_of(&applied.objects, RESOURCES0);
    assert_eq!(
        value::map_keys(resolver, field::RESOURCE_MAP),
        [name_hash("Test_Q_Cast")]
    );
    assert!(value::links(resolver).contains(&name_hash(&carried)));
    assert_eq!(run.only::<ResolverFrozen>().keys, 1);
    object_of(&applied.objects, BASE_PARTICLE);
    // The graph differs from the game's and ships under the port's name.
    let graph_name = format!("Mods/{NS}/Characters/Test/Animations/Skin0");
    assert_eq!(patched.graph(), Some(name_hash(&graph_name)));
    let ported = run.only::<UnitPorted>();
    assert_eq!(
        (ported.carried, ported.graph, ported.new_bin),
        (1, true, false)
    );
    let coverage = run.found::<Done>();
    assert!(
        coverage
            .iter()
            .any(|done| matches!(done.0, Coverage::Apply { patches: 1, .. }))
    );

    // The mod's own bin is not shipped: its objects are carried, or left to the game.
    assert!(!run.ships(SKIN0_BIN));
    let left = run.only::<Ported>().left;
    assert_eq!((left.not_carried, left.total()), (1, 1));

    // On disk: the project, its manifest and patch, and the package.
    let out = tempfile::tempdir().expect("a directory");
    let out = camino::Utf8Path::from_path(out.path()).expect("a UTF-8 path");
    let mut log = Diagnostics::new();
    let writers: [&dyn PackageWriter; 2] = [&ProjectDir::new(out), &Modpkg::new(out)];
    for writer in writers {
        let written = package.write(writer, "{}\n", &mut log.at(Stage::Write));
        written.expect("the package writes");
    }
    let project = out.join(&package.config.name);
    let base = project.join("content").join("base");
    assert!(base.join(MANIFEST_FILE).is_file());
    assert!(base.join("overrides").join("test-skin0.ptch").is_file());
    assert!(base.join(WAD).join(&texture).is_file());
    assert!(out.join(package.config.modpkg_name()).is_file());
}

#[test]
fn two_runs_of_one_mod_give_the_same_package_and_report() {
    let env = env(skins::game(), NoHistory);
    let files = own_mod();
    let (first, second) = (
        PortRun::of(&env, &archive(&files)),
        PortRun::of(&env, &archive(&files)),
    );
    assert_eq!(first.package(), second.package());
    assert_eq!(first.report(), second.report());
    let digests = |run: &PortRun<'_>| -> Vec<ContentId> {
        let chunks = run.package().chunks().map(|(_, _, chunk)| chunk.content);
        let patches = run.package().all_patches();
        chunks
            .chain(patches.map(|(_, patch)| ContentId::of(&patch.bytes)))
            .collect()
    };
    assert_eq!(digests(&first), digests(&second));
    // The report holds the plan, and each patched bin object by object.
    let report: serde_json::Value = serde_json::from_str(&first.report()).expect("JSON");
    assert_eq!(
        report["port"]["shipping"]["ships"],
        serde_json::json!(["%u0"])
    );
    let patch = &report["package"]["patches"][0];
    assert_eq!(patch["target"], SKIN0_BIN);
    assert_eq!(patch["applied"]["outcome"], "applied");
    let entry_hex = format!("{:08x}", name_hash(SKIN0).0);
    assert!(
        patch["applied"]["objects"][&entry_hex].is_string(),
        "{patch:#}"
    );
}

#[test]
fn two_units_of_a_character_write_bins_of_their_own() {
    // A skin0 port and a mod written for skin 3, each with a particle of its own, under the
    // options that keep other slots (#ADR-0082).
    let at = |slot: u16, prefix: &str| {
        let names = |rest: &str| format!("Characters/Test/{rest}");
        let (graph_name, resources) = (
            names(&format!("Animations/Skin{slot}")),
            names(&format!("Skins/Skin{slot}/Resources")),
        );
        let own = names(&format!("Skins/Skin{slot}/Particles/Own{slot}"));
        let skin = own_skin(prefix, &graph_name, &resources);
        let e = with_link(
            entry(&names(&format!("Skins/Skin{slot}")), &skin),
            "fx",
            &own,
        );
        let texture = format!("assets/mod/{prefix}/own.tex");
        let objects = vec![e, own_graph(prefix, &graph_name), particle(&own, &texture)];
        let mut files = own_files(prefix);
        files.push((texture.clone(), blob(&texture)));
        files.push((skin_bin("test", slot), bin(objects)));
        files
    };
    let mut files = at(0, "a");
    files.extend(at(3, "b"));
    let env = env(skins::game(), NoHistory);
    let options = PortOptions {
        keep_other_slots: true,
        ..PortOptions::default()
    };
    let archive = archive(&files);
    let run = PortRun::under(&env, &archive, Policy::default(), options);

    assert_eq!(run.port.units.len(), 2, "{}", run.dump());
    for slot in [0, 3] {
        let dir = format!("mods/{NS}/characters/test/skin{slot}");
        assert!(run.ships(&format!("{dir}/vfx.bin")), "{}", run.dump());
        let target = format!("data/characters/test/skins/skin{slot}.bin");
        let patch = run.patch(&Layer::Base, &target);
        assert!(
            patch
                .links
                .iter()
                .any(|link| link.as_str() == format!("{dir}/vfx.bin"))
        );
    }
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
}

#[test]
fn a_sub_character_at_a_slot_the_game_lacks_gets_a_new_bin() {
    // The champion's paste of skin 3 goes there. Its pet follows, and the game ships the
    // pet at skin0 alone.
    let pet = Skin {
        mesh: "assets/mod/pet/mesh.skn".into(),
        skeleton: "assets/mod/pet/skeleton.skl".into(),
        texture: "assets/mod/pet/body.tex".into(),
        ..skins::pet_skin()
    };
    let pet_graph = graph(
        "Characters/TestPet/Animations/Skin0",
        &["assets/mod/pet/idle.anm"],
    );
    let mut files = own_files("pet");
    files.push(mod_skin("test", 0, SKIN0, &skins::skin3(), vec![]));
    let pet_entry = "Characters/TestPet/Skins/Skin0";
    files.push(mod_skin("testpet", 0, pet_entry, &pet, vec![pet_graph]));
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&files));

    let target = "data/characters/testpet/skins/skin3.bin";
    let chunk = run.chunk(target);
    assert!(matches!(chunk.origin, Origin::Made(_)), "{}", run.dump());
    let patches = run.package().patches(&Layer::Base);
    assert!(patches.iter().all(|patch| patch.target.as_str() != target));
    let bytes = run.package().bytes(chunk);
    let written = Bin::from_reader(&mut Cursor::new(&bytes[..])).expect("a bin");
    let entry = written
        .objects
        .get(&name_hash("Characters/TestPet/Skins/Skin3"));
    let entry = SkinEntry::read(entry.expect("the entry under the target's name"));
    // The mod's model, where the port ships it.
    let mesh = entry.mesh().expect("a mesh").hash();
    assert!(run.package().by_hash(mesh).is_some(), "{}", run.dump());
    assert!(run.found::<NewBinFailed>().is_empty());
    // The build writes the bin, and says so where it does.
    let new: Vec<&NewBin> = run.found();
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].target.as_str(), target);
    let ported = run.found::<UnitPorted>();
    assert!(
        ported
            .iter()
            .any(|unit| unit.new_bin && unit.target.as_str() == target)
    );
    // The bin links the port's graph bin.
    let dir = format!("mods/{NS}/characters/testpet/skin3");
    let graph_bin = format!("{dir}/animations.bin");
    assert!(written.dependencies.contains(&graph_bin));
}

/// #GAME-BEHAVIOR-0008: the mod's keys go into the resolver under the game's own name.
#[test]
fn the_resolver_keeps_the_games_keys_under_the_mods() {
    // The game's resolver maps a key to its base particle. The mod's maps one of its own.
    let mut game = skins::game();
    game.insert(
        SKIN0_BIN,
        bin(vec![
            entry(SKIN0, &skins::base_skin()),
            graph(GRAPH0, BASE_CLIPS),
            resolver_with(RESOURCES0, &[("Test_Idle", BASE_PARTICLE)]),
            particle(BASE_PARTICLE, BASE_PARTICLE_TEX),
        ]),
    );
    let env = env(game, NoHistory);
    let run = PortRun::of(&env, &archive(&own_mod()));

    let applied = run.applied(&Layer::Base, SKIN0_BIN);
    let resolver = object_of(&applied.objects, RESOURCES0);
    let keys = value::map_keys(resolver, field::RESOURCE_MAP);
    assert_eq!(keys, [name_hash("Test_Idle"), name_hash("Test_Q_Cast")]);
    assert!(value::links(resolver).contains(&name_hash(BASE_PARTICLE)));
    assert_eq!(run.only::<ResolverFrozen>().keys, 2);
    assert!(run.check_errors().is_empty(), "{:#?}", run.check_errors());
}

const ROOT: &str = "data/characters/test/test.bin";
const ICON: &str = "assets/characters/test/hud/icons2d/test_q.dds";

/// The game with a root bin whose Q names an icon, and a cast time a patch leaves alone.
fn hud_game() -> MemoryGame {
    let icons = values::Container::new(
        PropertyKind::String,
        vec![text("ASSETS/Characters/Test/HUD/Icons2D/Test_Q.dds")],
    );
    let data = values::Struct {
        class_hash: name_hash("SpellDataResource"),
        properties: [
            (
                name_hash("mImgIconName"),
                PropertyValueEnum::Container(icons.expect("a list of strings")),
            ),
            (
                name_hash("mCastTime"),
                PropertyValueEnum::F32(values::F32::new(0.25)),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let spell = BinObject::builder(
        name_hash("Characters/Test/Spells/TestQ"),
        name_hash("SpellObject"),
    )
    .property(
        name_hash("mSpell"),
        PropertyValueEnum::Embedded(values::Embedded(data)),
    )
    .build();
    let mut game = skins::game();
    game.insert(ROOT, bin(vec![spell]));
    game.insert(ICON, blob(ICON));
    game
}

/// A port of the base skin's bin with a file at the icon's path, the fields of a spell
/// named.
fn hud_port<'a>(env: &'a reforge::env::Env, icon: Vec<u8>) -> PortRun<'a> {
    let files = [
        mod_skin(
            "test",
            0,
            SKIN0,
            &skins::base_skin(),
            vec![resolver(RESOURCES0)],
        ),
        (ICON.to_string(), icon),
    ];
    let mut facts = FactRun::of(env, &archive(&files));
    for name in ["mSpell", "mImgIconName", "mCastTime"] {
        facts.fix.names.learn_field(name);
    }
    PortRun::from_facts(env, facts, Policy::default(), PortOptions::default())
}

#[test]
fn the_hud_edits_a_plan_snapshot_keeps_read_back_as_the_port_made_them() {
    let env = env(hud_game(), NoHistory);
    let run = hud_port(&env, b"the mod's own icon".to_vec());
    assert!(
        run.port
            .hud
            .champions
            .iter()
            .any(|hud| !hud.edits.is_empty())
    );
    let store = MemoryStore::new();
    let values = PortValues::of(&run.port, &store);
    let json = serde_json::to_string(&run.port.part()).expect("the port writes");
    let part: Port = serde_json::from_str(&json).expect("the port reads");
    let reopened = values.reopen(part, &store).expect("the values read back");
    assert_eq!(reopened.hud, run.port.hud);
    assert_eq!(reopened.objects, run.port.objects);
    assert_eq!(reopened.units, run.port.units);
}

/// #GAME-BEHAVIOR-0004: the icon is named by a record on the champion's root bin.
#[test]
fn an_overridden_spell_icon_ships_in_the_hud_layer() {
    let env = env(hud_game(), NoHistory);
    let run = hud_port(&env, b"the mod's own icon".to_vec());

    let carried = run.only::<HudCarried>();
    assert_eq!((carried.files, carried.records), (1, 1), "{}", run.dump());
    let place = format!("mods/{NS}/characters/test/hud/test_q.dds");
    let package = run.package();
    let hud: Vec<&str> = package
        .layer(&Layer::Hud)
        .map(|(key, _)| key.path.as_str())
        .collect();
    assert_eq!(hud, [place.as_str()]);
    let patch = run.patch(&Layer::Hud, ROOT);
    assert_eq!(patch.path.as_str(), "overrides/test-hud.ptch");
    assert!(matches!(patch.of, PatchOf::Root { .. }));
    // Applied to the root bin: the icon names the shipped copy, the cast time stays.
    let applied = run.applied(&Layer::Hud, ROOT);
    assert!(applied.skipped.is_empty(), "{:?}", applied.skipped);
    let spell = object_of(&applied.objects, "Characters/Test/Spells/TestQ");
    let named: Vec<_> = value::file_refs(spell)
        .iter()
        .map(value::FileRef::hash)
        .collect();
    assert_eq!(named, [path_hash(&place)]);
    let cast = value::get(spell, &["mSpell", "mCastTime"]);
    assert_eq!(cast, Some(&PropertyValueEnum::F32(values::F32::new(0.25))));
    let layers: Vec<String> = package.layers().map(Layer::name).collect();
    assert_eq!(layers, ["base", "hud"]);
}

#[test]
fn an_icon_equal_to_the_games_is_left_alone() {
    let env = env(hud_game(), NoHistory);
    let run = hud_port(&env, blob(ICON));
    assert!(run.found::<HudCarried>().is_empty(), "{}", run.dump());
    assert!(run.port.hud.is_empty());
    let layers: Vec<String> = run.package().layers().map(Layer::name).collect();
    assert_eq!(layers, ["base"]);
}

#[test]
fn a_stale_copy_is_named_where_the_game_has_the_file() {
    // The mod ships the texture of the base as the game had it one build ago.
    let old = blob("the base texture one build ago");
    let history =
        MemoryHistory::new(&["16.18.1", "16.19.1"]).with_content(skins::BASE_TEX, &old, 0..=0);
    let env = env(skins::game(), history);
    let skin = Skin {
        texture: skins::BASE_TEX.into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    let mut files = own_files("a");
    files.push((skins::BASE_TEX.into(), old));
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![own_graph("a", GRAPH0)],
    ));
    let run = PortRun::of(&env, &archive(&files));

    // The game's newer bytes stand: the entry keeps naming the game's path.
    assert!(!run.ships(skins::BASE_TEX), "{}", run.dump());
    let applied = run.applied(&Layer::Base, SKIN0_BIN);
    let patched = SkinEntry::read(object_of(&applied.objects, SKIN0));
    let texture = patched.texture().expect("a texture").hash();
    assert_eq!(texture, path_hash(skins::BASE_TEX));
    assert_eq!(patched.entry(), name_hash(SKIN0));
    assert_eq!(
        run.port.unit(run.unit("test", 0)).map(|unit| unit.target),
        Some(Slot(0))
    );
    assert_eq!(run.only::<Ported>().left.stale, 1);
}

/// The mod's own rig in the skin bin of a slot, under the names of that slot.
fn own_mod_at(slot: u16) -> Vec<(String, Vec<u8>)> {
    let entry_name = format!("Characters/Test/Skins/Skin{slot}");
    let graph_name = format!("Characters/Test/Animations/Skin{slot}");
    let resources = format!("{entry_name}/Resources");
    let skin = own_skin("a", &graph_name, &resources);
    let mut files = own_files("a");
    let graph = own_graph("a", &graph_name);
    files.push(mod_skin("test", slot, &entry_name, &skin, vec![graph]));
    files
}

#[test]
fn a_carried_field_the_mods_entry_lacks_keeps_the_games_value() {
    // The game's entry holds an armor material, and the mod's entry holds none.
    let mut game = skins::game();
    let theirs = with_link(
        entry(SKIN0, &skins::base_skin()),
        "armorMaterial",
        "Characters/Test/Skins/Skin0/Materials/Armor",
    );
    game.insert(
        SKIN0_BIN,
        bin(vec![
            theirs,
            graph(GRAPH0, BASE_CLIPS),
            resolver(RESOURCES0),
        ]),
    );
    let env = env(game, NoHistory);
    let run = PortRun::of(&env, &archive(&own_mod()));
    assert_eq!(
        run.only::<FieldKept>(),
        &FieldKept {
            unit: run.unit("test", 0),
            fields: vec![name_hash("armorMaterial")],
        }
    );
    // A patch sets fields and removes none: the game's link stands in the patched entry.
    let applied = run.applied(&Layer::Base, SKIN0_BIN);
    let patched = object_of(&applied.objects, SKIN0);
    assert!(patched.properties.contains_key(&name_hash("armorMaterial")));
}

#[test]
fn an_entry_the_games_bin_does_not_hold_goes_into_the_patch_whole() {
    // The game's skin 7 bin holds a resolver and no skin entry.
    let mut game = skins::game();
    let target = skin_bin("test", 7);
    let resources = "Characters/Test/Skins/Skin7/Resources";
    game.insert(&target, bin(vec![resolver(resources)]));
    let env = env(game, NoHistory);
    let run = PortRun::of(&env, &archive(&own_mod_at(7)));

    let unit = run.unit("test", 7);
    assert_eq!(
        run.only::<EntryWhole>(),
        &EntryWhole { unit },
        "{}",
        run.dump()
    );
    // The patch adds the object to the game's bin, under the name the mod gave it.
    let applied = run.applied(&Layer::Base, &target);
    assert!(applied.skipped.is_empty(), "{:?}", applied.skipped);
    let entry_name = "Characters/Test/Skins/Skin7";
    let added = SkinEntry::read(object_of(&applied.objects, entry_name));
    let dir = format!("mods/{NS}/characters/test/skin7");
    let mesh = added.mesh().expect("a mesh").hash();
    assert_eq!(mesh, path_hash(&format!("{dir}/files/mesh.skn")));
    object_of(&applied.objects, resources);
}

#[test]
fn a_unit_whose_target_the_game_has_no_bin_for_is_not_written() {
    // A champion's unit at a slot the game ships no skin bin of.
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&own_mod_at(5)));

    let unit = run.unit("test", 5);
    let target = skin_bin("test", 5);
    let not_written = run.only::<NotWritten>();
    assert_eq!(
        (not_written.unit, not_written.target.as_str()),
        (unit, target.as_str()),
        "{}",
        run.dump()
    );
    // No patch and no bin of the unit's is in the package.
    let package = run.package();
    assert_eq!(package.all_patches().count(), 0, "{}", run.dump());
    assert!(run.found::<UnitPorted>().is_empty());
    assert!(!run.ships(&format!("mods/{NS}/characters/test/skin5/animations.bin")));
    assert_eq!(run.only::<Ported>().bins, 0);
}

#[test]
fn a_game_bin_that_does_not_read_is_a_fault_of_the_build() {
    let planned = env(skins::game(), NoHistory);
    let run = PortRun::of(&planned, &archive(&own_mod()));
    // The bin the unit's patch is for is listed and does not read.
    let game = BrokenGame::at(skins::game(), SKIN0_BIN);
    let sizes = game.sizes();
    let broken = Env::builder().game(game).sizes(sizes).build();
    let broken = broken.expect("an environment");
    let mut log = Diagnostics::new();
    let speller = run.facts.fix.speller();
    let builder = PortBuilder::new(&run.facts.fix.fixed, &run.port, &broken, &speller);
    let built = builder.build(&mut log.at(Stage::Build));
    assert!(
        matches!(built, Err(BuildError::Fault(Fault::Game(_)))),
        "{built:?}"
    );
    // A fault is no diagnostic, and ends no run.
    assert_eq!((log.len(), log.halted()), (0, None));
}

#[test]
fn a_ports_config_says_the_mod_was_ported_and_its_bins_are_written_whole() {
    let env = env(skins::game(), NoHistory);
    let run = PortRun::of(&env, &archive(&own_mod()));
    let config = &run.package().config;
    let fixed = &run.facts.fix.fixed;
    assert_eq!(config, &ProjectConfig::of(fixed, Made::Ported));
    let said = config.description.raw();
    assert!(
        said.starts_with("Ported by reforge from synthetic.fantome ("),
        "{said}"
    );

    // A bin the port makes has no bytes of the mod's to copy: every object is encoded, and
    // the bin is read back against them.
    let dir = format!("mods/{NS}/characters/test/skin0");
    let Origin::Made(written) = &run.chunk(&format!("{dir}/vfx.bin")).origin else {
        panic!("the port made the bin\n{}", run.dump());
    };
    assert_eq!(written.how, BinWrite::Whole);
    let carried = format!("Mods/{NS}/Characters/Test/Skins/Skin0/Particles/Test_Own");
    assert_eq!(
        written.entries.iter().collect::<Vec<_>>(),
        [&name_hash(&carried)]
    );
    assert_eq!(written.edited.len(), 1);
}
