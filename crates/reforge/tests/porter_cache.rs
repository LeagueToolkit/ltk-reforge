//! The raw snapshot as a cache (#ADR-0095): a run that finds its archive's raw snapshot in the
//! cache takes it in place of `read`, and one that does not reads the archive into the cache.
//! Either way the run's report is the report of a run with no cache. The cache is cleared by
//! archive. The game is `reforge_testkit::skins`.

use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};
use reforge::Porter;
use reforge::archive::{Caps, RAW_VERSION, RawSnapshot};
use reforge::diag::Stage;
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::{ContentId, FactAccess};
use reforge::snapshot::{Cleared, RawCache, to_json};
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

/// [`own_mod`] with a file of its own beside the ones every such mod has.
fn mod_with(own: &str) -> Vec<u8> {
    let objects = vec![entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0))];
    let mut files = own_files("a");
    files.push((skin_bin("test", 0), bin(objects)));
    let path = format!("assets/mod/a/{own}.tex");
    archive(&files).file(WAD, &path, blob(own)).build()
}

/// The raw snapshot of an archive loaded through the cache, which keeps it and the facts of
/// its blobs.
fn ingested(porter: &Porter<'_>, cache: &RawCache, bytes: &[u8]) -> RawSnapshot {
    let mut run = porter.run().in_cache(cache.clone());
    let raw = run
        .read(bytes, FILE_NAME)
        .expect("the archive reads")
        .clone();
    run.load().expect("the mod loads");
    raw
}

/// The bytes of the blobs a raw snapshot names, each once.
fn bytes_named(cache: &RawCache, ids: &BTreeSet<ContentId>) -> u64 {
    let size = |&id| cache.store().get(id).map_or(0, |bytes| bytes.len() as u64);
    ids.iter().map(size).sum()
}

#[test]
fn clearing_one_of_two_archives_that_share_a_blob_keeps_the_shared_blob_and_the_other() {
    let env = game_env();
    let porter = Porter::new(&env);
    let (_scratch, cache) = scratch();
    let (first, second) = (mod_with("first"), mod_with("second"));
    let first_raw = ingested(&porter, &cache, &first);
    let second_raw = ingested(&porter, &cache, &second);
    let first_ids: BTreeSet<ContentId> = first_raw.ids().collect();
    let second_ids: BTreeSet<ContentId> = second_raw.ids().collect();
    let shared: BTreeSet<ContentId> = first_ids.intersection(&second_ids).copied().collect();
    let own: Vec<ContentId> = first_ids.difference(&second_ids).copied().collect();
    assert!(!shared.is_empty() && !own.is_empty());
    let facts = cache.facts();
    assert!(
        own.iter().any(|&id| facts.row(id).is_some()),
        "load kept facts"
    );

    let entries = cache.entries();
    let mut listed = vec![
        (first_raw.meta.sha256, bytes_named(&cache, &first_ids)),
        (second_raw.meta.sha256, bytes_named(&cache, &second_ids)),
    ];
    listed.sort();
    let bytes = |entry| cache.entry_bytes(entry);
    let found: Vec<(ContentId, u64)> = entries.iter().map(|e| (e.sha256, bytes(e))).collect();
    assert_eq!(found, listed);
    assert!(
        entries
            .iter()
            .all(|e| e.version == RAW_VERSION && e.caps_hash.len() == 16)
    );
    let own_bytes = bytes_named(&cache, &own.iter().copied().collect());

    let cleared = cache
        .clear(first_raw.meta.sha256)
        .expect("the cache clears");

    assert_eq!(cleared.entries, 1);
    assert_eq!(cleared.blobs, own.len());
    assert_eq!(cleared.bytes, own_bytes);
    assert!(own.iter().all(|&id| !cache.store().contains(id)));
    assert!(
        own.iter().all(|&id| facts.row(id).is_none()),
        "the facts go with them"
    );
    assert!(shared.iter().all(|&id| cache.store().contains(id)));
    let caps = Caps::default();
    assert_eq!(cache.get(first_raw.meta.sha256, &caps), None);
    assert_eq!(
        cache.get(second_raw.meta.sha256, &caps),
        Some(second_raw.clone())
    );
    let left: Vec<ContentId> = cache.entries().iter().map(|e| e.sha256).collect();
    assert_eq!(left, [second_raw.meta.sha256]);
    assert_eq!(
        cache.clear(first_raw.meta.sha256).unwrap(),
        Cleared::default()
    );
}

#[test]
fn clearing_the_archives_not_kept_leaves_the_kept_ones() {
    let env = game_env();
    let porter = Porter::new(&env);
    let (_scratch, cache) = scratch();
    let first = ingested(&porter, &cache, &mod_with("first"));
    let second = ingested(&porter, &cache, &mod_with("second"));

    let cleared = cache
        .clear_unnamed(&[second.meta.sha256])
        .expect("the cache clears");

    assert_eq!(cleared.entries, 1);
    let caps = Caps::default();
    assert_eq!(cache.get(first.meta.sha256, &caps), None);
    assert_eq!(cache.get(second.meta.sha256, &caps), Some(second.clone()));
    let left: Vec<ContentId> = cache.entries().iter().map(|e| e.sha256).collect();
    assert_eq!(left, [second.meta.sha256]);
}
