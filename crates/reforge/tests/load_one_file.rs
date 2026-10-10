//! A WAD holds one chunk per path hash, and so does a mod: no step of `load` adds a second file
//! at a path a WAD holds one at. The game is `reforge_testkit::skins`.

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::archive::file_kind;
use reforge::diag::{Diagnostics, Stage};
use reforge::env::Env;
use reforge::game::{GameView, NameBook, Names};
use reforge::history::NoHistory;
use reforge::load::{FileFixCx, LoadOptions, NewFile, TakenFromGame, TakenSkin};
use reforge::model::entity::{
    FileNode, FileSource, FileUse, MadeBy, ModMeta, ModWad, Occupied, WadLayout, WadRole,
};
use std::sync::Arc;

use reforge::model::{Blobs, ChunkPath, ContentId, Mod, Slot, Spelled, WadName, path_hash};
use reforge::store::MemoryStore;
use reforge_testkit::archive::{STORED, zip};
use reforge_testkit::bins::{bin, blob, entry, linked_bin, particle};
use reforge_testkit::skins::{self, BASE_MESH, BASE_TEX, PET_MESH, WAD, pet_skin, skin_bin};
use reforge_testkit::{LoadRun, MemoryHistory, ModArchive};

/// How many files of the mod sit at a path, in any WAD.
fn count(run: &LoadRun<'_>, path: &str) -> usize {
    let hash = path_hash(path);
    let files = run.module().files.values();
    files.filter(|file| file.hash == hash).count()
}

fn node(m: &Mod, wad: reforge::model::entity::WadId, path: &str, bytes: &[u8]) -> FileNode {
    let path = ChunkPath::parse(path).expect("a path");
    FileNode {
        wad,
        hash: path.hash(),
        path: Some(path),
        size: bytes.len() as u64,
        content: m.blobs().keep(bytes.into()),
        kind: file_kind(bytes),
        used: FileUse::Unused,
        source: FileSource::Archive,
    }
}

fn two_wads() -> (Mod, [reforge::model::entity::WadId; 2]) {
    let mut m = Mod::new(
        ModMeta {
            file_name: Spelled::new("synthetic.fantome"),
            sha256: ContentId::of(b"synthetic"),
            mod_id: "000000000000".into(),
            name: None,
            author: None,
            version: None,
            description: None,
        },
        Blobs::new(Arc::new(MemoryStore::new())),
    );
    let mut wad = |name: &str| {
        m.wads.push(ModWad {
            name: WadName::parse(name).expect("a WAD name"),
            role: WadRole::Other,
            layout: WadLayout::Loose,
            files: Vec::new(),
            packed_locale: None,
        })
    };
    let wads = [wad("Test.wad.client"), wad("Other.wad.client")];
    (m, wads)
}

#[test]
fn a_second_file_at_a_path_of_a_wad_is_not_added() {
    let (mut m, [test, other]) = two_wads();
    let first = m
        .push_file(node(&m, test, BASE_TEX, b"first"))
        .expect("the path is free");
    let second = m.push_file(node(&m, test, BASE_TEX, b"second"));
    assert_eq!(second, Err(Occupied(first)));
    assert_eq!((m.files.len(), m.wads[test].files.len()), (1, 1));
    assert_eq!(m.file_at(test, path_hash(BASE_TEX)), Some(first));
    assert_eq!(&*m.bytes(first), b"first");
    // The path in another WAD is another chunk.
    let elsewhere = m
        .push_file(node(&m, other, BASE_TEX, b"second"))
        .expect("free there");
    assert_ne!(elsewhere, first);
    assert_eq!(m.files.len(), 2);
}

#[test]
fn a_file_fixup_adds_nothing_where_the_wad_holds_a_file() {
    let (mut m, [test, _]) = two_wads();
    let from = m
        .push_file(node(&m, test, "assets/x/old.dds", b"old"))
        .expect("free");
    let there = m
        .push_file(node(&m, test, BASE_TEX, b"the mod's own"))
        .expect("free");
    let game = GameView::new(Box::new(skins::game()));
    let (mut book, mut log) = (NameBook::new(), Diagnostics::new());
    let mut sink = log.at(Stage::Load);
    let mut cx = FileFixCx {
        module: &mut m,
        game: &game,
        history: &NoHistory,
        names: &Names::default(),
        book: &mut book,
        sink: &mut sink,
    };
    let new = |to: &str| NewFile {
        from,
        by: MadeBy::MovedAsset,
        to: ChunkPath::parse(to).expect("a path"),
        bytes: b"made".as_slice().into(),
        kind: reforge::model::entity::FileKind::Other,
    };
    assert_eq!(cx.add(new(BASE_TEX)), Err(Occupied(there)));
    let made = cx.add(new("assets/x/new.tex")).expect("free");
    assert_eq!(m.files.len(), 3);
    assert_eq!(
        m.files[made].source,
        FileSource::Made {
            from,
            by: MadeBy::MovedAsset
        }
    );
    assert_eq!(&*m.bytes(there), b"the mod's own");
}

#[test]
fn the_standard_fixups_make_nothing_at_a_path_the_mod_ships() {
    // The base texture in the dropped format and under a resolution prefix, and the mod's own
    // file at the path both would be made at.
    let dds = [b"DDS ".as_slice(), &[0u8; 124]].concat();
    let dropped = "assets/characters/test/skins/base/test.dds";
    let prefixed = "assets/characters/test/skins/base/2x_test.dds";
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("test", 0), bin(vec![]))
        .file(WAD, dropped, dds.clone())
        .file(WAD, prefixed, dds)
        .file(WAD, BASE_TEX, b"the mod's own texture".to_vec());
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]).with_path(prefixed, 0..=0);
    let env = Env::builder()
        .game(skins::game())
        .history(Box::new(history));
    let env = env.build().expect("an environment");
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert_eq!(count(&run, BASE_TEX), 1);
    assert_eq!(run.file(BASE_TEX).1.source, FileSource::Archive);
    assert_eq!(run.module().files.len(), 4);
}

#[test]
fn an_archive_listing_a_path_twice_gives_one_file() {
    let entries: [(&str, &[u8], _); 3] = [
        (
            "WAD/Test.wad.client/data/characters/test/skins/skin0.bin",
            &bin(vec![]),
            STORED,
        ),
        ("WAD/Test.wad.client/assets/a.tex", b"first", STORED),
        ("WAD/Test.wad.client/ASSETS/A.tex", b"second", STORED),
    ];
    let env = Env::builder()
        .game(skins::game())
        .build()
        .expect("an environment");
    let run = LoadRun::of_bytes(&env, LoadOptions::new(), &zip(&entries));
    assert_eq!(count(&run, "assets/a.tex"), 1);
    assert_eq!(&*run.module().bytes(run.file("assets/a.tex").0), b"first");
    assert!(run.codes().contains(&"archive.chunk-duplicate"));
}

/// Names that spell the sub-character's skin bin, as the hash cache does: the step for mods
/// with no skin bin finds a champion's sub-characters by them.
fn names_with_the_pet() -> (tempfile::TempDir, Names) {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("UTF-8");
    let pet = skin_bin("testpet", 0);
    let line = format!("{:x} {pet}\n", path_hash(&pet).0);
    fs::write(path.join("hashes.game.txt"), line).expect("the table writes");
    let names = Names::builder().text_tables(&path).build().expect("names");
    (dir, names)
}

#[test]
fn a_mod_with_no_champion_skin_bin_keeps_its_own_bin_of_a_sub_character() {
    // The WAD ships the sub-character's skin bin and its mesh, and the base mesh. It ships no
    // skin bin of the champion: the game's is taken for the base mesh, and the mod's own bin
    // of the sub-character stands where the game's would be taken for the sub-character's mesh.
    let own = entry("Characters/TestPet/Skins/Skin0", &pet_skin());
    let own = bin(vec![
        own,
        particle("Characters/TestPet/Skins/Skin0/P", "assets/mod/p.tex"),
    ]);
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("testpet", 0), own.clone())
        .file(WAD, PET_MESH, b"the mod's own pet".to_vec())
        .file(WAD, BASE_MESH, b"the mod's own mesh".to_vec());
    let (_dir, names) = names_with_the_pet();
    let env = Env::builder()
        .game(skins::game())
        .names(names)
        .build()
        .expect("an environment");
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    let m = run.module();

    assert_eq!(count(&run, &skin_bin("testpet", 0)), 1);
    let (pet_bin, node) = run.file(&skin_bin("testpet", 0));
    assert_eq!(node.source, FileSource::Archive);
    assert_eq!(&*m.bytes(pet_bin), own.as_slice());
    assert_eq!(m.units[run.unit("testpet", 0)].file, pet_bin);
    assert_eq!(m.units[run.unit("testpet", 0)].loads.len(), 1);

    // The champion's skin bin is the game's, and it is the one bin taken.
    let taken = run.only::<TakenFromGame>();
    let base = TakenSkin {
        character: "test".into(),
        slot: Slot(0),
    };
    assert_eq!(taken.skins, [base]);
    assert_eq!(taken.bins, [run.file(&skin_bin("test", 0)).0]);
    assert_eq!(count(&run, &skin_bin("test", 0)), 1);
    assert_eq!(m.bins.len(), 2);
    // Every path of every WAD holds one file.
    let mut paths: Vec<_> = m.files.values().map(|file| (file.wad, file.hash)).collect();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), m.files.len());
}

#[test]
fn a_mod_shipping_only_a_sub_characters_bin_takes_no_bin_of_the_game_for_it() {
    let own = bin(vec![entry("Characters/TestPet/Skins/Skin0", &pet_skin())]);
    let archive = ModArchive::new()
        .file(WAD, &skin_bin("testpet", 0), own)
        .file(WAD, PET_MESH, b"the mod's own pet".to_vec());
    let (_dir, names) = names_with_the_pet();
    let env = Env::builder()
        .game(skins::game())
        .names(names)
        .build()
        .expect("an environment");
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    // Nothing is taken, so the WAD holds no skin bin of its champion: no skin mod.
    assert_eq!(run.log.of::<TakenFromGame>().count(), 0);
    assert_eq!(run.codes(), ["wad.not-carried", "archive.no-skin"]);
}

#[test]
fn a_shared_bin_the_mod_ships_is_not_taken_from_the_game_beside_it() {
    // The game's shared bin names the base mesh, which the mod overrides, and the mod ships
    // its own copy of the shared bin.
    const SHARED: &str = "data/characters/test/test_multi_skins_skin0_skins_skin3.bin";
    let mut game = skins::game();
    let skin0 = vec![entry("Characters/Test/Skins/Skin0", &skins::base_skin())];
    let links = ["DATA/Characters/Test/Test_Multi_Skins_Skin0_Skins_Skin3.bin"];
    game.insert(&skin_bin("test", 0), linked_bin(skin0, &links));
    game.insert(
        SHARED,
        bin(vec![particle("Characters/Test/Skins/Skin0/P", BASE_MESH)]),
    );
    let archive = ModArchive::new()
        .file(
            WAD,
            SHARED,
            bin(vec![particle("Characters/Test/Skins/Skin0/P", BASE_TEX)]),
        )
        .file(WAD, BASE_MESH, blob("the mod's own mesh"));
    let env = Env::builder().game(game).build().expect("an environment");
    let run = LoadRun::of(&env, LoadOptions::new(), &archive);
    assert_eq!(count(&run, SHARED), 1);
    assert_eq!(run.file(SHARED).1.source, FileSource::Archive);
    assert_eq!(
        run.only::<TakenFromGame>().bins,
        [run.file(&skin_bin("test", 0)).0]
    );
}
