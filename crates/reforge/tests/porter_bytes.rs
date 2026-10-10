//! A run reads a mod's bytes from its store on use (#ADR-0100): a file node holds the content
//! id of its bytes, a stage fetches them where it reads them, and a carried file is named by
//! its id in the package and fetched by the writer. The game is `reforge_testkit::skins`.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use camino::{Utf8Path, Utf8PathBuf};
use reforge::diag::{Diag, DiagnosticPolicy, Stage};
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::{ByteView, ContentId};
use reforge::package::{Origin, PackageWriter, ProjectDir};
use reforge::porter::WorkingSetCapped;
use reforge::snapshot::RawCache;
use reforge::store::{BlobStore, MemoryStore};
use reforge::{Porter, Repairer, RunError};
use reforge_testkit::bins::{bin, blob, entry, particle, resolver_with};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::mods::FILE_NAME;
use reforge_testkit::skins::{self, WAD, skin_bin};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const OWN_PARTICLE: &str = "Characters/Test/Skins/Skin0/Particles/Test_Own";
const OWN_TEX: &str = "assets/mod/a/own.tex";

/// The mod's own rig at skin0 with a particle of its own.
fn own_mod() -> Vec<u8> {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
        resolver_with(RESOURCES0, &[("Test_Q_Cast", OWN_PARTICLE)]),
        particle(OWN_PARTICLE, OWN_TEX),
    ];
    let mut files = own_files("a");
    files.push((OWN_TEX.into(), blob(OWN_TEX)));
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files).build()
}

fn game_env() -> Env {
    env(skins::game(), NoHistory)
}

/// A scratch directory and a cache in it, not made yet.
fn scratch() -> (tempfile::TempDir, RawCache) {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let path = Utf8PathBuf::try_from(dir.path().join("cache")).expect("a UTF-8 path");
    (dir, RawCache::new(path))
}

#[test]
fn a_kept_file_is_named_by_its_id_and_written_from_the_store() {
    let env = game_env();
    let (scratch, cache) = scratch();
    let repairer = Repairer::new(&env);
    let mut run = repairer.run().in_cache(cache);
    run.read(&own_mod(), FILE_NAME).expect("it reads");
    run.through(Stage::Check).expect("the run answers");
    let package = run.package().expect("a package");
    let (_, key, chunk) = package
        .chunks()
        .find(|(_, key, _)| key.path.as_str() == OWN_TEX)
        .expect("the texture ships");
    assert!(
        matches!(chunk.origin, Origin::Kept(_)),
        "{:?}",
        chunk.origin
    );
    assert!(
        chunk.held().is_none(),
        "a kept file's bytes stay in the store"
    );
    assert_eq!(&*package.bytes(chunk), blob(OWN_TEX).as_slice());
    let path = key.path.clone();

    let out = Utf8Path::from_path(scratch.path())
        .expect("UTF-8")
        .join("out");
    let writers: [&dyn PackageWriter; 1] = [&ProjectDir::new(&out)];
    run.write(&writers, &DiagnosticPolicy::default())
        .expect("the package writes");
    let package = run.package().expect("a package");
    let written = out
        .join(&package.config.name)
        .join("content")
        .join("base")
        .join(WAD)
        .join(path.as_str());
    let written = fs_err::read(written).expect("the texture is written");
    assert_eq!(written, blob(OWN_TEX));
}

#[test]
fn a_stage_whose_store_lost_a_files_bytes_ends_with_a_fault_of_the_store() {
    // The bytes are fetched where a stage reads them: one the store no longer holds is a
    // fault, and the stage keeps no snapshot of what it judged without them.
    let env = game_env();
    let (_scratch, cache) = scratch();
    let porter = Porter::new(&env);
    let mut run = porter.run().in_cache(cache.clone());
    run.read(&own_mod(), FILE_NAME).expect("it reads");
    run.load().expect("the store answers");
    fs_err::remove_dir_all(cache.dir().join("content-v2")).expect("the blobs are removed");

    let Err(RunError::Store { stage, id }) = run.through(Stage::Check) else {
        panic!("a stage reads a file the store lost");
    };
    assert!(!run.ran(stage), "{stage} keeps no snapshot");
    let named = run.module().expect("the mod").files.values();
    assert!(
        named.map(|file| file.content).any(|content| content == id),
        "{id:?} is a file's"
    );
}

/// A store that loses the blob of the first read once it is armed, as a store whose blob is
/// removed under a run would, and counts the reads after it.
#[derive(Default)]
struct Losing {
    blobs: MemoryStore,
    armed: AtomicBool,
    lost: Mutex<Option<ContentId>>,
    reads_after: AtomicUsize,
}

impl Losing {
    fn read(&self, id: ContentId) -> Option<ByteView> {
        let mut lost = self.lost.lock().expect("the lock");
        if lost.is_some() {
            self.reads_after.fetch_add(1, Ordering::Relaxed);
        } else if self.armed.swap(false, Ordering::Relaxed) {
            *lost = Some(id);
        }
        (*lost != Some(id)).then(|| self.blobs.get(id)).flatten()
    }

    fn lost(&self) -> Option<ContentId> {
        *self.lost.lock().expect("the lock")
    }
}

impl BlobStore for Losing {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        self.read(id)
    }

    fn contains(&self, id: ContentId) -> bool {
        self.lost() != Some(id) && self.blobs.contains(id)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        self.blobs.put(bytes)
    }
}

/// A run of the mod started from its raw snapshot, its bytes in a store that loses one.
fn over_losing<'a>(porter: &'a Porter<'_>, bytes: &[u8]) -> (reforge::Run<'a>, Arc<Losing>) {
    let losing = Arc::new(Losing::default());
    let mut log = reforge::diag::Diagnostics::new();
    let caps = reforge::archive::Caps::default();
    let raw = reforge::archive::read(bytes, FILE_NAME, &caps, &*losing, &mut log.at(Stage::Read));
    let run = porter.resume(raw.expect("it reads"), Arc::clone(&losing));
    (run.expect("the run starts"), losing)
}

#[test]
fn a_blob_lost_mid_stage_ends_the_stage_at_its_fetch() {
    let env = game_env();
    let porter = Porter::new(&env);
    let (mut run, losing) = over_losing(&porter, &own_mod());
    run.through(Stage::Fix).expect("the store answers");
    losing.armed.store(true, Ordering::Relaxed);

    let Err(RunError::Store { stage, id }) = run.through(Stage::Check) else {
        panic!("facts reads a file the store lost");
    };
    assert_eq!((stage, Some(id)), (Stage::Facts, losing.lost()));
    assert_eq!(
        losing.reads_after.load(Ordering::Relaxed),
        0,
        "nothing is read after the fetch the store did not answer"
    );
    assert!(!run.ran(Stage::Facts), "facts keeps no snapshot");
    assert!(run.ran(Stage::Fix), "the stages before it keep theirs");
    assert!(run.fixed().is_some(), "the mod stays the run's");
}

#[test]
fn a_blob_lost_while_a_writer_reads_it_leaves_nothing_written() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    let (mut run, losing) = over_losing_repair(&repairer, &own_mod());
    run.through(Stage::Check).expect("the store answers");
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let out = Utf8Path::from_path(scratch.path())
        .expect("UTF-8")
        .join("out");
    let writers: [&dyn PackageWriter; 1] = [&ProjectDir::new(&out)];
    losing.armed.store(true, Ordering::Relaxed);

    let Err(RunError::Store { stage, id }) = run.write(&writers, &DiagnosticPolicy::default())
    else {
        panic!("a writer reads a kept file the store lost");
    };
    assert_eq!((stage, Some(id)), (Stage::Write, losing.lost()));
    assert!(!run.ran(Stage::Write));
    let left: Vec<_> = fs_err::read_dir(&out)
        .expect("the output directory")
        .collect();
    assert!(left.is_empty(), "the staged project is removed: {left:?}");

    // A blob lost before the write is found before any writer starts.
    let Err(RunError::Store { stage, .. }) = run.write(&writers, &DiagnosticPolicy::default())
    else {
        panic!("the store lacks a kept file");
    };
    assert_eq!(stage, Stage::Write);
    assert_eq!(fs_err::read_dir(&out).expect("the output").count(), 0);
}

/// [`over_losing`] for a repair, whose package keeps the mod's files by id.
fn over_losing_repair<'a>(
    repairer: &'a Repairer<'_>,
    bytes: &[u8],
) -> (reforge::Run<'a>, Arc<Losing>) {
    let losing = Arc::new(Losing::default());
    let mut log = reforge::diag::Diagnostics::new();
    let caps = reforge::archive::Caps::default();
    let raw = reforge::archive::read(bytes, FILE_NAME, &caps, &*losing, &mut log.at(Stage::Read));
    let run = repairer.resume(raw.expect("it reads"), Arc::clone(&losing));
    (run.expect("the run starts"), losing)
}

#[test]
fn what_a_stage_holds_at_once_is_counted_while_its_fetches_live() {
    let store = std::sync::Arc::new(reforge::store::MemoryStore::new());
    let (a, b) = (
        reforge::store::BlobStore::put(&*store, blob("a").into()),
        reforge::store::BlobStore::put(&*store, blob("bb").into()),
    );
    let blobs = reforge::model::Blobs::new(store);
    let (size_a, size_b) = (blob("a").len() as u64, blob("bb").len() as u64);
    let first = blobs.fetch(a);
    let second = blobs.fetch(b);
    drop((first, second));
    let third = blobs.fetch(a);
    assert_eq!(blobs.take_peak(), size_a + size_b, "the two held together");
    assert_eq!(
        blobs.take_peak(),
        size_a,
        "the count starts again from what is held"
    );
    drop(third);
    assert_eq!(blobs.take_peak(), size_a);
    assert_eq!(blobs.take_peak(), 0);
}

#[test]
fn a_stage_that_holds_more_than_the_working_set_cap_raises_a_note_and_runs_on() {
    let env = game_env();
    let bytes = own_mod();
    let capped = reforge::archive::Caps {
        working_bytes: 16,
        ..reforge::archive::Caps::default()
    };
    let porter = Porter::new(&env);
    let run = porter.port(&bytes, FILE_NAME).expect("the run answers");
    let notes = |run: &reforge::Run<'_>| -> Vec<Stage> {
        let raised = run.log().iter();
        let raised = raised.filter(|diagnostic| diagnostic.code == WorkingSetCapped::CODE);
        raised.map(|diagnostic| diagnostic.stage).collect()
    };
    assert_eq!(notes(&run), [], "the default cap holds every file");

    let porter = Porter::new(&env).caps(capped);
    let held = porter.port(&bytes, FILE_NAME).expect("the run answers");
    let stages = notes(&held);
    assert!(stages.contains(&Stage::Load), "{stages:?}");
    assert!(stages.contains(&Stage::Facts), "{stages:?}");
    let note = held.log().of::<WorkingSetCapped>().next().expect("a note");
    assert_eq!(note.cap, 16);
    assert!(note.held > 16);
    assert_eq!(held.package(), run.package(), "the stages run on");
}
