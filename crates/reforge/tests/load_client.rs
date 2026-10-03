//! What the client loads: skin bins and the bins they link, resolved to the mod's file, the
//! game's, or nothing; the skin entry found anywhere in a unit's load; bins nothing links never
//! read; files marked by how the client comes to use them. The game is
//! `reforge_testkit::skins`.

use ltk_meta::BinObject;
use reforge::env::Env;
use reforge::load::{
    BinUnreadable, EntryDuplicates, FormerPathFiles, LinksMissing, LoadOptions, MAX_LOAD_BINS,
    UnusedFiles,
};
use reforge::model::bin::BinError;
use reforge::model::entity::{FileUse, LinkTo};
use reforge::model::{Slot, name_hash};
use reforge_testkit::bins::{Skin, bin, blob, entry, graph, linked_bin, particle};
use reforge_testkit::skins::{self, BASE_TEX, S3_CLIPS, WAD, skin_bin, skin3};
use reforge_testkit::{LoadRun, MemoryHistory, ModArchive};

const ENTRY: &str = "Characters/Test/Skins/Skin0";
const GRAPH: &str = "Characters/Test/Animations/Skin0";

fn env() -> Env {
    Env::builder()
        .game(skins::game())
        .build()
        .expect("an environment")
}

fn options() -> LoadOptions {
    LoadOptions::new().without_fixups()
}

/// The mod's own model, textures and clips, with a skin entry and a graph naming them.
fn own_work() -> (ModArchive, Skin, BinObject) {
    let mut archive = ModArchive::new();
    for file in [
        "mesh.skn",
        "skeleton.skl",
        "body.tex",
        "idle.anm",
        "dance.anm",
    ] {
        let path = format!("assets/mod/a/{file}");
        archive = archive.file(WAD, &path, blob(&path));
    }
    let skin = Skin {
        mesh: "assets/mod/a/mesh.skn".into(),
        skeleton: "assets/mod/a/skeleton.skl".into(),
        texture: "assets/mod/a/body.tex".into(),
        graph: GRAPH.into(),
        resolver: "Characters/Test/Skins/Skin0/Resources".into(),
        name: "Test".into(),
        class: None,
        parent: None,
    };
    let clips = graph(GRAPH, &["assets/mod/a/idle.anm", "assets/mod/a/dance.anm"]);
    (archive, skin, clips)
}

#[test]
fn a_repathed_skin_bin_keeps_its_entry_in_a_linked_bin() {
    // The skin bin holds nothing but links. The entry and the graph are in a bin the mod named.
    let (archive, skin, clips) = own_work();
    let archive = archive
        .file(
            WAD,
            &skin_bin("test", 0),
            linked_bin(vec![], &["DATA/Mod_Test_Skin0.bin"]),
        )
        .file(
            WAD,
            "data/mod_test_skin0.bin",
            bin(vec![entry(ENTRY, &skin), clips]),
        );
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    let unit = &m.units[run.unit("test", 0)];
    let entry = unit.entry.expect("the entry is found in the linked bin");
    assert_eq!(
        m.bins[m.objects[entry].bin].file,
        run.file("data/mod_test_skin0.bin").0
    );
    assert_eq!(unit.loads.len(), 2);
    assert_eq!(unit.bin, Some(unit.loads[0]), "the skin bin first");
    assert_eq!(unit.entry_name_slot, Some(Slot(0)));
}

#[test]
fn a_bin_nothing_links_is_never_read() {
    // Beside its own work, the mod carries an old bin under another name.
    let (archive, skin, clips) = own_work();
    let old = "data/characters/test/skins/skin0_old.bin";
    let archive = archive
        .file(
            WAD,
            &skin_bin("test", 0),
            bin(vec![entry(ENTRY, &skin), clips]),
        )
        .file(
            WAD,
            old,
            bin(vec![entry(ENTRY, &skin3()), graph(GRAPH, S3_CLIPS)]),
        )
        .file(WAD, "assets/mod/a/body_old.tex", blob("an old texture"));
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    let (stale, node) = run.file(old);
    assert_eq!(node.used, FileUse::Unused);
    assert!(m.bin_of(stale).is_none(), "an unused bin is not parsed");
    let (junk, node) = run.file("assets/mod/a/body_old.tex");
    assert_eq!(node.used, FileUse::Unused);
    let unused = run.only::<UnusedFiles>();
    assert_eq!(unused.files.len(), 2);
    assert!(unused.files.contains(&junk));
    assert_eq!(unused.bins, [stale]);
    assert_eq!(unused.bytes, m.files[stale].size + m.files[junk].size);
    assert_eq!(m.bins.len(), 1);
}

#[test]
fn links_resolve_to_the_mod_then_the_game_then_nothing() {
    let (archive, skin, clips) = own_work();
    let links = [
        "DATA/Characters/Test/Skins/Skin3.bin",
        "DATA/Mod_Extra.bin",
        "DATA/Nowhere.bin",
    ];
    let archive = archive
        .file(
            WAD,
            &skin_bin("test", 0),
            linked_bin(vec![entry(ENTRY, &skin), clips], &links),
        )
        .file(WAD, "data/mod_extra.bin", bin(vec![]));
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    let (extra, node) = run.file("data/mod_extra.bin");
    assert_eq!(node.used, FileUse::Loaded);
    let skin_bin = m.units[run.unit("test", 0)]
        .bin
        .expect("the skin bin reads");
    let to: Vec<LinkTo> = m.bins[skin_bin].links.iter().map(|l| l.to).collect();
    assert_eq!(to, [LinkTo::Game, LinkTo::Mod(extra), LinkTo::Missing]);
    assert_eq!(m.bins[skin_bin].links[2].path.raw(), "DATA/Nowhere.bin");

    let missing = run.only::<LinksMissing>();
    assert_eq!(missing.bin, skin_bin);
    assert_eq!(missing.links.len(), 1);
    assert_eq!(missing.links[0].path.raw(), "DATA/Nowhere.bin");
    assert!(!missing.links[0].former);
}

#[test]
fn files_are_marked_by_how_the_client_uses_them() {
    // Own files the entry names are referenced. A file at a game path the mod's bins do not
    // name overrides the game's. Anything else is unused.
    let (archive, skin, clips) = own_work();
    let archive = archive
        .file(
            WAD,
            &skin_bin("test", 0),
            bin(vec![entry(ENTRY, &skin), clips]),
        )
        .file(WAD, BASE_TEX, blob("a repainted base texture"))
        .file(WAD, "assets/mod/a/unused.skn", blob("unused"));
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);

    let used = |path: &str| run.file(path).1.used;
    assert_eq!(used(&skin_bin("test", 0)), FileUse::Loaded);
    assert_eq!(used("assets/mod/a/mesh.skn"), FileUse::Referenced);
    assert_eq!(used("assets/mod/a/idle.anm"), FileUse::Referenced);
    assert_eq!(used(BASE_TEX), FileUse::Override);
    assert_eq!(used("assets/mod/a/unused.skn"), FileUse::Unused);
}

#[test]
fn a_file_where_the_game_used_to_have_one_is_at_a_former_path() {
    // The game once shipped a `.dds` the mod repainted, and a shared bin the mod's bin links.
    const OLD: &str = "assets/characters/test/skins/base/test_old.dds";
    let (archive, skin, clips) = own_work();
    let bins = linked_bin(
        vec![entry(ENTRY, &skin), clips],
        &["DATA/Test_Skins_Skin0_Old.bin"],
    );
    let archive = archive
        .file(WAD, &skin_bin("test", 0), bins)
        .file(WAD, OLD, blob(OLD));
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"])
        .with_path(OLD, 0..=0)
        .with_path("data/test_skins_skin0_old.bin", 0..=0);
    let env = Env::builder()
        .game(skins::game())
        .history(Box::new(history))
        .build()
        .expect("an environment");
    let run = LoadRun::of(&env, options(), &archive);

    let (old, node) = run.file(OLD);
    assert_eq!(node.used, FileUse::FormerPath);
    let former = run.only::<FormerPathFiles>();
    assert_eq!(
        (former.files.as_slice(), former.bytes),
        ([old].as_slice(), node.size)
    );
    let missing = run.only::<LinksMissing>();
    assert!(
        missing.links[0].former,
        "the game shipped the shared bin at an earlier build"
    );
    let message = run
        .messages()
        .into_iter()
        .find(|m| m.starts_with("bin.link-missing"));
    let message = message.expect("the finding prints");
    assert!(
        message.contains("1 of them the game shipped at earlier builds"),
        "{message}"
    );
}

#[test]
fn an_entry_under_another_slots_name_is_found_by_its_class() {
    // A copy of the base's bin at slot 3 keeps the base's entry name.
    let (archive, skin, clips) = own_work();
    let archive = archive.file(
        WAD,
        &skin_bin("test", 3),
        bin(vec![clips, entry(ENTRY, &skin)]),
    );
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();
    let unit = &m.units[run.unit("test", 3)];
    let entry = unit.entry.expect("the first object of the skin class");
    assert_eq!(m.objects[entry].entry, name_hash(ENTRY));
    assert_eq!(unit.entry_name_slot, Some(Slot(0)));
}

#[test]
fn a_skin_bin_that_does_not_parse_is_loaded_and_reported() {
    let archive = ModArchive::new().file(WAD, &skin_bin("test", 0), b"PROP not a bin".to_vec());
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    let unit = &m.units[run.unit("test", 0)];
    assert_eq!((unit.bin, unit.entry, unit.loads.len()), (None, None, 0));
    let (file, node) = run.file(&skin_bin("test", 0));
    assert_eq!(node.used, FileUse::Loaded);
    let unreadable = run.only::<BinUnreadable>();
    assert_eq!(m.bins[unreadable.bin].file, file);
    assert!(m.bins[unreadable.bin].error.is_some());
}

#[test]
fn a_patch_at_a_skin_bins_path_is_no_bin() {
    let archive = ModArchive::new().file(WAD, &skin_bin("test", 0), b"PTCH\x01\0\0\0".to_vec());
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    assert_eq!(run.only::<BinUnreadable>().why, BinError::Patch);
}

#[test]
fn a_bin_listing_an_entry_twice_keeps_the_last_and_is_reported() {
    // Two objects of one size, the second renamed to the first's entry hash in the bytes.
    let first = "Characters/Test/Skins/Skin0/Particles/A";
    let second = "Characters/Test/Skins/Skin0/Particles/B";
    let objects = vec![
        particle(first, "assets/x/one.tex"),
        particle(first, "assets/x/two.tex"),
    ];
    let mut objects = objects;
    objects[1].path_hash = name_hash(second);
    let mut bytes = bin(objects);
    let (from, to) = (
        name_hash(second).0.to_le_bytes(),
        name_hash(first).0.to_le_bytes(),
    );
    let at = bytes
        .windows(4)
        .position(|w| w == from)
        .expect("the second entry hash");
    bytes[at..at + 4].copy_from_slice(&to);

    let archive = ModArchive::new().file(WAD, &skin_bin("test", 0), bytes);
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    let duplicates = run.only::<EntryDuplicates>();
    assert_eq!(duplicates.entries, [name_hash(first)]);
    assert_eq!(m.bins[duplicates.bin].objects.len(), 1);
}

#[test]
fn a_bin_at_a_path_the_game_ships_is_loaded_with_no_unit() {
    const ROOT: &str = "data/characters/test/test.bin";
    let mut game = skins::game();
    game.insert(ROOT, bin(vec![]));
    let (archive, skin, clips) = own_work();
    let spell = particle("Characters/Test/Spells/Q", "assets/mod/a/q.tex");
    let archive = archive
        .file(
            WAD,
            &skin_bin("test", 0),
            bin(vec![entry(ENTRY, &skin), clips]),
        )
        .file(WAD, ROOT, bin(vec![spell]))
        .file(WAD, "assets/mod/a/q.tex", blob("q"));
    let env = Env::builder().game(game).build().expect("an environment");
    let run = LoadRun::of(&env, options(), &archive);
    let m = run.module();

    let (root, node) = run.file(ROOT);
    assert_eq!(node.used, FileUse::Loaded);
    let root_bin = m.bin_of(root).expect("read");
    assert!(m.units.values().all(|unit| !unit.loads.contains(&root_bin)));
    // Bins in load order: the unit's load, then the bins at game paths.
    assert_eq!(root_bin.index(), 1);
    assert_eq!(run.file("assets/mod/a/q.tex").1.used, FileUse::Referenced);
}

#[test]
fn a_load_follows_no_more_bins_than_the_cap() {
    let chain = MAX_LOAD_BINS + 6;
    let name = |i: usize| format!("data/mod/link{i}.bin");
    let first = linked_bin(vec![], &[name(0).as_str()]);
    let mut archive = ModArchive::new().file(WAD, &skin_bin("test", 0), first);
    for i in 0..chain {
        let next = linked_bin(vec![], &[name(i + 1).as_str()]);
        archive = archive.file(WAD, &name(i), next);
    }
    let env = env();
    let run = LoadRun::of(&env, options(), &archive);
    let unit = &run.module().units[run.unit("test", 0)];
    assert_eq!(unit.loads.len(), MAX_LOAD_BINS);
    assert_eq!(run.file(&name(chain - 1)).1.used, FileUse::Unused);
}
