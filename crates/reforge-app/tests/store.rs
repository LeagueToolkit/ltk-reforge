//! The app's store (#ADR-0096): the run tier's blobs by content id, and the rows.

use camino::{Utf8Path, Utf8PathBuf};
use fjall::{Database, KeyspaceCreateOptions};
use fs_err as fs;
use reforge::model::{ByteView, ContentId};
use reforge::store::BlobStore;
use reforge_app::store::{AppStore, FORMAT, KEYSPACE, ROWS_FILE, RunTier};
use rusqlite::Connection;
use tempfile::TempDir;

/// A folder for a store, removed with it.
struct Folder {
    _dir: TempDir,
    root: Utf8PathBuf,
}

impl Folder {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        Self { _dir: dir, root }
    }

    fn tier_dir(&self) -> Utf8PathBuf {
        self.root.join("run-tier")
    }

    fn tier(&self) -> RunTier {
        RunTier::open(self.tier_dir()).unwrap()
    }

    fn store(&self) -> AppStore {
        AppStore::open(&self.root).unwrap()
    }

    fn rows(&self) -> Connection {
        Connection::open(self.root.join(ROWS_FILE)).unwrap()
    }
}

/// The format the rows file says it is of.
fn format_of(rows: &Connection) -> String {
    let query = "SELECT value FROM meta WHERE key = 'format'";
    rows.query_row(query, [], |row| row.get(0)).unwrap()
}

/// The tables a rows file holds, by name.
fn tables_of(rows: &Connection) -> Vec<String> {
    let query = "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name";
    let mut statement = rows.prepare(query).unwrap();
    let names = statement.query_map([], |row| row.get(0)).unwrap();
    names.map(Result::unwrap).collect()
}

fn bytes(text: &str) -> ByteView {
    ByteView::from(text.as_bytes())
}

/// A blob past `fjall`'s separation threshold, so that it is kept in the value log.
fn large(byte: u8) -> ByteView {
    ByteView::from(vec![byte; 64 * 1024])
}

/// The bytes of the value log's files under `dir`: the files of each `blobs` folder, which is
/// where `fjall` 3.1 keeps a keyspace's value log.
fn log_bytes(dir: &Utf8Path) -> u64 {
    let mut bytes = 0;
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = Utf8PathBuf::from_path_buf(entry.path()).unwrap();
        if !path.is_dir() {
            continue;
        }
        if path.file_name() == Some("blobs") {
            let files = fs::read_dir(&path).unwrap().flatten();
            bytes += files
                .map(|file| file.metadata().unwrap().len())
                .sum::<u64>();
        } else {
            bytes += log_bytes(&path);
        }
    }
    bytes
}

#[test]
fn a_blob_put_reads_back_under_its_id() {
    let folder = Folder::new();
    let tier = folder.tier();

    let id = tier.put(bytes("a bin"));

    assert_eq!(id, ContentId::of(b"a bin"));
    assert!(tier.contains(id));
    assert_eq!(tier.get(id).as_deref(), Some(&b"a bin"[..]));
    assert_eq!(tier.get_unchecked(id).as_deref(), Some(&b"a bin"[..]));
    assert!(tier.on_disk());
}

#[test]
fn a_blob_put_reads_back_after_the_tier_opens_again() {
    let folder = Folder::new();
    let id = folder.tier().put(large(1));

    let tier = folder.tier();

    assert_eq!(tier.get(id), Some(large(1)));
}

#[test]
fn a_checked_get_of_bytes_that_fail_their_id_answers_nothing_and_removes_them() {
    let folder = Folder::new();
    let id = ContentId::of(b"the bytes");
    {
        let db = Database::builder(folder.tier_dir()).open().unwrap();
        let blobs = db
            .keyspace(KEYSPACE, KeyspaceCreateOptions::default)
            .unwrap();
        blobs.insert(id.0, &b"other bytes"[..]).unwrap();
    }
    let tier = folder.tier();
    assert_eq!(
        tier.get_unchecked(id).as_deref(),
        Some(&b"other bytes"[..]),
        "an unchecked read reads what is there"
    );

    assert_eq!(tier.get(id), None);

    assert!(!tier.contains(id));
    assert_eq!(tier.ids().unwrap(), []);
    assert_eq!(tier.put(bytes("the bytes")), id);
    assert_eq!(tier.get(id).as_deref(), Some(&b"the bytes"[..]));
}

#[test]
fn the_empty_blob_is_answered_and_never_kept() {
    let folder = Folder::new();
    let tier = folder.tier();

    assert_eq!(tier.get(ContentId::EMPTY).as_deref(), Some(&b""[..]));
    assert!(tier.contains(ContentId::EMPTY));
    assert_eq!(tier.put(bytes("")), ContentId::EMPTY);

    assert_eq!(tier.ids().unwrap(), []);
}

#[test]
fn the_ids_held_are_listed_in_order() {
    let folder = Folder::new();
    let tier = folder.tier();
    let mut put: Vec<ContentId> = ["c", "a", "b", "a"]
        .into_iter()
        .map(|text| tier.put(bytes(text)))
        .collect();

    let ids = tier.ids().unwrap();

    put.sort();
    put.dedup();
    assert_eq!(ids, put);
}

#[test]
fn a_blob_removed_is_gone() {
    let folder = Folder::new();
    let tier = folder.tier();
    let kept = tier.put(bytes("kept"));
    let removed = tier.put(bytes("removed"));

    tier.remove(removed).unwrap();

    assert!(!tier.contains(removed));
    assert_eq!(tier.get(removed), None);
    assert_eq!(tier.ids().unwrap(), [kept]);
    tier.remove(removed).unwrap();
}

#[test]
fn collecting_after_every_blob_is_removed_empties_the_value_log() {
    let folder = Folder::new();
    let tier = folder.tier();
    let ids: Vec<ContentId> = (0..16).map(|byte| tier.put(large(byte))).collect();
    tier.collect().unwrap();
    let full = log_bytes(&folder.tier_dir());
    assert!(full >= 16 * 64 * 1024, "{full} bytes in the value log");

    for id in ids {
        tier.remove(id).unwrap();
    }
    tier.collect().unwrap();
    // A log file dropped is deleted once no version of the tree names it.
    drop(tier);
    let tier = folder.tier();

    assert_eq!(log_bytes(&folder.tier_dir()), 0);
    assert_eq!(tier.ids().unwrap(), []);
}

#[test]
fn a_store_opened_twice_keeps_its_rows_file() {
    let folder = Folder::new();
    drop(folder.store());
    folder
        .rows()
        .execute("INSERT INTO meta (key, value) VALUES ('mark', 'kept')", [])
        .unwrap();

    drop(folder.store());

    let rows = folder.rows();
    assert_eq!(format_of(&rows), FORMAT.to_string());
    let mark: String = rows
        .query_row("SELECT value FROM meta WHERE key = 'mark'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(mark, "kept");
    assert_eq!(
        tables_of(&rows),
        ["load", "meta", "raw", "run", "run_blob", "sqlite_sequence"]
    );
    assert!(!folder.root.join(format!("{ROWS_FILE}.unread")).exists());
}

#[test]
fn a_rows_file_of_another_format_is_moved_aside_and_a_new_one_made() {
    let folder = Folder::new();
    let path = folder.root.join(ROWS_FILE);
    {
        let old = Connection::open(&path).unwrap();
        let made = "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO meta (key, value) VALUES ('format', '2');
            CREATE TABLE run (id INTEGER PRIMARY KEY);";
        old.execute_batch(made).unwrap();
    }

    drop(folder.store());

    let aside = Connection::open(folder.root.join(format!("{ROWS_FILE}.format-2"))).unwrap();
    assert_eq!(format_of(&aside), "2");
    assert_eq!(tables_of(&aside), ["meta", "run"]);
    let rows = folder.rows();
    assert_eq!(format_of(&rows), FORMAT.to_string());
    assert!(tables_of(&rows).contains(&"run_blob".to_string()));
}

#[test]
fn a_rows_file_whose_format_does_not_read_is_moved_aside() {
    let folder = Folder::new();
    fs::write(folder.root.join(ROWS_FILE), "no database").unwrap();

    drop(folder.store());

    let aside = folder.root.join(format!("{ROWS_FILE}.unread"));
    assert_eq!(fs::read_to_string(aside).unwrap(), "no database");
    assert_eq!(format_of(&folder.rows()), FORMAT.to_string());
}
