//! The app's store (#ADR-0096): the run tier's blobs by content id, the rows, and the runs the
//! app keeps. The game is `reforge_testkit::skins`.

use camino::{Utf8Path, Utf8PathBuf};
use fjall::{Database, KeyspaceCreateOptions};
use fs_err as fs;
use reforge::archive::Caps;
use reforge::history::NoHistory;
use reforge::model::{ByteView, ContentId};
use reforge::store::BlobStore;
use reforge_app::inputs::Inputs;
use reforge_app::run::{
    self, RunKind, RunRequest, RunSettings, RunView, StageName, StageState, Unwatched,
};
use reforge_app::store::{AppStore, FORMAT, KEYSPACE, ROWS_FILE, RunTier};
use reforge_testkit::ModArchive;
use reforge_testkit::bins::{bin, entry};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, skin_bin};
use rusqlite::Connection;
use tempfile::TempDir;

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

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

/// A mod with its own rig at skin0.
fn own_mod() -> ModArchive {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
    ];
    let mut files = own_files("a");
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files)
}

/// A store, the inputs over the testkit's game, and a folder for archives and packages.
struct Bench {
    folder: Folder,
    inputs: Inputs,
    store: AppStore,
}

impl Bench {
    fn new() -> Self {
        let folder = Folder::new();
        let store = AppStore::open(&folder.root.join("store")).unwrap();
        Self {
            folder,
            inputs: Inputs::of_env(env(skins::game(), NoHistory)),
            store,
        }
    }

    /// A repair of `archive`, written as `name`, through `through`.
    fn request(&self, archive: &ModArchive, name: &str, through: StageName) -> RunRequest {
        let path = self.folder.root.join(name);
        fs::write(&path, archive.build()).unwrap();
        RunRequest {
            kind: RunKind::Repair,
            archive: path.to_string(),
            through,
            settings: RunSettings::default(),
            out: Some(self.folder.root.join("out").to_string()),
        }
    }

    fn run(&self, request: &RunRequest) -> RunView {
        let view = run::run(&self.inputs, request, Some(&self.store), &mut Unwatched).unwrap();
        assert_eq!(view.fault, None);
        view
    }

    fn rows(&self) -> Connection {
        Connection::open(self.folder.root.join("store").join(ROWS_FILE)).unwrap()
    }

    fn count(&self, table: &str) -> i64 {
        let query = format!("SELECT count(*) FROM {table}");
        self.rows().query_row(&query, [], |row| row.get(0)).unwrap()
    }

    /// The ids `run_blob` lists, each once, in order.
    fn named(&self) -> Vec<ContentId> {
        let rows = self.rows();
        let mut statement = rows
            .prepare("SELECT DISTINCT id FROM run_blob ORDER BY id")
            .unwrap();
        let ids = statement
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .unwrap();
        ids.map(|id| ContentId(id.unwrap().try_into().unwrap()))
            .collect()
    }
}

#[test]
fn a_port_through_write_keeps_a_row_of_each_kind_and_its_layer_in_the_run_tier() {
    let bench = Bench::new();
    // A port writes the bins it renames: bytes the run makes.
    let mut request = bench.request(&own_mod(), "mod.fantome", StageName::Write);
    request.kind = RunKind::Port;

    let view = bench.run(&request);

    assert!(view.written.is_some());
    assert_eq!(view.saved, Some(1));
    for table in ["raw", "load", "run"] {
        assert_eq!(bench.count(table), 1, "{table}");
    }
    let named = bench.named();
    assert!(!named.is_empty(), "the run made no bytes");
    assert_eq!(bench.store.tier().ids().unwrap(), named);
    for id in named {
        assert!(bench.store.tier().get(id).is_some(), "{id:?}");
    }
}

#[test]
fn a_second_run_of_the_same_archive_shares_its_raw_and_load_rows() {
    let bench = Bench::new();
    let request = bench.request(&own_mod(), "mod.fantome", StageName::Write);

    let first = bench.run(&request);
    let second = bench.run(&request);

    assert_eq!((first.saved, second.saved), (Some(1), Some(2)));
    assert_eq!(bench.count("raw"), 1);
    assert_eq!(bench.count("load"), 1);
    assert_eq!(bench.count("run"), 2);
}

#[test]
fn a_run_through_check_keeps_nothing() {
    let bench = Bench::new();

    let view = bench.run(&bench.request(&own_mod(), "mod.fantome", StageName::Check));

    assert_eq!(view.saved, None);
    for table in ["raw", "load", "run", "run_blob"] {
        assert_eq!(bench.count(table), 0, "{table}");
    }
    assert_eq!(bench.store.tier().ids().unwrap(), []);
}

#[test]
fn the_second_run_of_an_archive_starts_from_the_ingest_tiers_raw_snapshot() {
    let bench = Bench::new();
    let request = bench.request(&own_mod(), "mod.fantome", StageName::Write);
    let sha256 = ContentId::of(&fs::read(&request.archive).unwrap());

    bench.run(&request);
    let raw = bench.store.ingest().get(sha256, &Caps::default());
    assert!(raw.is_some(), "the first run left no raw snapshot");
    let second = bench.run(&request);

    let read = second.stages.iter().find(|s| s.stage == StageName::Read);
    assert_eq!(read.map(|stage| stage.state), Some(StageState::Ran));
    assert!(second.stages.iter().all(|s| s.state == StageState::Ran));
    let rows = bench.rows();
    let reports: Vec<Vec<u8>> = rows
        .prepare("SELECT report FROM run ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(reports[0], reports[1], "the reports of a read and a resume");
}

#[test]
fn the_runs_are_listed_newest_first_with_the_stages_each_reached() {
    let bench = Bench::new();
    let repair = bench.request(&own_mod(), "repaired.fantome", StageName::Write);
    let mut port = bench.request(&own_mod(), "ported.fantome", StageName::Write);
    port.kind = RunKind::Port;
    bench.run(&repair);
    bench.run(&port);

    let listed = bench.store.list().unwrap();

    let ids: Vec<u64> = listed.iter().map(|row| row.id).collect();
    assert_eq!(ids, [2, 1]);
    let (ported, repaired) = (&listed[0], &listed[1]);
    assert_eq!(
        (ported.kind, repaired.kind),
        (RunKind::Port, RunKind::Repair)
    );
    use StageName::*;
    assert_eq!(repaired.reached, [Load, Fix, Build, Check]);
    assert_eq!(
        ported.reached,
        [Load, Fix, Facts, Decide, Plan, Build, Check]
    );
    // One archive under two names: one raw row, named by the last read.
    assert_eq!(ported.file_name, "ported.fantome");
    assert_eq!(ported.sha256, repaired.sha256);
    assert_eq!(ported.sha256.len(), 64);
    assert!(!ported.kept);
    assert!(
        ported
            .package
            .as_deref()
            .is_some_and(|p| p.ends_with(".modpkg"))
    );
    let view = bench.store.view().unwrap();
    assert_eq!((view.runs, view.kept, view.mods), (2, 0, 1));
    assert!(view.rows_bytes > 0 && view.tier_bytes > 0 && view.ingest_bytes > 0);
}

#[test]
fn a_mods_text_reaches_the_listing_cleaned() {
    let bench = Bench::new();
    bench.run(&bench.request(&own_mod(), "mod.fantome", StageName::Write));
    // A right-to-left override in the name, an escape sequence in the author.
    bench
        .rows()
        .execute(
            "UPDATE raw SET name = ?1, author = ?2",
            [
                format!("Evil{}Name", '\u{202e}'),
                format!("a{}[31m", '\u{1b}'),
            ],
        )
        .unwrap();

    let listed = bench.store.list().unwrap();

    assert_eq!(listed[0].name.as_deref(), Some(r"Evil\u{202e}Name"));
    assert_eq!(listed[0].author.as_deref(), Some(r"a\u{1b}[31m"));
}

#[test]
fn a_kept_runs_report_reads_back_as_the_run_wrote_it() {
    let bench = Bench::new();
    let request = bench.request(&own_mod(), "mod.fantome", StageName::Write);
    let view = bench.run(&request);
    let id = view.saved.unwrap();

    let report = bench.store.report(id).unwrap().expect("the report");

    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(report.get("package").is_some(), "{report}");
    assert_eq!(bench.store.report(id + 1).unwrap(), None);
}
