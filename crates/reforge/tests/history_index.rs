//! The index on disk, without its writer: the tables are created from their definitions, a few
//! rows are put in by hand, and `SqliteHistory` answers. What does not open is a typed error,
//! and to a run a diagnostic.

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::diag::{Diagnostics, Stage};
use reforge::game::Build;
use reforge::history::objects::{Decoder, LEVEL};
use reforge::history::tables::{index_sql, schema_sql, table_names};
use reforge::history::{
    self, BuildDetails, BuildRow, ClipJoints, DB_FILE, FORMAT, Families, FileKind, History,
    HistoryAbsent, HistoryError, IndexMissing, IndexStale, InstallNewer, Joint, Life, MAX_REACHED,
    ObjectCodec, Placed, Position, STAMP_FILE, SqliteHistory, Stamp,
};
use reforge::model::{BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash};
use rusqlite::Connection;
use tempfile::TempDir;

fn build(version: &str) -> Build {
    Build::parse(version).unwrap()
}

fn rows(versions: &[&str]) -> Vec<BuildRow> {
    let rows = (0u16..).zip(versions).map(|(at, version)| BuildRow {
        at: Position(at),
        commit: format!("c{at}"),
        build: build(version),
        manifest: "0000000000000000".into(),
        date: "2026-01-01".into(),
        details: BuildDetails::default(),
    });
    rows.collect()
}

fn stamp(builds: &[BuildRow]) -> Stamp {
    Stamp {
        format: FORMAT,
        repo: "by hand".into(),
        head: builds.last().unwrap().commit.clone(),
        newest_build: build("1.3.1"),
        builds: builds.len(),
        families: Families::ALL,
        writer: "test".into(),
        rows: BTreeMap::new(),
        unparsed_texts: 0,
        objectless_texts: 0,
        objects: ObjectCodec::current(),
    }
}

/// An index of three builds whose tables hold what `fill` puts in.
fn index(fill: &str) -> (TempDir, Utf8PathBuf) {
    let scratch = TempDir::new().unwrap();
    let dir = Utf8Path::from_path(scratch.path()).unwrap().join("index");
    fs::create_dir_all(&dir).unwrap();
    let db = Connection::open(dir.join(DB_FILE)).unwrap();
    db.execute_batch(&schema_sql()).unwrap();
    db.execute_batch(fill).unwrap();
    db.execute_batch(&index_sql()).unwrap();
    let builds = rows(&["1.1.1", "1.2.1", "1.3.1"]);
    BuildRow::write_all(&dir, &builds).unwrap();
    stamp(&builds).write(&dir).unwrap();
    (scratch, dir)
}

#[test]
fn an_empty_index_answers_every_question_with_nothing() {
    let (_scratch, dir) = index("");
    let history = SqliteHistory::open(&dir).unwrap();
    assert_eq!(history.dir(), dir);
    assert_eq!(history.stamp().unwrap().head, "c2");
    assert_eq!(history.head(), Some(Position(2)));
    assert_eq!(history.version(Position(1)), Some(&build("1.2.1")));
    let (path, key) = (WadHash(1), FileKey([7; 16]));
    assert_eq!(history.file(key).unwrap(), None);
    assert_eq!(history.path(path).unwrap(), None);
    assert_eq!(history.held(path, key).unwrap(), None);
    assert!(history.paths_of(key).unwrap().is_empty());
    assert!(history.placed(path, Position(0)).unwrap().is_empty());
    assert_eq!(history.bin(path, Position(0)).unwrap(), None);
    let entry = BinHash(7);
    assert_eq!(history.text_blob(path, entry, Position(0)).unwrap(), None);
    assert_eq!(
        history.object_bytes(path, entry, Position(0)).unwrap(),
        None
    );
    assert!(history.links(path, entry, Position(0)).unwrap().is_empty());
    assert_eq!(
        history.reached(path, entry, Position(0)).unwrap(),
        Default::default()
    );
    assert!(
        history
            .field_types(entry, entry, Position(0))
            .unwrap()
            .is_empty()
    );
    assert!(history.object_lives(ObjectKey([7; 16])).unwrap().is_empty());
    assert_eq!(history.object(ObjectKey([7; 16])).unwrap(), None);
    assert!(history.objects_by_norm(NormKey(7)).unwrap().is_empty());
    assert_eq!(history.clip(ClipKey(7)).unwrap(), None);
    assert!(history.named_by(path).unwrap().is_empty());
    assert!(history.units_loading(path).unwrap().is_empty());
    assert!(history.event_wems(path, 1).unwrap().is_empty());
    assert!(history.events_playing(1).unwrap().is_empty());
    assert!(history.skeletons(path).unwrap().is_empty());
    assert!(history.meshes(path).unwrap().is_empty());
    assert!(history.clip_joints(path).unwrap().is_empty());
    assert_eq!(history.joints_of(key).unwrap(), None);
    assert!(history.events_banks(path).unwrap().is_empty());
}

#[test]
fn every_table_of_the_definition_is_in_the_file() {
    let (_scratch, dir) = index("");
    let db = Connection::open(dir.join(DB_FILE)).unwrap();
    let mut listed = db
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    let found: Vec<String> = listed
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut defined = table_names();
    defined.sort_unstable();
    assert_eq!(found, defined);
}

#[test]
fn rows_are_read_back_as_the_questions_spell_them() {
    // A hash above `i64::MAX` is stored as a negative integer.
    let high = WadHash(0xf000_0000_0000_0001);
    let (_scratch, dir) = index(
        "INSERT INTO files (key, kind, path, first, last, alive) \
           VALUES (x'07070707070707070707070707070707', 2, -1152921504606846975, 0, 2, 1);
         INSERT INTO wads (id, path) VALUES (1, 'data/b.wad.client'), (2, 'data/a.wad.client');
         INSERT INTO placements (path, wad, key, checksum, first, last, alive) \
           VALUES (-1152921504606846975, 1, x'07070707070707070707070707070707', 0, 0, 1, 0), \
                  (3, 1, x'07070707070707070707070707070707', 0, 2, 2, 1), \
                  (3, 2, x'07070707070707070707070707070707', 5, 2, 2, 1);
         INSERT INTO names (id, name) VALUES (1, 'Root'), (2, 'Hip');
         INSERT INTO skeletons (id, path, record, influences, first, last, alive) \
           VALUES (1, 4, 11, 4, 0, 1, 0), (2, 4, 12, 4, 2, 2, 1);
         INSERT INTO joints VALUES (1, 0, 1, NULL), (2, 0, 1, NULL), (2, 1, 2, 1);",
    );
    let history = SqliteHistory::open(&dir).unwrap();
    let key = FileKey([7; 16]);
    let file = history.file(key).unwrap().unwrap();
    assert_eq!(
        (file.life, file.path, file.kind),
        (Life::new(0, 2), high, FileKind::of("tex"))
    );
    assert_eq!(history.held(high, key).unwrap(), Some(Life::new(0, 1)));
    let paths: Vec<WadHash> = history
        .paths_of(key)
        .unwrap()
        .iter()
        .map(|at| at.path)
        .collect();
    assert_eq!(paths, [high, WadHash(3)]);
    let path = history.path(high).unwrap().unwrap();
    assert_eq!(
        (path.life, path.kind),
        (Life::new(0, 1), FileKind::of("tex"))
    );
    // Two WADs held the bytes at path 3 at the last build, listed in WAD path order.
    let placed = history.placed(WadHash(3), Position(2)).unwrap();
    let wads: Vec<(&str, u64)> = placed
        .iter()
        .map(|p| (p.wad.as_str(), p.checksum))
        .collect();
    assert_eq!(wads, [("data/a.wad.client", 5), ("data/b.wad.client", 0)]);
    assert!(history.placed(WadHash(3), Position(1)).unwrap().is_empty());

    let skeletons = history.skeletons(WadHash(4)).unwrap();
    assert_eq!(skeletons.len(), 2);
    assert_eq!(skeletons[0].life, Life::new(0, 1));
    assert_eq!(skeletons[0].skeleton.joints.len(), 1);
    assert!(history.reaches_head(skeletons[1].life));
    let hip = Joint {
        name: "Hip".into(),
        parent: Some("Root".into()),
    };
    assert_eq!(skeletons[1].skeleton.joints[1], hip);
    assert_eq!(skeletons[1].skeleton.joints[0].parent, None);
}

#[test]
fn an_object_under_two_classes_answers_the_life_that_reaches_furthest() {
    let (_scratch, dir) = index(
        "INSERT INTO objects (key, class, entry, bin, norm, first, last, alive) \
           VALUES (x'07070707070707070707070707070707', 2, 1, 5, 7, 0, 1, 0), \
                  (x'07070707070707070707070707070707', 3, 1, 6, 7, 1, 2, 1);",
    );
    let history = SqliteHistory::open(&dir).unwrap();
    let key = ObjectKey([7; 16]);
    let hit = history.object(key).unwrap().unwrap();
    assert_eq!((hit.class, hit.life), (BinHash(3), Life::new(1, 2)));
    let lives = history.object_lives(key).unwrap();
    let lives: Vec<(BinHash, WadHash, Life)> =
        lives.iter().map(|h| (h.class, h.bin, h.life)).collect();
    let expected = [
        (BinHash(2), WadHash(5), Life::new(0, 1)),
        (BinHash(3), WadHash(6), Life::new(1, 2)),
    ];
    assert_eq!(lives, expected);
    assert!(history.object_lives(ObjectKey([8; 16])).unwrap().is_empty());
}

#[test]
fn a_chain_of_links_longer_than_the_bound_is_cut_and_said_to_be() {
    // Objects 1 to 4,200 of bin 2, each linking the next.
    let (_scratch, dir) = index(
        "INSERT INTO wads (id, path) VALUES (1, 'data/x.wad.client');
         WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 4200)
           INSERT INTO entry_texts (bin, entry, wad, blob, first, last, alive)
           SELECT 2, i, 1, zeroblob(20), 0, 2, 1 FROM n;
         WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 4200)
           INSERT INTO links (bin, entry, linked, first, last, alive)
           SELECT 2, i, i + 1, 0, 2, 1 FROM n;",
    );
    let history = SqliteHistory::open(&dir).unwrap();
    let all = history
        .reached(WadHash(2), BinHash(1), Position(1))
        .unwrap();
    assert!(all.capped);
    assert_eq!(all.objects.len(), MAX_REACHED);
    let tail = history
        .reached(WadHash(2), BinHash(4191), Position(1))
        .unwrap();
    assert!(!tail.capped);
    assert_eq!(tail.objects.len(), 10);
    assert_eq!(tail.objects[0], (WadHash(2), BinHash(4191)));
}

#[test]
fn a_clips_joints_and_a_banks_objects_are_read_back_through_their_records() {
    // A record above `i64::MAX` is stored as a negative integer.
    let (_scratch, dir) = index(
        "INSERT INTO joint_lists (record, count, first, last, alive) VALUES (-5, 2, 0, 2, 2);
         INSERT INTO list_joints VALUES (-5, 0, 4026531841), (-5, 1, 7);
         INSERT INTO animations (path, key, joints, first, last, alive) \
           VALUES (4, x'07070707070707070707070707070707', -5, 0, 1, 0), \
                  (4, x'08080808080808080808080808080808', -5, 2, 2, 1), \
                  (5, x'07070707070707070707070707070707', -5, 0, 2, 1);
         INSERT INTO bank_objects (record, kind, id, body, first, last, alive) \
           VALUES (-2, 2, 4026531841, x'0a0b', 0, 2, 1), (3, 4, 225, x'', 0, 2, 1);
         INSERT INTO event_banks \
           (id, path, key, record, version, bank_id, header, objects, first, last, alive) \
           VALUES (1, 9, x'09090909090909090909090909090909', 11, 145, 5, x'9100000005000000', \
                   x'0300000000000000feffffffffffffff', 0, 2, 1), \
                  (2, 10, x'0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a', 12, 145, 5, \
                   x'9100000005000000', x'0900000000000000', 0, 2, 1);",
    );
    let history = SqliteHistory::open(&dir).unwrap();
    let joints = vec![0xf000_0001, 7];
    let clip = |key, life| ClipJoints {
        life,
        key: FileKey([key; 16]),
        joints: joints.clone(),
    };
    let expected = [clip(7, Life::new(0, 1)), clip(8, Life::new(2, 2))];
    assert_eq!(history.clip_joints(WadHash(4)).unwrap(), expected);
    assert_eq!(history.joints_of(FileKey([7; 16])).unwrap(), Some(joints));
    assert_eq!(history.joints_of(FileKey([9; 16])).unwrap(), None);

    let banks = history.events_banks(WadHash(9)).unwrap();
    assert_eq!(banks.len(), 1);
    let bank = &banks[0].bank;
    assert_eq!(banks[0].life, Life::new(0, 2));
    assert_eq!((bank.version(), bank.bank_id()), (145, 5));
    let objects: Vec<(u8, u32, &[u8])> = bank
        .objects
        .iter()
        .map(|o| (o.kind, o.id, o.body.as_slice()))
        .collect();
    // In the order the bank lists them, not the order of the records.
    let expected: [(u8, u32, &[u8]); 2] = [(4, 225, &[]), (2, 0xf000_0001, &[0x0a, 0x0b])];
    assert_eq!(objects, expected);

    // A bank that lists an object the index lacks is an error, not a shorter bank.
    let lacking = history.events_banks(WadHash(10));
    assert!(matches!(
        lacking,
        Err(HistoryError::BankObject {
            path: WadHash(10),
            record: 9
        })
    ));
}

// #ADR-0081: every locale's WAD holds its own voice bank at one path.
#[test]
fn each_events_bank_at_a_path_names_the_wads_that_held_its_file() {
    const FR: &str = "data/final/champions/test.fr_fr.wad.client";
    const FR_TOO: &str = "data/final/champions/test2.fr_fr.wad.client";
    const EN: &str = "data/final/champions/test.en_us.wad.client";
    // French bytes throughout, in two WADs; English bytes that change at the second build.
    let (_scratch, dir) = index(&format!(
        "INSERT INTO wads (id, path) VALUES (1, '{FR}'), (2, '{EN}'), (3, '{FR_TOO}');
         INSERT INTO placements (path, wad, key, checksum, first, last, alive) \
           VALUES (9, 1, x'0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f', 0, 0, 2, 1), \
                  (9, 3, x'0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f', 0, 1, 2, 1), \
                  (9, 2, x'0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e', 0, 0, 0, 0), \
                  (9, 2, x'0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d', 0, 1, 2, 1);
         INSERT INTO bank_objects (record, kind, id, body, first, last, alive) \
           VALUES (3, 4, 225, x'', 0, 2, 3);
         INSERT INTO event_banks \
           (id, path, key, record, version, bank_id, header, objects, first, last, alive) \
           VALUES (1, 9, x'0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f', 11, 145, 5, x'9100000005000000', \
                   x'0300000000000000', 0, 2, 2), \
                  (2, 9, x'0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e', 12, 145, 6, x'9100000006000000', \
                   x'0300000000000000', 0, 0, 0), \
                  (3, 9, x'0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d', 13, 145, 7, x'9100000007000000', \
                   x'0300000000000000', 1, 2, 1);"
    ));
    let history = SqliteHistory::open(&dir).unwrap();
    let banks = history.events_banks(WadHash(9)).unwrap();
    // A bank's id, its file, and each WAD that held the file with its builds.
    type Held<'a> = (u32, FileKey, Vec<(&'a str, Life)>);
    let held: Vec<Held> = banks
        .iter()
        .map(|hit| {
            let wads = hit.placed.iter().map(|p| (p.wad.as_str(), p.life));
            (hit.bank.bank_id(), hit.key, wads.collect())
        })
        .collect();
    let expected = [
        (
            5,
            FileKey([0x0f; 16]),
            vec![(FR, Life::new(0, 2)), (FR_TOO, Life::new(1, 2))],
        ),
        (6, FileKey([0x0e; 16]), vec![(EN, Life::new(0, 0))]),
        (7, FileKey([0x0d; 16]), vec![(EN, Life::new(1, 2))]),
    ];
    assert_eq!(held, expected);

    // A reader of one locale's WAD takes the bank that WAD held at its build.
    let in_wad = |wad: &str, at: u16| {
        let holds = |p: &Placed| p.wad == wad && p.life.holds(Position(at));
        let hit = banks.iter().find(|hit| hit.placed.iter().any(&holds));
        hit.map(|hit| hit.bank.bank_id())
    };
    assert_eq!(in_wad(EN, 0), Some(6));
    assert_eq!(in_wad(EN, 2), Some(7));
    assert_eq!(in_wad(FR, 2), Some(5));
}

#[test]
fn an_index_of_another_format_is_refused_for_its_format() {
    let (_scratch, dir) = index("");
    let other = r#"{"format": 8, "repo": "elsewhere", "forge_version": "0.1.0"}"#;
    fs::write(dir.join(STAMP_FILE), other).unwrap();
    assert!(matches!(
        SqliteHistory::open(&dir),
        Err(HistoryError::Format { found: 8, .. })
    ));
}

#[test]
fn an_index_stamped_for_other_builds_than_it_lists_is_refused() {
    let (_scratch, dir) = index("");
    BuildRow::write_all(&dir, &rows(&["1.1.1", "1.2.1"])).unwrap();
    assert!(matches!(
        SqliteHistory::open(&dir),
        Err(HistoryError::BuildCount {
            stamped: 3,
            listed: 2,
            ..
        })
    ));
    // The second line is numbered for another position.
    let mut out_of_place = rows(&["1.1.1", "1.2.1"]);
    out_of_place[1].at = Position(2);
    let text: String = out_of_place.iter().map(BuildRow::line).collect();
    fs::write(dir.join("builds.tsv"), text).unwrap();
    assert!(matches!(
        SqliteHistory::open(&dir),
        Err(HistoryError::Builds { line: 2, .. })
    ));
}

#[test]
fn a_run_with_no_index_is_told_so_and_asks_no_history() {
    let mut log = Diagnostics::new();
    let history = history::open(None, &mut log.at(Stage::Read));
    assert!(history.stamp().is_none());
    assert_eq!(log.of::<HistoryAbsent>().count(), 1);
    assert_eq!(log.len(), 1);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// An object's bytes from its entry hash on: entry 7 with `fields` u32 fields, each its index.
fn object(fields: u16) -> Vec<u8> {
    let mut bytes = 7u32.to_le_bytes().to_vec();
    bytes.extend(fields.to_le_bytes());
    for field in 0..u32::from(fields) {
        bytes.extend(field.to_le_bytes());
        bytes.push(7);
        bytes.extend(field.to_le_bytes());
    }
    bytes
}

#[test]
fn a_delta_decodes_against_its_keyframes_bytes() {
    let keyframe: Vec<u8> = (0..4096u32).flat_map(|n| (n % 251).to_le_bytes()).collect();
    let mut version = keyframe.clone();
    version[100] ^= 1;
    let frame = zstd::bulk::compress(&keyframe, LEVEL).unwrap();
    let mut c = zstd::bulk::Compressor::with_dictionary(LEVEL, &keyframe).unwrap();
    let delta = c.compress(&version).unwrap();
    assert!(
        delta.len() < 64,
        "a one-byte change takes {} bytes",
        delta.len()
    );
    let mut decoder = Decoder::default();
    let base = decoder.keyframe(None, &frame).unwrap();
    assert_eq!(base, keyframe);
    assert_eq!(decoder.delta(&base, &delta).unwrap(), version);
}

#[test]
fn an_objects_bytes_at_a_build_decode_from_its_keyframe_and_its_own_frame() {
    let (dict, keyframe, version) = (object(40), object(64), object(65));
    let mut with_dict = zstd::bulk::Compressor::with_dictionary(19, &dict).unwrap();
    let mut with_keyframe = zstd::bulk::Compressor::with_dictionary(19, &keyframe).unwrap();
    let (key_frame, delta) = (
        with_dict.compress(&keyframe).unwrap(),
        with_keyframe.compress(&version).unwrap(),
    );
    // Entry 7 of the bin at path 3: the keyframe at build 0, the version after it at build 1,
    // and at build 2 a text that held no object. Row 3 names a keyframe the index lacks.
    let (_scratch, dir) = index(&format!(
        "INSERT INTO wads (id, path) VALUES (1, 'data/a.wad.client');
         INSERT INTO entry_texts (entry, bin, wad, blob, first, last, alive) \
           VALUES (7, 3, 1, x'{a}', 0, 0, 0), (7, 3, 1, x'{b}', 1, 1, 0), \
                  (7, 3, 1, x'{c}', 2, 2, 1), (8, 3, 1, x'{d}', 0, 2, 1);
         INSERT INTO object_dicts (id, class, dict) VALUES (1, 2, x'{dict}');
         INSERT INTO object_bytes (id, keyframe, dict, frame) \
           VALUES (1, 1, 1, x'{key_frame}'), (2, 1, NULL, x'{delta}'), (3, 9, NULL, x'{delta}');
         INSERT INTO text_bytes (blob, class, object) \
           VALUES (x'{a}', 2, 1), (x'{b}', 2, 2), (x'{d}', 2, 3);",
        a = hex(&[0xa; 20]),
        b = hex(&[0xb; 20]),
        c = hex(&[0xc; 20]),
        d = hex(&[0xd; 20]),
        dict = hex(&dict),
        key_frame = hex(&key_frame),
        delta = hex(&delta),
    ));
    let history = SqliteHistory::open(&dir).unwrap();
    let bytes_at = |entry: u32, at: u16| {
        let found = history.object_bytes(WadHash(3), BinHash(entry), Position(at));
        found.unwrap().map(|object| (object.class, object.bytes))
    };
    assert_eq!(bytes_at(7, 0), Some((BinHash(2), keyframe.clone())));
    assert_eq!(bytes_at(7, 1), Some((BinHash(2), version.clone())));
    assert_eq!(bytes_at(7, 2), None);
    let at_one = history.object_bytes(WadHash(3), BinHash(7), Position(1));
    let at_one = at_one.unwrap().unwrap();
    assert_eq!(
        (at_one.wad.as_str(), at_one.life),
        ("data/a.wad.client", Life::new(1, 1))
    );
    let stored = history.stored_object(&[0xb; 20]).unwrap().unwrap();
    assert_eq!((stored.row, stored.keyframe), (2, Some(keyframe)));
    assert_eq!(history.stored_object(&[0xc; 20]).unwrap(), None);
    let broken = history.object_bytes(WadHash(3), BinHash(8), Position(0));
    assert!(
        matches!(broken, Err(HistoryError::ObjectRow { object: 3, .. })),
        "{broken:?}"
    );
}

#[test]
fn an_index_that_does_not_open_is_a_diagnostic_and_no_history() {
    let scratch = TempDir::new().unwrap();
    let dir = Utf8Path::from_path(scratch.path()).unwrap().join("absent");
    let mut log = Diagnostics::new();
    let history = history::open(Some(&dir), &mut log.at(Stage::Read));
    assert!(history.stamp().is_none());
    let missing: Vec<&IndexMissing> = log.of::<IndexMissing>().collect();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].index, dir.as_str());
    assert!(
        missing[0].reason.contains("stamp.json"),
        "{}",
        missing[0].reason
    );
    assert_eq!(log.of::<HistoryAbsent>().count(), 0);
}

#[test]
fn an_index_that_opens_raises_nothing() {
    let (_scratch, dir) = index("");
    let mut log = Diagnostics::new();
    let history = history::open(Some(&dir), &mut log.at(Stage::Read));
    assert_eq!(history.builds().len(), 3);
    assert!(log.is_empty());
}

#[test]
fn a_stamp_is_checked_against_the_historys_head_and_the_install() {
    let stamp = stamp(&rows(&["1.1.1", "1.2.1", "1.3.1"]));
    let mut log = Diagnostics::new();
    stamp.check(Some("c2"), Some(&build("1.3.1")), &mut log.at(Stage::Read));
    stamp.check(None, None, &mut log.at(Stage::Read));
    // An install older than the newest build, by its version and not by its last number.
    stamp.check(None, Some(&build("1.2.9")), &mut log.at(Stage::Read));
    assert!(log.is_empty());

    stamp.check(Some("c3"), Some(&build("1.4.0")), &mut log.at(Stage::Read));
    let stale: Vec<&IndexStale> = log.of::<IndexStale>().collect();
    let expected = IndexStale {
        index_head: "c2".into(),
        repo_head: "c3".into(),
    };
    assert_eq!(stale, [&expected]);
    let newer: Vec<&InstallNewer> = log.of::<InstallNewer>().collect();
    let expected = InstallNewer {
        install: build("1.4.0"),
        newest: build("1.3.1"),
    };
    assert_eq!(newer, [&expected]);
    assert_eq!(log.len(), 2);
}
