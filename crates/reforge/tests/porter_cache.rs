//! The raw snapshot as a cache (#ADR-0095): a run that finds its archive's raw snapshot in the
//! cache takes it in place of `read`, and one that does not reads the archive into the cache.
//! Either way the run's report is the report of a run with no cache. The game is
//! `reforge_testkit::skins`.

use camino::{Utf8Path, Utf8PathBuf};
use reforge::Porter;
use reforge::archive::{Caps, RAW_VERSION, RawSnapshot};
use reforge::diag::Stage;
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::ContentId;
use reforge::snapshot::{RawCache, to_json};
use reforge::store::BlobStore;
use reforge_testkit::bins::{bin, blob, entry, particle, resolver_with};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::mods::FILE_NAME;
use reforge_testkit::skins::{self, WAD, skin_bin};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const OWN_PARTICLE: &str = "Characters/Test/Skins/Skin0/Particles/Test_Own";
const OWN_TEX: &str = "assets/mod/a/own.tex";

/// The mod's own rig at skin0 with a particle of its own, an entry outside the WADs, and a
/// chunk listed twice, which the read raises.
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
    archive(&files)
        .file(WAD, "assets/mod/a/twice.tex", blob("first"))
        .file(WAD, "ASSETS/mod/a/twice.tex", blob("second"))
        .extra("README.txt", b"read me".to_vec())
        .build()
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

/// Every file under a directory.
fn files(dir: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut found = Vec::new();
    let mut open = vec![dir.to_path_buf()];
    while let Some(dir) = open.pop() {
        for entry in fs_err::read_dir(&dir).expect("the directory reads") {
            let path = Utf8PathBuf::try_from(entry.expect("an entry").path()).expect("UTF-8");
            if path.is_dir() {
                open.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found
}

/// The report of a port of the archive, read through the cache when one is given, and the raw
/// snapshot the run held.
fn port(porter: &Porter<'_>, cache: Option<&RawCache>, bytes: &[u8]) -> (String, RawSnapshot) {
    let mut run = porter.run();
    if let Some(cache) = cache {
        run = run.in_cache(cache.clone());
    }
    let raw = run
        .read(bytes, FILE_NAME)
        .expect("the archive reads")
        .clone();
    run.through(Stage::Check).expect("no fault");
    assert_eq!(run.halted(), None);
    (run.report().to_json(), raw)
}

#[test]
fn a_second_read_of_an_archive_takes_its_raw_snapshot_from_the_cache() {
    let env = game_env();
    let porter = Porter::new(&env);
    let bytes = own_mod();
    let (report, raw) = port(&porter, None, &bytes);
    assert!(!raw.findings.is_empty(), "the read raised a finding");

    let (_scratch, cache) = scratch();
    let sha256 = ContentId::of(&bytes);
    assert_eq!(cache.get(sha256, &Caps::default()), None, "an empty cache");
    let (populating, first) = port(&porter, Some(&cache), &bytes);
    assert_eq!(populating, report, "the run that fills the cache");
    assert_eq!(first, raw);
    let entry = cache.entry(sha256, RAW_VERSION, &Caps::default());
    assert!(entry.is_file(), "the entry of the archive");
    assert!(raw.ids().all(|id| cache.store().contains(id)));

    let (hitting, second) = port(&porter, Some(&cache), &bytes);
    assert_eq!(hitting, report, "the run that reads the cache");
    assert_eq!(second, raw);
    for id in raw.ids() {
        let bytes = cache.store().get(id).expect("the store holds every id");
        assert_eq!(ContentId::of(&bytes), id);
    }

    // The entry is what the second read gave: edited, the run reads the edit.
    let mut edited: serde_json::Value =
        serde_json::from_str(&fs_err::read_to_string(&entry).expect("the entry reads"))
            .expect("JSON");
    edited["meta"]["name"] = serde_json::json!("from the cache");
    fs_err::write(&entry, edited.to_string()).expect("the entry writes");
    let mut run = porter.run().in_cache(cache.clone());
    let read = run.read(&bytes, "another name.fantome").expect("a hit");
    assert_eq!(
        read.meta.name.as_ref().map(|name| name.raw()),
        Some("from the cache")
    );
    assert_eq!(
        read.meta.file_name.raw(),
        "another name.fantome",
        "the caller's name for the archive"
    );
}

#[test]
fn a_run_with_a_cache_puts_the_archives_bytes_in_the_ingest_store_and_not_its_layer() {
    let env = game_env();
    let porter = Porter::new(&env);
    let (_scratch, cache) = scratch();
    let mut run = porter.run().in_cache(cache.clone());
    let raw = run.read(&own_mod(), FILE_NAME).expect("the archive reads");
    let ids: Vec<ContentId> = raw.ids().collect();
    assert!(run.into_layer().is_empty(), "read put nothing in the layer");
    assert!(ids.iter().all(|&id| cache.store().contains(id)));
}

#[test]
fn a_raw_snapshot_of_other_caps_or_another_reading_is_a_miss() {
    let env = game_env();
    let porter = Porter::new(&env);
    let bytes = own_mod();
    let sha256 = ContentId::of(&bytes);
    let (_scratch, cache) = scratch();
    let (report, raw) = port(&porter, Some(&cache), &bytes);

    let caps = Caps {
        entries: 1 << 10,
        ..Caps::default()
    };
    assert_eq!(cache.get(sha256, &caps), None, "other caps");
    let capped = Porter::new(&env).caps(caps);
    let (_, capped_raw) = port(&capped, Some(&cache), &bytes);
    assert_eq!(capped_raw.caps, caps, "read under the run's caps");
    let entries = cache.dir().join("raw");
    assert_eq!(files(&entries).len(), 2, "an entry for each caps");
    assert_eq!(cache.get(sha256, &Caps::default()), Some(raw.clone()));

    // An entry of another reading is under a name of its own, which this program never asks.
    let mut other = raw.clone();
    other.version = RAW_VERSION + 1;
    cache.put(&other).expect("the entry writes");
    let current = cache.entry(sha256, RAW_VERSION, &Caps::default());
    fs_err::remove_file(&current).expect("the entry is removed");
    assert_eq!(cache.get(sha256, &Caps::default()), None);
    // And one that says another reading under this reading's name does not read as it.
    fs_err::write(&current, to_json(&other)).expect("the entry writes");
    assert_eq!(cache.get(sha256, &Caps::default()), None);

    let (again, read) = port(&porter, Some(&cache), &bytes);
    assert_eq!(again, report, "a miss reads the archive");
    assert_eq!(read, raw);
    assert_eq!(cache.get(sha256, &Caps::default()), Some(raw));
}

#[test]
fn a_raw_snapshot_naming_a_blob_the_store_lacks_is_a_miss_and_no_fault() {
    let env = game_env();
    let porter = Porter::new(&env);
    let bytes = own_mod();
    let sha256 = ContentId::of(&bytes);
    let (_scratch, cache) = scratch();
    let (report, raw) = port(&porter, Some(&cache), &bytes);

    let id = raw.ids().next().expect("the archive has a file");
    let blob = files(cache.dir()).into_iter().find(|path| {
        path.file_name()
            .is_some_and(|name| id.hex().ends_with(name) && name.len() == 60)
    });
    fs_err::remove_file(blob.expect("the blob's file")).expect("the blob is removed");
    assert!(!cache.store().contains(id));
    assert_eq!(cache.get(sha256, &Caps::default()), None);

    let (again, read) = port(&porter, Some(&cache), &bytes);
    assert_eq!(again, report);
    assert_eq!(read, raw);
    assert!(cache.store().contains(id), "the miss put the blob back");
    assert_eq!(cache.get(sha256, &Caps::default()), Some(raw));
}

#[test]
fn a_run_started_from_the_raw_snapshot_the_cache_holds_by_hash_reports_as_one_that_read() {
    // `reforge port <sha256> --store`: no archive file is read, and its bytes are the store's.
    let env = game_env();
    let porter = Porter::new(&env);
    let bytes = own_mod();
    let (_scratch, cache) = scratch();
    let (report, _) = port(&porter, Some(&cache), &bytes);

    let sha256 = ContentId::of(&bytes);
    let raw = cache
        .get(sha256, porter.settings().caps())
        .expect("the cache holds it");
    let run = porter.run().in_cache(cache.clone());
    let mut run = run.resume(raw).expect("the snapshot is this program's");
    assert!(run.ran(Stage::Read));
    run.through(Stage::Check).expect("no fault");
    assert_eq!(run.report().to_json(), report);
    let absent = ContentId::of(b"no archive the cache read");
    assert_eq!(cache.get(absent, porter.settings().caps()), None);
}
