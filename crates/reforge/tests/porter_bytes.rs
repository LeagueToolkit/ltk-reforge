//! A run reads a mod's bytes from its store on use (#ADR-0100): a file node holds the content
//! id of its bytes, a stage fetches them where it reads them, and a carried file is named by
//! its id in the package and fetched by the writer. The game is `reforge_testkit::skins`.

use camino::{Utf8Path, Utf8PathBuf};
use reforge::diag::{Diag, DiagnosticPolicy, Stage};
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::package::{Origin, PackageWriter, ProjectDir};
use reforge::porter::WorkingSetCapped;
use reforge::snapshot::RawCache;
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
