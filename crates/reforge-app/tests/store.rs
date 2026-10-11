//! The app's store (#ADR-0096): the run tier's blobs by content id, the rows, and the runs the
//! app keeps. The game is `reforge_testkit::skins`.

use camino::{Utf8Path, Utf8PathBuf};
use fjall::{Database, KeyspaceCreateOptions};
use fs_err as fs;
use reforge::archive::Caps;
use reforge::fixups::{FixupSelection, FixupSet};
use reforge::history::NoHistory;
use reforge::model::{ByteView, ContentId};
use reforge::snapshot::{self, BuildSnapshot, CheckSnapshot, LoadSnapshot, StageSnapshot};
use reforge::store::BlobStore;
use reforge_app::inputs::Inputs;
use reforge_app::run::{
    self, PartView, RunKind, RunRequest, RunSettings, RunView, StageEvent, StageName, StageState,
    Unwatched, Watch,
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
    run::run(&bench.inputs, &request, Some(&bench.store), &mut first).unwrap();

    let mut heard = Heard::default();
    run::run(&bench.inputs, &request, Some(&bench.store), &mut heard).unwrap();

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
