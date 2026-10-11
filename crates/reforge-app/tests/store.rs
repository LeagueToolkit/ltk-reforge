//! The app's store (#ADR-0096): the run tier's blobs by content id, the rows, and the runs the
//! app keeps. The game is `reforge_testkit::skins`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use fjall::{Database, KeyspaceCreateOptions};
use fs_err as fs;
use reforge::archive::Caps;
use reforge::diag::Stage;
use reforge::fixups::{FixupSelection, FixupSet};
use reforge::history::NoHistory;
use reforge::model::{ByteView, ContentId, Head};
use reforge::snapshot::{self, BuildSnapshot, CheckSnapshot, LoadSnapshot, StageSnapshot};
use reforge::store::BlobStore;
use reforge::{Porter, Start};
use reforge_app::error::AppError;
use reforge_app::inputs::Inputs;
use reforge_app::run::{
    self, PartView, RunKind, RunRequest, RunSettings, RunView, StageEvent, StageName, StageState,
    Unwatched, Watch,
};
use reforge_app::store::{
    AppStore, FORMAT, FileView, KEYSPACE, Origin, Outdated, ROWS_FILE, RunTier, StoreOpen,
};
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

/// What a store that opens outdated says a reset loses.
fn outdated(root: &Utf8Path) -> Outdated {
    match AppStore::open(root) {
        Err(StoreOpen::Outdated(outdated)) => outdated,
        Err(StoreOpen::Failed(error)) => panic!("the store failed: {error}"),
        Ok(_) => panic!("the store at {root} opened"),
    }
}

#[test]
fn a_rows_file_of_format_1_with_the_old_run_table_opens_outdated_with_its_runs() {
    let folder = Folder::new();
    let path = folder.root.join(ROWS_FILE);
    {
        let old = Connection::open(&path).unwrap();
        let made = "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO meta (key, value) VALUES ('format', '1');
            CREATE TABLE run (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              kind TEXT NOT NULL,
              kept INTEGER NOT NULL DEFAULT 0,
              report BLOB NOT NULL
            );
            INSERT INTO run (kind, kept, report) VALUES ('port', 1, x'00'), ('repair', 0, x'00');";
        old.execute_batch(made).unwrap();
    }

    let outdated = outdated(&folder.root);

    assert_eq!((outdated.format, outdated.runs, outdated.kept), (1, 2, 1));
    assert_eq!(outdated.rows_bytes, fs::metadata(&path).unwrap().len());
    let rows = folder.rows();
    assert_eq!(format_of(&rows), "1", "the file is left as it is");
    assert_eq!(tables_of(&rows), ["meta", "run", "sqlite_sequence"]);
}

#[test]
fn a_rows_file_of_this_format_with_a_column_dropped_opens_outdated() {
    let folder = Folder::new();
    drop(folder.store());
    folder
        .rows()
        .execute_batch("ALTER TABLE run DROP COLUMN view")
        .unwrap();

    let outdated = outdated(&folder.root);

    assert_eq!((outdated.format, outdated.runs), (FORMAT, 0));
}

#[test]
fn a_rows_file_that_is_no_database_opens_outdated_and_stays() {
    let folder = Folder::new();
    fs::write(folder.root.join(ROWS_FILE), "no database").unwrap();

    let outdated = outdated(&folder.root);

    assert_eq!((outdated.format, outdated.runs, outdated.kept), (0, 0, 0));
    let kept = fs::read_to_string(folder.root.join(ROWS_FILE)).unwrap();
    assert_eq!(kept, "no database");
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

    /// A bench whose store keeps `bound` runs not marked kept.
    fn bounded(bound: usize) -> Self {
        let bench = Self::new();
        Self {
            store: bench.store.with_bound(bound),
            ..bench
        }
    }

    /// A port of `archive` through write, written as `name`: a run that makes bytes, since a
    /// port writes the bins it renames.
    fn port(&self, archive: &ModArchive, name: &str) -> RunView {
        let mut request = self.request(archive, name, StageName::Write);
        request.kind = RunKind::Port;
        self.run(&request)
    }

    fn ids(&self) -> Vec<u64> {
        self.store
            .list()
            .unwrap()
            .iter()
            .map(|row| row.id)
            .collect()
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
        let view = run::run(
            &self.inputs,
            request,
            Some(&self.store),
            &mut Unwatched,
            None,
        )
        .unwrap();
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

    /// The ids `run_blob` lists that are the run tier's: an id the ingest tier holds is
    /// absent from it.
    fn named_in_tier(&self) -> Vec<ContentId> {
        let tier = self.store.tier();
        self.named()
            .into_iter()
            .filter(|&id| tier.contains(id))
            .collect()
    }

    /// The snapshot in a packed column of a row, read back.
    fn snapshot<S: StageSnapshot>(&self, query: &str) -> S {
        let packed: Vec<u8> = self.rows().query_row(query, [], |row| row.get(0)).unwrap();
        let json = String::from_utf8(zstd::decode_all(&packed[..]).unwrap()).unwrap();
        snapshot::from_json(&json).unwrap()
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
    let named = bench.named_in_tier();
    assert!(!named.is_empty(), "the run made no bytes");
    assert_eq!(bench.store.tier().ids().unwrap(), named);
    for id in named {
        assert!(bench.store.tier().get(id).is_some(), "{id:?}");
    }
}

#[test]
fn a_kept_run_names_every_id_its_snapshots_name() {
    let bench = Bench::new();
    bench.port(&own_mod(), "mod.fantome");

    let load: LoadSnapshot = bench.snapshot("SELECT json FROM load");
    let build: BuildSnapshot = bench.snapshot("SELECT build FROM run");
    let check: CheckSnapshot = bench.snapshot("SELECT \"check\" FROM run");

    let named = bench.named();
    let ids: Vec<ContentId> = load.ids().chain(build.ids()).chain(check.ids()).collect();
    assert!(build.ids().count() > 0 && check.ids().count() > 0);
    for id in ids {
        assert!(named.contains(&id), "{id:?} is not named");
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
    let first = bench.store.report(1).unwrap();
    // With the first run kept, the second resumes from its snapshots in place of the raw one.
    bench.store.remove_run(1).unwrap();
    let second = bench.run(&request);

    let read = second.stages.iter().find(|s| s.stage == StageName::Read);
    assert_eq!(read.map(|stage| stage.state), Some(StageState::Ran));
    assert!(second.stages.iter().all(|s| s.state == StageState::Ran));
    assert!(second.stages.iter().all(|s| !s.resumed));
    let report = bench.store.report(second.saved.unwrap()).unwrap();
    assert_eq!(first, report, "the reports of a read and a resume");
}

/// Which stages of a run's view the run took from a kept run's snapshots.
fn resumed(view: &RunView) -> Vec<StageName> {
    let stages = view.stages.iter().filter(|stage| stage.resumed);
    stages.map(|stage| stage.stage).collect()
}

/// The first fixup on bin values of the standard set, which `fix` declares.
fn a_bin_fixup() -> String {
    let set = FixupSet::from(&FixupSelection::standard());
    set.bin_ids()[0].as_str().to_string()
}

#[test]
fn a_rerun_resumes_from_the_latest_stage_its_settings_leave_as_the_kept_run_had_them() {
    use StageName::*;
    let bench = Bench::new();
    let first = bench.port(&own_mod(), "mod.fantome");
    assert_eq!(resumed(&first), []);

    let second = bench.port(&own_mod(), "mod.fantome");

    assert!(second.stages.iter().all(|s| s.state == StageState::Ran));
    assert_eq!(
        resumed(&second),
        [Load, Fix, Facts, Decide, Plan, Build, Check]
    );
    let reports = |id: u64| bench.store.report(id).unwrap().expect("the report");
    assert_eq!(
        reports(2),
        reports(1),
        "the reports of a run and its resume"
    );

    let mut request = bench.request(&own_mod(), "mod.fantome", Write);
    request.kind = RunKind::Port;
    request.settings.fixups.off = vec![a_bin_fixup()];
    let third = bench.run(&request);

    assert_eq!(resumed(&third), [Load], "the fix settings differ");
    let straight = Bench::new();
    let from_archive = straight.run(&request);
    assert_eq!(resumed(&from_archive), []);
    let straight_report = straight.store.report(1).unwrap();
    assert_eq!(reports(3), straight_report.expect("the report"));
}

/// The run tier, counting the reads of a blob or its head.
struct Counted {
    tier: RunTier,
    reads: Arc<AtomicUsize>,
}

impl BlobStore for Counted {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.tier.get(id)
    }

    fn get_unchecked(&self, id: ContentId) -> Option<ByteView> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.tier.get_unchecked(id)
    }

    fn get_head(&self, id: ContentId, len: usize) -> Option<Head> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.tier.get_head(id, len)
    }

    fn contains(&self, id: ContentId) -> bool {
        self.tier.contains(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        self.tier.put(bytes)
    }
}

#[test]
fn a_rerun_with_a_fixup_off_reads_no_blob_before_it_resumes_from_the_load() {
    let bench = Bench::new();
    bench.port(&own_mod(), "mod.fantome");
    let key: String = bench
        .rows()
        .query_row("SELECT key FROM load", [], |row| row.get(0))
        .unwrap();
    let kept = bench.store.newest_kept_run_of(&key, RunKind::Port);
    let kept = kept.unwrap().expect("the kept port");
    let selection = FixupSelection::standard().without(a_bin_fixup().as_str());
    let porter = Porter::new(bench.inputs.env()).fixups(FixupSet::from(&selection));
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = Counted {
        tier: bench.store.tier().clone(),
        reads: Arc::clone(&reads),
    };
    let run = porter
        .run()
        .in_store(counted)
        .in_cache(bench.store.ingest().clone());

    let stage = run::kept_start(&run, &kept, StageName::Write);

    assert_eq!(stage, Some(Stage::Load), "the fix settings differ");
    assert_eq!(
        reads.load(Ordering::Relaxed),
        0,
        "the reads before the resume"
    );
    let start = Start::Load(Box::new(kept.snapshots.load));
    run.resume(start).expect("the run resumes");
    assert!(
        reads.load(Ordering::Relaxed) > 0,
        "the resume reads the bins"
    );
}

#[test]
fn the_newest_kept_run_of_a_load_is_read_by_its_kind() {
    let bench = Bench::new();
    bench.port(&own_mod(), "mod.fantome");
    bench.port(&own_mod(), "mod.fantome");
    bench.run(&bench.request(&own_mod(), "mod.fantome", StageName::Write));
    // A repair's load is of its own settings: its run is put under the ports' load.
    let rows = bench.rows();
    let query = "SELECT load.id, load.key FROM load JOIN run ON run.load_id = load.id
         WHERE run.id = 1";
    let (load, key): (i64, String) = rows
        .query_row(query, [], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap();
    rows.execute("UPDATE run SET load_id = ?1 WHERE id = 3", [load])
        .unwrap();

    let port = bench.store.newest_kept_run_of(&key, RunKind::Port).unwrap();
    let repair = bench.store.newest_kept_run_of(&key, RunKind::Repair);

    let port = port.expect("the newest port");
    assert_eq!((port.id, port.kind), (2, RunKind::Port));
    assert!(port.snapshots.facts.is_some());
    let repair = repair.unwrap().expect("the newest repair");
    assert_eq!((repair.id, repair.kind), (3, RunKind::Repair));
    assert!(repair.snapshots.facts.is_none());
    let other = bench
        .store
        .newest_kept_run_of("another load", RunKind::Port);
    assert!(other.unwrap().is_none());
}

/// What a watch heard of a run: each stage's events, and how many parts.
#[derive(Default)]
struct Heard {
    stages: Vec<(StageName, &'static str)>,
    parts: usize,
}

impl Watch for Heard {
    fn stage(&mut self, stage: StageName, event: StageEvent) {
        let event = match event {
            StageEvent::Started => "started",
            StageEvent::Ended { .. } => "ended",
            StageEvent::Resumed => "resumed",
        };
        self.stages.push((stage, event));
    }

    fn takes_parts(&self) -> bool {
        true
    }

    fn part(&mut self, _part: PartView) {
        self.parts += 1;
    }
}

#[test]
fn a_watch_hears_the_stages_a_rerun_takes_from_a_kept_run_with_their_parts() {
    use StageName::*;
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), "mod.fantome", Write);
    request.kind = RunKind::Port;
    let mut first = Heard::default();
    run::run(
        &bench.inputs,
        &request,
        Some(&bench.store),
        &mut first,
        None,
    )
    .unwrap();

    let mut heard = Heard::default();
    run::run(
        &bench.inputs,
        &request,
        Some(&bench.store),
        &mut heard,
        None,
    )
    .unwrap();

    let resumed = [Load, Fix, Facts, Decide, Plan, Build, Check].map(|stage| (stage, "resumed"));
    let mut expected = vec![(Read, "started"), (Read, "ended")];
    expected.extend(resumed);
    expected.extend([(Write, "started"), (Write, "ended")]);
    assert_eq!(heard.stages, expected);
    assert_eq!(
        heard.parts, first.parts,
        "the parts of a run and its resume"
    );
}

#[test]
fn a_kept_run_gives_its_archive_and_settings_to_run_again() {
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), "mod.fantome", StageName::Write);
    request.kind = RunKind::Port;
    request.settings.fixups.off = vec![a_bin_fixup()];
    let id = bench.run(&request).saved.unwrap();

    let again = bench.store.run_again(id).unwrap().expect("the run");

    assert_eq!(again.kind, RunKind::Port);
    assert_eq!(again.archive, request.archive);
    assert_eq!(again.settings, request.settings);
    assert!(again.archive_present);
    assert_eq!(bench.store.run_again(id + 1).unwrap(), None);

    fs::remove_file(&request.archive).unwrap();

    let again = bench.store.run_again(id).unwrap().expect("the run");
    assert!(
        !again.archive_present,
        "the archive is no longer at its path"
    );
}

#[test]
fn clearing_a_mods_archive_data_keeps_its_runs_and_marks_them_not_ingested() {
    let bench = Bench::new();
    let view = bench.port(&own_mod(), "mod.fantome");
    let row = bench.store.list().unwrap().remove(0);
    assert!(row.archive_ingested);
    let sha256 = ContentId::parse(&row.sha256).unwrap();
    assert!(bench.store.mod_ingest_bytes(sha256) > 0);

    let cleared = bench.store.clear_ingest(sha256).unwrap();

    assert_eq!(cleared.entries, 1);
    assert!(cleared.blobs > 0);
    for table in ["raw", "load", "run"] {
        assert_eq!(bench.count(table), 1, "{table}");
    }
    let row = bench.store.list().unwrap().remove(0);
    assert!(!row.archive_ingested);
    assert_eq!(bench.store.mod_ingest_bytes(sha256), 0);
    assert_eq!(
        bench.store.run_view(row.id).unwrap().map(|v| v.saved),
        Some(view.saved)
    );

    // Run again reads the archive, which ingests it again.
    let again = bench.port(&own_mod(), "mod.fantome");
    assert_eq!(resumed(&again), []);
    let rows = bench.store.list().unwrap();
    assert!(rows.iter().all(|row| row.archive_ingested));
}

#[test]
fn clearing_the_unused_archive_data_keeps_the_mods_with_runs() {
    let bench = Bench::new();
    let other = own_mod().extra("README.txt", "another archive");
    bench.port(&own_mod(), "mod.fantome");
    let unkept = bench.run(&bench.request(&other, "other.fantome", StageName::Check));
    assert_eq!(unkept.saved, None);
    let kept = ContentId::parse(&bench.store.list().unwrap()[0].sha256).unwrap();
    let other_sha256 = ContentId::of(&other.build());
    let unused = bench.store.view().unwrap().ingest_unused_bytes;
    assert_eq!(unused, bench.store.mod_ingest_bytes(other_sha256));
    assert!(unused > 0);

    let cleared = bench.store.clear_unused_ingest().unwrap();

    assert_eq!(cleared.entries, 1);
    assert_eq!(bench.store.view().unwrap().ingest_unused_bytes, 0);
    assert_eq!(bench.store.mod_ingest_bytes(other_sha256), 0);
    assert!(bench.store.mod_ingest_bytes(kept) > 0);
    assert!(bench.store.list().unwrap()[0].archive_ingested);
}

#[test]
fn the_store_view_counts_the_bytes_of_the_entries_no_run_names_alone() {
    let bench = Bench::new();
    let other = own_mod().extra("README.txt", "another archive");
    bench.port(&own_mod(), "mod.fantome");
    bench.run(&bench.request(&other, "other.fantome", StageName::Check));
    let named = ContentId::parse(&bench.store.list().unwrap()[0].sha256).unwrap();
    let entries = bench.store.ingest().entries();
    assert_eq!(entries.len(), 2);
    let bytes_of = |sha256: ContentId| {
        let entry = entries.iter().find(|entry| entry.sha256 == sha256).unwrap();
        bench.store.ingest().entry_bytes(entry)
    };
    let (named_bytes, unnamed_bytes) = (bytes_of(named), bytes_of(ContentId::of(&other.build())));
    assert!(named_bytes > 0 && unnamed_bytes > 0);

    let view = bench.store.view().unwrap();

    assert_eq!(view.ingest_unused_bytes, unnamed_bytes);
    assert_eq!(bench.store.mod_ingest_bytes(named), named_bytes);
}

#[test]
fn a_rerun_through_a_stage_resumes_no_later_than_it() {
    use StageName::*;
    let bench = Bench::new();
    bench.port(&own_mod(), "mod.fantome");
    let mut request = bench.request(&own_mod(), "mod.fantome", Fix);
    request.kind = RunKind::Port;

    let view = bench.run(&request);

    assert_eq!(resumed(&view), [Load, Fix]);
    let reached: Vec<StageName> = view
        .stages
        .iter()
        .filter(|s| s.state == StageState::Ran)
        .map(|s| s.stage)
        .collect();
    assert_eq!(reached, [Read, Load, Fix]);
}

#[test]
fn a_repair_after_a_port_of_the_same_archive_resumes_from_nothing_of_the_ports() {
    let bench = Bench::new();
    bench.port(&own_mod(), "mod.fantome");

    let repair = bench.run(&bench.request(&own_mod(), "mod.fantome", StageName::Write));

    assert_eq!(resumed(&repair), []);
    assert!(repair.stages.iter().all(|s| s.state == StageState::Ran));
}

#[test]
fn a_run_resumed_from_a_kept_run_keeps_its_bytes_when_the_kept_run_is_removed() {
    let bench = Bench::new();
    bench.port(&own_mod(), "mod.fantome");
    let resumed_run = bench.port(&own_mod(), "mod.fantome");
    assert!(!resumed(&resumed_run).is_empty());
    let tier = bench.store.tier().ids().unwrap();
    assert!(!tier.is_empty(), "the first run made no bytes");

    bench.store.remove_run(1).unwrap();

    assert_eq!(bench.ids(), [2]);
    assert_eq!(bench.store.tier().ids().unwrap(), tier);
    for id in tier {
        let shown = bench.store.file_view(2, id, None);
        assert!(shown.is_ok(), "{id:?}: {shown:?}");
    }
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
    assert_eq!(
        (ported.origin, ported.opened_from.as_deref()),
        (Origin::Run, None)
    );
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

#[test]
fn a_kept_runs_view_reads_back_as_the_run_drew_it_with_its_number() {
    let bench = Bench::new();
    let view = bench.port(&own_mod(), "mod.fantome");
    let id = view.saved.unwrap();

    let kept = bench.store.run_view(id).unwrap().expect("the view");

    assert_eq!(kept, view);
    assert!(kept.package.is_some() && kept.targets.is_some());
    assert_eq!(bench.store.run_view(id + 1).unwrap(), None);
}

#[test]
fn a_kept_run_names_the_package_it_wrote() {
    let bench = Bench::new();
    let view = bench.port(&own_mod(), "mod.fantome");
    let id = view.saved.unwrap();

    assert_eq!(bench.store.package_of(id).unwrap(), view.written);
    assert_eq!(bench.store.package_of(id + 1).unwrap(), None);
}

#[test]
fn removing_the_only_run_empties_the_run_tier_and_the_rows() {
    let bench = Bench::new();
    let id = bench.port(&own_mod(), "mod.fantome").saved.unwrap();
    assert!(!bench.store.tier().ids().unwrap().is_empty());

    bench.store.remove_run(id).unwrap();

    for table in ["raw", "load", "run", "run_blob"] {
        assert_eq!(bench.count(table), 0, "{table}");
    }
    assert_eq!(bench.store.tier().ids().unwrap(), []);
}

/// The names of a zip's entries, in order, each with whether it is deflated.
fn entries_of(path: &Utf8Path) -> Vec<(String, bool)> {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|at| {
            let entry = zip.by_index(at).unwrap();
            let deflated = entry.compression() == zip::CompressionMethod::Deflated;
            (entry.name().to_string(), deflated)
        })
        .collect()
}

#[test]
fn a_port_exported_holds_its_json_and_one_stored_blob_per_id() {
    let bench = Bench::new();
    let request = bench.request(&own_mod(), "mod.fantome", StageName::Write);
    let id = bench.port(&own_mod(), "mod.fantome").saved.unwrap();
    let sha256 = ContentId::of(&fs::read(&request.archive).unwrap());
    let raw = bench.store.ingest().get(sha256, &Caps::default()).unwrap();
    let mut ids: Vec<ContentId> = bench.named();
    ids.extend(raw.ids());
    ids.sort();
    ids.dedup();
    let path = bench.folder.root.join("mod.reforge-project");

    let size = bench.store.export_project(id, &path).unwrap();

    assert_eq!(size, Some(fs::metadata(&path).unwrap().len()));
    let mut expected = vec![("project.json".to_string(), true)];
    expected.extend(ids.iter().map(|id| (format!("blobs/{}", id.hex()), false)));
    assert_eq!(entries_of(&path), expected);
    let mut zip = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    for id in &ids {
        let mut bytes = Vec::new();
        let mut entry = zip.by_name(&format!("blobs/{}", id.hex())).unwrap();
        std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
        assert_eq!(ContentId::of(&bytes), *id);
    }
    let json: serde_json::Value =
        serde_json::from_reader(zip.by_name("project.json").unwrap()).unwrap();
    assert_eq!(json["format"], 1);
    assert_eq!(json["run"]["origin"], "run");
    assert_eq!(json["raw"]["sha256"], sha256.hex());
    let listed = json["blobs"].as_array().unwrap();
    let listed: Vec<&str> = listed.iter().map(|b| b["id"].as_str().unwrap()).collect();
    assert_eq!(listed, ids.iter().map(ContentId::hex).collect::<Vec<_>>());
    let left: Vec<String> = fs::read_dir(&bench.folder.root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".tmp") || name.starts_with(".tmp"))
        .collect();
    assert_eq!(left, Vec::<String>::new(), "a temporary file is left");
    assert_eq!(bench.store.export_project(id + 1, &path), Ok(None));
}

/// A port of [`own_mod`] kept and exported: the bench and the project file's path.
fn exported_port() -> (Bench, u64, Utf8PathBuf) {
    let bench = Bench::new();
    let id = bench.port(&own_mod(), "mod.fantome").saved.unwrap();
    let path = bench.folder.root.join("mod.reforge-project");
    bench
        .store
        .export_project(id, &path)
        .unwrap()
        .expect("the run");
    (bench, id, path)
}

/// A copy of a zip at `to`, each entry through `edit`, which renames it, changes its bytes or
/// drops it, then `extra` added.
fn rewritten(
    from: &Utf8Path,
    to: &Utf8Path,
    edit: impl Fn(&str, Vec<u8>) -> Option<(String, Vec<u8>)>,
    extra: &[(&str, &[u8])],
) {
    let mut zip = zip::ZipArchive::new(fs::File::open(from).unwrap()).unwrap();
    let mut out = zip::ZipWriter::new(fs::File::create(to).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    let mut write = |name: &str, bytes: &[u8]| {
        out.start_file(name, options).unwrap();
        std::io::Write::write_all(&mut out, bytes).unwrap();
    };
    for at in 0..zip.len() {
        let mut entry = zip.by_index(at).unwrap();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
        let name = entry.name().to_string();
        drop(entry);
        if let Some((name, bytes)) = edit(&name, bytes) {
            write(&name, &bytes);
        }
    }
    for (name, bytes) in extra {
        write(name, bytes);
    }
    out.finish().unwrap();
}

/// `project.json`'s value edited.
fn json_edited(bytes: &[u8], edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let mut json: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    edit(&mut json);
    serde_json::to_vec(&json).unwrap()
}

#[test]
fn a_project_imported_into_another_store_reads_as_the_run_it_was() {
    let (bench, id, path) = exported_port();
    let other = Bench::new();

    let imported = other.store.import_project(&path).unwrap();

    let listed = other.store.list().unwrap();
    assert_eq!(listed.len(), 1);
    let row = &listed[0];
    assert_eq!(row.id, imported);
    assert_eq!(row.origin, Origin::Project);
    assert!(row.kept);
    assert_eq!(row.opened_from.as_deref(), Some(path.as_str()));
    assert!(row.archive_ingested);
    let first = bench.store.list().unwrap().remove(0);
    assert_eq!(
        (&row.sha256, &row.made_at, &row.reached),
        (&first.sha256, &first.made_at, &first.reached)
    );
    let view = |store: &AppStore, id| {
        let mut view = store.run_view(id).unwrap().expect("the view");
        view.saved = None;
        view
    };
    assert_eq!(view(&other.store, imported), view(&bench.store, id));
    let mut bins = Vec::new();
    for &stage in &first.reached {
        let files = bench.store.stage_files(id, stage);
        let files = files.map(|files| files.expect("the run"));
        if let Ok(files) = &files {
            let rows = files.wads.iter().flat_map(|wad| &wad.files);
            bins.extend(rows.filter(|row| row.kind == "property-bin").cloned());
        }
        assert_eq!(
            other.store.stage_files(imported, stage).map(Option::unwrap),
            files,
            "{stage:?}"
        );
    }
    assert!(!bins.is_empty());
    for bin in bins {
        let content = ContentId::parse(&bin.content).unwrap();
        let shown = other.store.file_view(imported, content, None);
        assert!(
            matches!(shown, Ok(FileView::Bin { .. })),
            "{}: {shown:?}",
            bin.content
        );
    }
}

#[test]
fn a_project_whose_blob_is_not_the_content_its_name_spells_is_refused_and_nothing_inserted() {
    let (bench, _, path) = exported_port();
    let other = Bench::new();
    let zip = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    // The last blob, so that a put of the blobs before it would leave them behind.
    let last = zip.file_names().filter(|n| n.starts_with("blobs/")).max();
    let last = last.unwrap().to_string();
    let renamed = bench.folder.root.join("renamed.reforge-project");
    let forged = format!("blobs/{}", ContentId::of(b"other bytes").hex());
    rewritten(
        &path,
        &renamed,
        |name, bytes| match name == last {
            true => Some((forged.clone(), bytes)),
            false => Some((name.to_string(), bytes)),
        },
        &[],
    );
    let flipped = bench.folder.root.join("flipped.reforge-project");
    rewritten(
        &path,
        &flipped,
        |name, mut bytes| {
            if name == last {
                bytes[0] ^= 0xff;
            }
            Some((name.to_string(), bytes))
        },
        &[],
    );

    let renamed = other.store.import_project(&renamed);
    let flipped = other.store.import_project(&flipped);

    assert!(
        matches!(&renamed, Err(AppError::ProjectMalformed { detail }) if detail.contains("not listed")),
        "{renamed:?}"
    );
    assert!(
        matches!(&flipped, Err(AppError::ProjectMalformed { detail }) if detail.contains("not its content")),
        "{flipped:?}"
    );
    for table in ["raw", "load", "run", "run_blob"] {
        assert_eq!(other.count(table), 0, "{table}");
    }
    assert_eq!(other.store.tier().ids().unwrap(), []);
    assert_eq!(other.store.ingest().entries(), []);
    let blobs = other
        .folder
        .root
        .join("store")
        .join("ingest")
        .join("content-v2");
    assert!(!blobs.exists(), "a blob reached the ingest tier");
}

#[test]
fn a_project_with_a_stray_entry_is_refused() {
    let (bench, _, path) = exported_port();
    let other = Bench::new();
    let edited = bench.folder.root.join("edited.reforge-project");
    rewritten(
        &path,
        &edited,
        |name, bytes| Some((name.to_string(), bytes)),
        &[("../readme.txt", b"a stray entry")],
    );

    let refused = other.store.import_project(&edited);

    assert!(
        matches!(refused, Err(AppError::ProjectMalformed { .. })),
        "{refused:?}"
    );
    assert_eq!(other.count("run"), 0);
}

#[test]
fn a_project_of_another_format_is_refused() {
    let (bench, _, path) = exported_port();
    let other = Bench::new();
    let edited = bench.folder.root.join("edited.reforge-project");
    rewritten(
        &path,
        &edited,
        |name, bytes| match name {
            "project.json" => Some((
                name.to_string(),
                json_edited(&bytes, |json| json["format"] = 2.into()),
            )),
            _ => Some((name.to_string(), bytes)),
        },
        &[],
    );

    let refused = other.store.import_project(&edited);

    assert_eq!(refused, Err(AppError::ProjectFormat { found: 2 }));
    assert_eq!(other.count("run"), 0);
}

#[test]
fn a_raw_row_with_a_run_and_no_load_survives_the_sweep() {
    let bench = Bench::new();
    bench.port(&own_mod(), "mod.fantome");
    // A run as an opened package makes it: its mod's raw row and no load.
    bench
        .rows()
        .execute(
            "INSERT INTO run (sha256, origin, opened_from, kind, settings, program_version,
               kept, report, view)
             SELECT sha256, 'package', 'mod.modpkg', 'port', '{}', '0', 1, x'', x'' FROM raw",
            [],
        )
        .unwrap();

    bench.store.remove_run(1).unwrap();

    assert_eq!((bench.count("raw"), bench.count("load")), (1, 0));
    let listed = bench.store.list().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].origin, Origin::Package);
    assert_eq!(listed[0].opened_from.as_deref(), Some("mod.modpkg"));
    assert_eq!(listed[0].reached, []);
    assert!(!listed[0].archive_ingested);

    bench.store.remove_run(2).unwrap();

    assert_eq!(bench.count("raw"), 0);
}

#[test]
fn two_runs_of_one_load_one_removed_keep_the_load_and_its_blobs() {
    let bench = Bench::new();
    bench.port(&own_mod(), "mod.fantome");
    bench.port(&own_mod(), "mod.fantome");
    let before = bench.store.tier().ids().unwrap();

    bench.store.remove_run(1).unwrap();

    assert_eq!(bench.ids(), [2]);
    assert_eq!((bench.count("raw"), bench.count("load")), (1, 1));
    assert_eq!(bench.store.tier().ids().unwrap(), before);
    assert_eq!(bench.named_in_tier(), before);
}

#[test]
fn removing_a_mods_runs_leaves_the_other_mods() {
    let bench = Bench::new();
    let other = own_mod().extra("README.txt", "another archive");
    bench.port(&own_mod(), "mod.fantome");
    bench.port(&other, "other.fantome");
    let listed = bench.store.list().unwrap();
    let sha256 = ContentId::parse(&listed[1].sha256).unwrap();

    bench.store.remove_mod(sha256).unwrap();

    let left = bench.store.list().unwrap();
    assert_eq!(bench.ids(), [2]);
    assert_eq!(left[0].file_name, "other.fantome");
    assert_eq!(bench.count("raw"), 1);
    assert_eq!(bench.store.tier().ids().unwrap(), bench.named_in_tier());

    bench.store.remove_all().unwrap();

    assert_eq!(bench.count("run"), 0);
    assert_eq!(bench.store.tier().ids().unwrap(), []);
}

#[test]
fn the_bound_removes_the_lowest_numbers_first() {
    let bench = Bench::bounded(2);
    let request = bench.request(&own_mod(), "mod.fantome", StageName::Write);

    for _ in 0..3 {
        bench.run(&request);
    }

    assert_eq!(bench.ids(), [3, 2]);
}

#[test]
fn a_run_marked_kept_outlives_the_bound() {
    let bench = Bench::bounded(1);
    let request = bench.request(&own_mod(), "mod.fantome", StageName::Write);
    bench.run(&request);
    bench.store.keep(1, true).unwrap();

    bench.run(&request);
    bench.run(&request);

    assert_eq!(bench.ids(), [3, 1]);
    let listed = bench.store.list().unwrap();
    assert_eq!((listed[0].kept, listed[1].kept), (false, true));

    bench.store.keep(1, false).unwrap();

    assert_eq!(bench.ids(), [3], "unmarked, the oldest is past the bound");
}
