//! The index written from a history in memory and read back: every key found with its life,
//! absent keys absent, the stamp checked against a history's head and an install's build.

mod common;
mod link;

use camino::{Utf8Path, Utf8PathBuf};
use ltk_meta::PropertyKind as Kind;
use reforge::diag::{Diagnostics, Stage};
use reforge::game::Build;
use reforge::game::FieldType;
use reforge::history::{
    BinEntry, ClipJoints, Families, FileKind, History, IndexStale, InstallNewer, Joint, Life,
    NamedBy, PlayedBy, Position, SqliteHistory, UnitHit, WemPlayed,
};
use reforge::model::{
    BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash, name_hash, path_hash,
};
use reforge_census::Indexer;
use tempfile::TempDir;

use self::common::{A, B, HEADER, JOINTS, NAMED, bank, build, entry, memory, object_text};
use self::link::linking;

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = TempDir::new().unwrap();
    let out = Utf8Path::from_path(dir.path()).unwrap().join("index");
    (dir, out)
}

#[test]
fn round_trip() {
    let (_dir, out) = scratch();
    let history = memory();
    let stamp = Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    assert_eq!(stamp.builds, 3);
    assert_eq!(stamp.head, "c2");
    assert_eq!(stamp.newest_build.as_str(), "1.3.1");
    assert_eq!(stamp.repo, "memory");
    let tables = [
        "files",
        "placements",
        "wad_lives",
        "wads",
        "bins",
        "bin_entries",
        "objects",
        "refs",
        "clips",
        "skeletons",
        "joints",
        "textures",
        "joint_lists",
        "list_joints",
        "animations",
        "bank_objects",
        "event_banks",
    ];
    let counts = tables.map(|table| stamp.rows_of(table));
    assert_eq!(counts, [9, 11, 1, 1, 1, 1, 1, 3, 2, 2, 3, 2, 1, 2, 2, 3, 2]);

    let index = SqliteHistory::open(&out).unwrap();
    assert_eq!(index.stamp(), Some(&stamp));
    // Every field of `build.yaml` is the build's.
    assert_eq!(index.builds()[1].details, history.commits[1].0.details);
    assert_eq!(index.builds()[1].details.patch, "1.2");
    let (ka, kb) = (FileKey::parse(A).unwrap(), FileKey::parse(B).unwrap());
    let a = index.file(ka).unwrap().unwrap();
    assert_eq!(a.life, Life::new(0, 2));
    assert_eq!(a.path, WadHash(1));
    assert_eq!(a.kind, FileKind::of("tex"));
    assert_eq!(index.file(kb).unwrap().unwrap().life, Life::new(1, 1));
    assert!(index.file(FileKey([0x77; 16])).unwrap().is_none());

    // Path 1 held a file in every build (A, then B, then A); path 3 only in the last.
    let path = |hash| index.path(WadHash(hash)).unwrap();
    assert_eq!(path(1).unwrap().life, Life::new(0, 2));
    assert_eq!(path(3).unwrap().life, Life::new(2, 2));
    assert!(path(6).is_none());
    assert_eq!(index.head(), Some(Position(2)));

    // Every path a content lived at, where `file` names only the first: A at path 3 too.
    assert_eq!(index.held(WadHash(1), ka).unwrap(), Some(Life::new(0, 2)));
    assert_eq!(index.held(WadHash(3), ka).unwrap(), Some(Life::new(2, 2)));
    assert_eq!(index.held(WadHash(1), kb).unwrap(), Some(Life::new(1, 1)));
    assert_eq!(index.held(WadHash(3), kb).unwrap(), None);
    let paths = index.paths_of(ka).unwrap();
    let paths: Vec<WadHash> = paths.iter().map(|at| at.path).collect();
    assert_eq!(paths, [WadHash(1), WadHash(3)]);

    let o = index.object(ObjectKey::parse(A).unwrap()).unwrap().unwrap();
    assert_eq!((o.entry, o.class), (BinHash(1), BinHash(2)));
    assert_eq!((o.bin, o.norm), (WadHash(2), NormKey(7)));
    assert_eq!(index.objects_by_norm(NormKey(7)).unwrap(), [o]);
    assert!(
        index
            .object(ObjectKey::parse(B).unwrap())
            .unwrap()
            .is_none()
    );

    // Which object named path 1, and when.
    let whole = Life::new(0, 2);
    let named = NamedBy {
        bin: WadHash(2),
        entry: BinHash(1),
        life: whole,
    };
    assert_eq!(index.named_by(WadHash(1)).unwrap(), [named]);
    // The object's voice unit loads the bank, whose event 0xe1 plays wem 1, in every build.
    let unit = UnitHit {
        bin: WadHash(2),
        entry: BinHash(1),
        unit: "Test_Base_VO".into(),
        voice: true,
        life: whole,
    };
    assert_eq!(index.units_loading(bank()).unwrap(), [unit]);
    let wem = WemPlayed {
        wem: 1,
        life: whole,
    };
    assert_eq!(index.event_wems(bank(), 0xe1).unwrap(), [wem]);
    let event = PlayedBy {
        bank: bank(),
        event: 0xe1,
        life: whole,
    };
    assert_eq!(index.events_playing(1).unwrap(), [event]);

    // The skeleton at path 4, before and after a joint was added.
    let skeletons = index.skeletons(WadHash(4)).unwrap();
    let shape: Vec<(Life, usize)> = skeletons
        .iter()
        .map(|s| (s.life, s.skeleton.joints.len()))
        .collect();
    assert_eq!(shape, [(Life::new(0, 1), 1), (Life::new(2, 2), 2)]);
    assert_eq!(skeletons[1].skeleton.influences, 4);
    let hip = Joint {
        name: "Hip".into(),
        parent: Some("Root".into()),
    };
    assert_eq!(skeletons[1].skeleton.joints[1], hip);
    assert_eq!(skeletons[1].skeleton.joints[0].parent, None);
    assert!(index.meshes(WadHash(4)).unwrap().is_empty());

    assert_eq!(index.clip(ClipKey(2)).unwrap(), Some(Life::new(1, 2)));
    assert_eq!(index.clip(ClipKey(3)).unwrap(), None);
    assert_eq!(index.span(whole), "1.1.1 .. 1.3.1");
    assert!(index.reaches_head(whole) && !index.reaches_head(Life::new(1, 1)));
    assert_eq!(index.first_of_patch("1.2"), Some(Position(1)));

    let mut log = Diagnostics::new();
    let install = |version: &str| Build::parse(version).unwrap();
    stamp.check(
        Some("c2"),
        Some(&install("1.3.1")),
        &mut log.at(Stage::Read),
    );
    assert!(log.is_empty());
    stamp.check(
        Some("c3"),
        Some(&install("1.4.0")),
        &mut log.at(Stage::Read),
    );
    assert_eq!(log.of::<IndexStale>().count(), 1);
    assert_eq!(log.of::<InstallNewer>().count(), 1);
}

#[test]
fn a_path_whose_bytes_go_and_come_back_has_a_placement_per_life() {
    let (_dir, out) = scratch();
    Indexer::new(&memory(), "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    let (ka, kb) = (FileKey::parse(A).unwrap(), FileKey::parse(B).unwrap());
    // Path 1 holds bytes A, then B, then A again.
    let at = |build: u16| {
        let placed = index.placed(WadHash(1), Position(build)).unwrap();
        let placed = placed
            .into_iter()
            .map(|p| (p.wad, p.key, p.checksum, p.life));
        placed.collect::<Vec<_>>()
    };
    let wad = "data/x.wad.client".to_string();
    assert_eq!(at(0), [(wad.clone(), ka, 1, Life::new(0, 0))]);
    assert_eq!(at(1), [(wad.clone(), kb, 1, Life::new(1, 1))]);
    assert_eq!(at(2), [(wad, ka, 1, Life::new(2, 2))]);
    assert!(index.placed(WadHash(3), Position(1)).unwrap().is_empty());
    // The hull of the lives answers as the content's path did before.
    assert_eq!(index.held(WadHash(1), ka).unwrap(), Some(Life::new(0, 2)));
}

#[test]
fn a_bin_at_a_path_is_read_back_with_its_links_and_its_objects() {
    let (_dir, out) = scratch();
    let mut history = memory();
    // From the second build the bin at path 2 links another bin and holds a second object.
    let bin = format!(
        "sha256: \"{}\"\nkind: \"bin\"\nlinks:\n - \"DATA/Characters/Test/Test.bin\"\nobjects:\n \
         \"00000001\":\n  class: \"00000002\"\n  content: \"{A}\"\n  norm: \
         \"0000000000000007\"\n \"00000003\":\n  class: \"00000004\"\n  content: \"{B}\"\n  \
         norm: \"0000000000000008\"\n",
        "8".repeat(64)
    );
    for (_, files) in &mut history.commits[1..] {
        files.extend([entry(2, bin.clone())]);
    }
    Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    let object = |entry, class, key: &str, norm| BinEntry {
        entry: BinHash(entry),
        class: BinHash(class),
        object: ObjectKey::parse(key).unwrap(),
        norm: NormKey(norm),
    };

    let first = index.bin(WadHash(2), Position(0)).unwrap().unwrap();
    assert_eq!(first.wad, "data/x.wad.client");
    assert_eq!(first.key, FileKey::parse(&"9".repeat(64)).unwrap());
    assert_eq!(first.life, Life::new(0, 0));
    assert!(first.links.is_empty());
    assert_eq!(first.entries, [object(1, 2, A, 7)]);

    let later = index.bin(WadHash(2), Position(2)).unwrap().unwrap();
    assert_eq!(later.life, Life::new(1, 2));
    assert_eq!(later.links, [path_hash("DATA/Characters/Test/Test.bin")]);
    assert_eq!(later.entries, [object(1, 2, A, 7), object(3, 4, B, 8)]);
    // A texture is no bin.
    assert_eq!(index.bin(WadHash(1), Position(0)).unwrap(), None);
}

#[test]
fn an_object_a_bin_holds_under_another_class_has_a_row_per_class() {
    let (_dir, out) = scratch();
    let mut history = memory();
    // From the second build a bin at path 6 holds object A's bytes under class 5.
    let bin = format!(
        "sha256: \"{}\"\nkind: \"bin\"\nlinks: []\nobjects:\n \"00000001\":\n  class: \
         \"00000005\"\n  content: \"{A}\"\n  norm: \"0000000000000007\"\n",
        "7".repeat(64)
    );
    for (_, files) in &mut history.commits[1..] {
        files.extend([entry(6, bin.clone())]);
    }
    Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    let key = ObjectKey::parse(A).unwrap();
    let lives = index.object_lives(key).unwrap();
    let lives: Vec<(BinHash, WadHash, Life)> =
        lives.iter().map(|h| (h.class, h.bin, h.life)).collect();
    let expected = [
        (BinHash(2), WadHash(2), Life::new(0, 2)),
        (BinHash(5), WadHash(6), Life::new(1, 2)),
    ];
    assert_eq!(lives, expected);
    // Both reach the head, and the older answers.
    assert_eq!(index.object(key).unwrap().unwrap().class, BinHash(2));
}

#[test]
fn a_fields_kinds_at_a_build_are_the_ones_its_texts_held_then() {
    let (_dir, out) = scratch();
    let mut history = memory();
    let text = "files/data/x.wad.client/00/0000000000000002.bin/00/00000001.rito";
    let string = format!("string = \"{NAMED}\"");
    // The object's icon is a file from the second build. In the third a second object of the
    // class holds it as a string.
    let file = object_text().replace(&string, "file = 0x0000000000000005");
    for (_, files) in &mut history.commits[1..] {
        files.insert(text.into(), file.clone());
    }
    let second = object_text().replace("0x00000001 = 0x00000002", "0x00000003 = 0x00000002");
    let second_text = text.replace("00000001.rito", "00000003.rito");
    history.commits[2].1.insert(second_text, second);
    Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    let plain = |kind| FieldType {
        kind,
        key: None,
        item: None,
        class: None,
    };
    let icon = |build| {
        let types = index.field_types(BinHash(2), name_hash("iconSquare"), Position(build));
        types.unwrap()
    };
    assert_eq!(icon(0), [plain(Kind::String)]);
    assert_eq!(icon(1), [plain(Kind::WadChunkLink)]);
    assert_eq!(icon(2), [plain(Kind::String), plain(Kind::WadChunkLink)]);

    let units = index.field_types(BinHash(2), name_hash("bankUnits"), Position(0));
    let embedded = FieldType {
        kind: Kind::UnorderedContainer,
        key: None,
        item: Some(Kind::Embedded),
        class: Some(name_hash("BankUnit")),
    };
    assert_eq!(units.unwrap(), [embedded]);
    let voice = index.field_types(name_hash("BankUnit"), name_hash("voiceOver"), Position(2));
    assert_eq!(voice.unwrap(), [plain(Kind::Bool)]);
    let unknown = index.field_types(BinHash(2), name_hash("unknown"), Position(0));
    assert!(unknown.unwrap().is_empty());
}

#[test]
fn an_object_reaches_what_it_links_and_what_those_link_at_a_build() {
    let (_dir, out) = scratch();
    let mut history = memory();
    let text = |bin: u64, entry: u32| {
        format!("files/data/x.wad.client/00/{bin:016x}.bin/00/{entry:08x}.rito")
    };
    // 0x11 of bin 2 links 0x12, which links 0x21 of bin 6, which links 0x11 again. From the
    // second build 0x12 links nothing.
    for (at, (_, files)) in history.commits.iter_mut().enumerate() {
        let onward: &[u32] = if at == 0 { &[0x21] } else { &[] };
        files.insert(text(2, 0x11), linking(0x11, &[0x12]));
        files.insert(text(2, 0x12), linking(0x12, onward));
        files.insert(text(6, 0x21), linking(0x21, &[0x11]));
    }
    Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    let (bin, other) = (WadHash(2), WadHash(6));
    let entry = BinHash(0x12);
    assert_eq!(
        index.links(bin, entry, Position(0)).unwrap(),
        [BinHash(0x21)]
    );
    assert!(index.links(bin, entry, Position(1)).unwrap().is_empty());

    let reached = |build| index.reached(bin, BinHash(0x11), Position(build)).unwrap();
    let all = [
        (bin, BinHash(0x11)),
        (bin, BinHash(0x12)),
        (other, BinHash(0x21)),
    ];
    let first = reached(0);
    assert_eq!((first.objects.as_slice(), first.capped), (&all[..], false));
    assert_eq!(reached(1).objects, all[..2]);
    let none = index.reached(bin, BinHash(0x99), Position(0)).unwrap();
    assert!(none.objects.is_empty() && !none.capped);
}

#[test]
fn a_wad_whose_manifest_changes_it_has_a_life_per_version() {
    let (_dir, out) = scratch();
    let mut history = memory();
    let wad = "files/data/x.wad.client/_wad.yaml";
    let changed = "version: \"3.4\"\nfileId: \"0000000000000002\"\ntags:\n - \"en_US\"\n";
    history.commits[1].1.insert(wad.into(), changed.into());
    Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let db = rusqlite::Connection::open(out.join("history.sqlite")).unwrap();
    let mut rows = db
        .prepare(
            "SELECT w.path, v.name, l.file_id, t.name, l.first, l.last FROM wad_lives l \
             JOIN wads w ON w.id = l.wad JOIN names v ON v.id = l.version \
             JOIN names t ON t.id = l.tags ORDER BY l.first",
        )
        .unwrap();
    let lives: Vec<(String, String, i64, String, u16, u16)> = rows
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let life = |file_id, tags: &str, first, last| {
        let wad = "data/x.wad.client".to_string();
        (
            wad,
            "3.4".to_string(),
            file_id,
            tags.to_string(),
            first,
            last,
        )
    };
    let expected = [life(1, "", 0, 0), life(2, "en_US", 1, 1), life(1, "", 2, 2)];
    assert_eq!(lives, expected);
}

#[test]
fn an_animation_clip_is_read_back_with_its_joints_by_path_and_by_key() {
    let (_dir, out) = scratch();
    Indexer::new(&memory(), "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    let key = |bytes: char| FileKey::parse(&bytes.to_string().repeat(64)).unwrap();
    // The clip's bytes change in the last build and its joints stay.
    let clip = |bytes, life| ClipJoints {
        life,
        key: key(bytes),
        joints: JOINTS.to_vec(),
    };
    let expected = [clip('5', Life::new(0, 1)), clip('6', Life::new(2, 2))];
    assert_eq!(index.clip_joints(WadHash(7)).unwrap(), expected);
    assert!(index.clip_joints(WadHash(4)).unwrap().is_empty());
    assert_eq!(index.joints_of(key('6')).unwrap(), Some(JOINTS.to_vec()));
    assert_eq!(index.joints_of(key('c')).unwrap(), None);
}

#[test]
fn an_events_bank_is_read_back_with_its_header_and_its_objects_in_order() {
    let (_dir, out) = scratch();
    Indexer::new(&memory(), "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    let banks = index.events_banks(bank()).unwrap();
    let lives: Vec<Life> = banks.iter().map(|hit| hit.life).collect();
    assert_eq!(lives, [Life::new(0, 0), Life::new(1, 2)]);
    // #ADR-0081: each bank is its file's, and says which WAD held it.
    let key = |bytes: char| FileKey::parse(&bytes.to_string().repeat(64)).unwrap();
    let files: Vec<(FileKey, Vec<(&str, Life)>)> = banks
        .iter()
        .map(|hit| {
            let wads = hit.placed.iter().map(|p| (p.wad.as_str(), p.life));
            (hit.key, wads.collect())
        })
        .collect();
    let expected = [
        (key('e'), vec![("data/x.wad.client", Life::new(0, 0))]),
        (key('f'), vec![("data/x.wad.client", Life::new(1, 2))]),
    ];
    assert_eq!(files, expected);
    for (hit, sound) in banks.iter().zip([1u8, 2]) {
        assert_eq!(hit.bank.header, HEADER);
        assert_eq!((hit.bank.version(), hit.bank.bank_id()), (145, 5));
        let objects: Vec<(u8, u32, &[u8])> = hit
            .bank
            .objects
            .iter()
            .map(|o| (o.kind, o.id, o.body.as_slice()))
            .collect();
        let expected: [(u8, u32, &[u8]); 2] = [(2, 1, &[sound]), (4, 0xe1, &[1, 1, 0, 0, 0])];
        assert_eq!(objects, expected);
    }
    assert!(index.events_banks(WadHash(7)).unwrap().is_empty());

    // A bank is written from what the index holds of it.
    let held = &banks[1].bank;
    let mut written = reforge_wwise::Bank::new_bnk(held.header.clone(), true);
    written.objects = held.objects.clone();
    let read = reforge_wwise::Bank::read(&written.write()).unwrap();
    assert_eq!((read.id(), &read.objects), (5, &held.objects));
}

#[test]
fn a_mesh_is_read_back_with_its_submeshes_in_order() {
    let (_dir, out) = scratch();
    let mut history = memory();
    let mesh = "sha256: \"1111111111111111111111111111111111111111111111111111111111111111\"\n\
                kind: \"skn\"\nmesh:\n vertexType: \"basic\"\n vertices: 30\n indices: 60\n \
                submeshes:\n  - name: \"Body\"\n    vertices: 20\n    indices: 42\n    \
                positions: \"00000000000000b1\"\n    rounded: \"f0000000000000b2\"\n  - name: \
                \"Cape\"\n    vertices: 10\n    indices: 18\n    positions: \
                \"00000000000000c1\"\n    rounded: \"00000000000000c2\"\n";
    history.commits[1].1.extend([entry(5, mesh.to_string())]);
    Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    let meshes = index.meshes(WadHash(5)).unwrap();
    assert_eq!(meshes.len(), 1);
    assert_eq!(meshes[0].life, Life::new(1, 1));
    let mesh = &meshes[0].mesh;
    assert_eq!(mesh.vertex_type, "basic");
    assert_eq!((mesh.vertices, mesh.indices), (30, 60));
    let submeshes: Vec<(&str, u32, u32, u64, u64)> = mesh
        .submeshes
        .iter()
        .map(|s| {
            (
                s.material.as_str(),
                s.vertices,
                s.indices,
                s.positions,
                s.rounded,
            )
        })
        .collect();
    let expected = [
        ("Body", 20, 42, 0xb1, 0xf000_0000_0000_00b2),
        ("Cape", 10, 18, 0xc1, 0xc2),
    ];
    assert_eq!(submeshes, expected);
}

#[test]
fn a_late_hotfix_is_the_head_not_the_newest_build() {
    let (_dir, out) = scratch();
    let mut history = memory();
    history.commits[2].0 = build("1.1.2");
    let stamp = Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    assert_eq!(stamp.head, "c2");
    assert_eq!(stamp.newest_build.as_str(), "1.2.1");
}

#[test]
fn rebuild_replaces_whole() {
    let (_dir, out) = scratch();
    let history = memory();
    let mut indexer = Indexer::new(&history, "memory");
    indexer.build(&out, Families::ALL).unwrap();
    let clips = Families::parse("clips").unwrap();
    let stamp = indexer.build(&out, clips).unwrap();
    assert_eq!(stamp.families, clips);
    assert_eq!(stamp.rows_of("files"), 0);
    let index = SqliteHistory::open(&out).unwrap();
    assert!(index.file(FileKey::parse(A).unwrap()).unwrap().is_none());
    assert!(index.clip(ClipKey(1)).unwrap().is_some());
    assert_eq!(index.events_banks(bank()).unwrap().len(), 2);
    assert!(index.clip_joints(WadHash(7)).unwrap().is_empty());
    assert!(!out.with_extension("tmp").exists());
}

#[test]
fn the_same_history_gives_the_same_file() {
    let (_dir, out) = scratch();
    let other = out.with_file_name("other");
    let history = memory();
    for dir in [&out, &other] {
        Indexer::new(&history, "memory")
            .build(dir, Families::ALL)
            .unwrap();
    }
    for file in ["history.sqlite", "builds.tsv", "stamp.json"] {
        let read = |dir: &Utf8Path| fs_err::read(dir.join(file)).unwrap();
        assert!(
            read(&out) == read(&other),
            "{file} differs between two builds"
        );
    }
}
