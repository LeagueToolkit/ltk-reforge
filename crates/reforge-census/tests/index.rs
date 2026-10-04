//! The index written from a history in memory and read back: every key found with its life,
//! absent keys absent, the stamp checked against a history's head and an install's build.

mod common;

use camino::{Utf8Path, Utf8PathBuf};
use reforge::diag::{Diagnostics, Stage};
use reforge::game::Build;
use reforge::history::{
    ClipJoints, Families, FileKind, History, IndexStale, InstallNewer, Joint, Life, NamedBy,
    PlayedBy, Position, SqliteHistory, UnitHit, WemPlayed,
};
use reforge::model::{BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash};
use reforge_census::Indexer;
use tempfile::TempDir;

use self::common::{A, B, HEADER, JOINTS, bank, build, entry, memory};

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
        "paths",
        "contents",
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
    assert_eq!(counts, [9, 6, 10, 1, 3, 2, 2, 3, 2, 1, 2, 2, 3, 2]);

    let index = SqliteHistory::open(&out).unwrap();
    assert_eq!(index.stamp(), Some(&stamp));
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
