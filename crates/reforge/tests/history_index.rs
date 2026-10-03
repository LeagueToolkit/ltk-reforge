//! The index on disk, without its writer: the tables are created from their definitions, a few
//! rows are put in by hand, and `SqliteHistory` answers. What does not open is a typed error,
//! and to a run a diagnostic.

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::diag::{Diagnostics, Stage};
use reforge::game::Build;
use reforge::history::tables::{index_sql, schema_sql, table_names};
use reforge::history::{
    self, BuildRow, DB_FILE, FORMAT, Families, FileKind, History, HistoryAbsent, HistoryError,
    IndexMissing, IndexStale, InstallNewer, Joint, Life, Position, STAMP_FILE, SqliteHistory,
    Stamp,
};
use reforge::model::{ClipKey, FileKey, NormKey, ObjectKey, WadHash};
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
    assert_eq!(history.object(ObjectKey([7; 16])).unwrap(), None);
    assert!(history.objects_by_norm(NormKey(7)).unwrap().is_empty());
    assert_eq!(history.clip(ClipKey(7)).unwrap(), None);
    assert!(history.named_by(path).unwrap().is_empty());
    assert!(history.units_loading(path).unwrap().is_empty());
    assert!(history.event_wems(path, 1).unwrap().is_empty());
    assert!(history.events_playing(1).unwrap().is_empty());
    assert!(history.skeletons(path).unwrap().is_empty());
    assert!(history.meshes(path).unwrap().is_empty());
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
         INSERT INTO contents (path, key, first, last, alive) \
           VALUES (-1152921504606846975, x'07070707070707070707070707070707', 0, 1, 0), \
                  (3, x'07070707070707070707070707070707', 2, 2, 1);
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
    fs::write(
        dir.join("builds.tsv"),
        "0\tc0\t1.1.1\tm\td\n2\tc1\t1.2.1\tm\td\n",
    )
    .unwrap();
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
