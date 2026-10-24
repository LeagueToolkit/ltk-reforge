//! A batch as the app runs it (#ADR-0091): the archives of a folder listed, and a batch run
//! against the testkit's game (`reforge_testkit::skins`) with a store in a tempdir.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::diag::Cancel;
use reforge::history::NoHistory;
use reforge_app::batch::{self, BatchInputs, BatchProgress, BatchRequest, BatchView, Outcome};
use reforge_app::error::AppError;
use reforge_app::inputs::Inputs;
use reforge_app::run::{RunKind, RunSettings, StageName};
use reforge_app::store::AppStore;
use reforge_testkit::ModArchive;
use reforge_testkit::bins::{bin, blob, entry};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, skin_bin};
use tempfile::TempDir;

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

/// A folder for archives, removed with it.
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

    fn path(&self, name: &str) -> Utf8PathBuf {
        self.root.join(name)
    }
}

#[test]
fn a_folder_lists_its_archives_by_extension_in_any_case_sorted_by_name() {
    let folder = Folder::new();
    for name in ["b.zip", "a.FANTOME", "readme.txt", "c.fantome.bak", "d.Zip"] {
        fs::write(folder.path(name), b"").unwrap();
    }
    fs::create_dir(folder.path("sub")).unwrap();
    fs::write(folder.path("sub").join("e.fantome"), b"").unwrap();
    // A folder named as an archive is no archive.
    fs::create_dir(folder.path("f.zip")).unwrap();

    let listed = batch::list_archives(&folder.root).unwrap();

    let expected: Vec<String> = ["a.FANTOME", "b.zip", "d.Zip"]
        .iter()
        .map(|name| folder.path(name).to_string())
        .collect();
    assert_eq!(listed, expected);
}

#[test]
fn a_folder_that_does_not_read_is_refused_with_its_path() {
    let folder = Folder::new();
    let missing = folder.path("missing");

    let Err(AppError::FolderUnreadable { path, .. }) = batch::list_archives(&missing) else {
        panic!("a missing folder listed");
    };
    assert_eq!(path, missing.to_string());
}

/// A mod with its own rig at skin0, which a repair writes.
fn own_mod() -> ModArchive {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
    ];
    let mut files = own_files("a");
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files)
}

/// A mod of one texture in a WAD of no champion: `load` ends at `archive.no-skin`.
fn no_skin_mod() -> ModArchive {
    ModArchive::new().file("Other.wad.client", "assets/other/x.tex", blob("x"))
}

/// The testkit's game, opened anew each time a batch opens its inputs.
struct Game;

impl BatchInputs for Game {
    fn open(&self) -> Result<Arc<Inputs>, AppError> {
        Ok(Arc::new(Inputs::of_env(env(skins::game(), NoHistory))))
    }

    fn forget(&self) {}
}

/// A store and a folder of archives.
struct Bench {
    folder: Folder,
    store: AppStore,
}

impl Bench {
    fn new() -> Self {
        let folder = Folder::new();
        let store = AppStore::open(&folder.path("store")).unwrap();
        Self { folder, store }
    }

    /// Three archives: one that does not read, one that halts in `load`, one that writes.
    fn request(&self) -> BatchRequest {
        let unreadable = self.folder.path("a-unreadable.fantome");
        fs::write(&unreadable, b"not a zip").unwrap();
        let halts = self.folder.path("b-no-skin.fantome");
        fs::write(&halts, no_skin_mod().build()).unwrap();
        let writes = self.folder.path("c-own.fantome");
        fs::write(&writes, own_mod().build()).unwrap();
        BatchRequest {
            kind: RunKind::Repair,
            through: StageName::Write,
            archives: [unreadable, halts, writes].map(String::from).to_vec(),
            settings: RunSettings::default(),
            out: Some(self.folder.path("out").to_string()),
            format: reforge_app::settings::PackageFormat::default(),
        }
    }

    fn run(
        &self,
        request: &BatchRequest,
        cancel: &Cancel,
        progress: &mut dyn FnMut(BatchProgress),
    ) -> BatchView {
        batch::run_batch(&Game, request, Some(&self.store), cancel, progress).unwrap()
    }
}

#[test]
fn each_archive_of_a_batch_is_a_row_of_its_own_runs_answer() {
    let bench = Bench::new();
    let mut heard = Vec::new();

    let view = bench.run(&bench.request(), &Cancel::new(), &mut |seen| {
        heard.push(seen)
    });

    assert!(!view.stopped);
    let outcomes: Vec<&Outcome> = view.rows.iter().map(|row| &row.outcome).collect();
    assert_eq!(
        outcomes,
        [
            &Outcome::Unreadable,
            &Outcome::Halted {
                code: "archive.no-skin".to_string()
            },
            &Outcome::Written
        ]
    );
    let names: Vec<&str> = view.rows.iter().map(|row| row.archive.as_str()).collect();
    assert_eq!(
        names,
        ["a-unreadable.fantome", "b-no-skin.fantome", "c-own.fantome"]
    );
    assert_eq!(view.rows[1].counts.fatal, 1);
    let written = &view.rows[2];
    assert_eq!(written.counts.fatal, 0);
    let package = written.package.as_deref().expect("the package written");
    assert!(Utf8PathBuf::from(package).is_file(), "{package}");
    let saved = written.saved.expect("the run kept");
    let kept: Vec<u64> = bench
        .store
        .list()
        .unwrap()
        .iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(kept, [saved]);
    // Each archive is heard as it starts and as it ends, with its row.
    let heard: Vec<(usize, bool)> = heard
        .iter()
        .map(|seen| (seen.index, seen.row.is_some()))
        .collect();
    assert_eq!(
        heard,
        [
            (0, false),
            (0, true),
            (1, false),
            (1, true),
            (2, false),
            (2, true)
        ]
    );
}

/// #ADR-0107: a batch through `decide` scans the mods' origins, writes nothing and keeps no
/// run.
#[test]
fn a_batch_through_decide_answers_each_units_origin_and_writes_nothing() {
    let bench = Bench::new();
    let mut request = bench.request();
    request.kind = RunKind::Port;
    request.through = StageName::Decide;

    let view = bench.run(&request, &Cancel::default(), &mut |_| {});

    let outcomes: Vec<&Outcome> = view.rows.iter().map(|row| &row.outcome).collect();
    assert_eq!(
        outcomes,
        [
            &Outcome::Unreadable,
            &Outcome::Halted {
                code: "archive.no-skin".to_string()
            },
            &Outcome::Scanned
        ]
    );
    let scanned = &view.rows[2];
    assert_eq!((scanned.package.as_deref(), scanned.saved), (None, None));
    assert!(bench.store.list().unwrap().is_empty());
    let [unit] = &scanned.units[..] else {
        panic!("one unit decided: {:?}", scanned.units);
    };
    assert_eq!((unit.character.as_str(), unit.source), ("test", 0));
    assert!(
        ["base", "premium", "custom", "unknown"].contains(&unit.origin.verdict.as_str()),
        "{:?}",
        unit.origin
    );
    assert!(
        unit.origin.strength.is_none() || !unit.origin.evidence.is_empty(),
        "{:?}",
        unit.origin
    );
    let out = bench.folder.path("out");
    assert!(!out.exists() || fs::read_dir(&out).unwrap().next().is_none());
}

#[test]
fn a_cancel_set_before_the_second_archive_stops_the_batch_after_the_first() {
    let bench = Bench::new();
    let mut request = bench.request();
    request.archives.rotate_left(2);
    let cancel = Cancel::new();
    let set = cancel.clone();

    let view = bench.run(&request, &cancel, &mut |seen| {
        if seen.row.is_some() {
            set.cancel();
        }
    });

    assert!(view.stopped);
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].outcome, Outcome::Written);
}

#[test]
fn a_cancel_set_while_an_archive_runs_ends_it_and_the_batch() {
    let bench = Bench::new();
    let mut request = bench.request();
    request.archives.rotate_left(2);
    let cancel = Cancel::new();
    let set = cancel.clone();

    let view = bench.run(&request, &cancel, &mut |seen| {
        if seen.row.is_none() {
            set.cancel();
        }
    });

    assert!(view.stopped);
    assert_eq!(view.rows.len(), 1);
    let Outcome::Cancelled { stage } = view.rows[0].outcome else {
        panic!("{:?}", view.rows[0].outcome);
    };
    assert!(
        matches!(stage, StageName::Read | StageName::Load),
        "{stage:?}"
    );
    assert_eq!(view.rows[0].saved, None);
    assert!(bench.store.list().unwrap().is_empty());
}

#[test]
fn settings_the_library_does_not_take_refuse_the_batch_before_it_starts() {
    let bench = Bench::new();
    let mut request = bench.request();
    request.settings.checks_off = vec!["no-such-check".to_string()];

    let ran = batch::run_batch(
        &Game,
        &request,
        Some(&bench.store),
        &Cancel::new(),
        &mut |_| {
            panic!("an archive started");
        },
    );

    assert!(matches!(ran, Err(AppError::UnknownCheck { .. })));
}

/// The testkit's game, counting how often a batch opens and forgets it.
#[derive(Default)]
struct Counted {
    opened: AtomicUsize,
    forgotten: AtomicUsize,
}

impl BatchInputs for Counted {
    fn open(&self) -> Result<Arc<Inputs>, AppError> {
        self.opened.fetch_add(1, Ordering::Relaxed);
        Game.open()
    }

    fn forget(&self) {
        self.forgotten.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn a_batch_opens_its_inputs_again_after_every_sixteen_archives() {
    let bench = Bench::new();
    let path = bench.folder.path("unreadable.fantome");
    fs::write(&path, b"not a zip").unwrap();
    let request = BatchRequest {
        archives: vec![path.to_string(); 2 * batch::REOPEN_EVERY + 1],
        ..bench.request()
    };
    let inputs = Counted::default();

    let view = batch::run_batch(&inputs, &request, None, &Cancel::new(), &mut |_| {}).unwrap();

    assert_eq!(view.rows.len(), 2 * batch::REOPEN_EVERY + 1);
    assert_eq!(inputs.opened.load(Ordering::Relaxed), 3);
    assert_eq!(inputs.forgotten.load(Ordering::Relaxed), 2);
}
