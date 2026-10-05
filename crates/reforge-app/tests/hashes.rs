//! The hash tables downloaded by mimir's updater: a release read from a test's folder through
//! the same `Source` the web is, each table checked against its digest, installed whole, and
//! read by the name tables.

use std::io::Read;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use ltk_hashdb::{Compression, HashDbWriter};
use ltk_mimir_cache::{HashStore, Table};
use reforge::game::Names;
use reforge::model::path_hash;
use reforge_app::error::AppError;
use reforge_app::hashes::{self, HashesProgress, Release};
use reforge_app::index::Source;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const PATH: &str = "assets/characters/test/test.bin";
const BASE: &str = "release";

/// A release of one table, `game`, which names `PATH`, in a folder.
struct Folder {
    dir: TempDir,
}

impl Folder {
    fn new() -> Self {
        let folder = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        let config = Table::Game.key_config();
        let mut writer = HashDbWriter::with_key_config(config, Compression::None);
        writer.insert(config.hash(PATH), PATH);
        let table = folder.path("game-2026-10-05.lhdb");
        writer.build(fs::File::create(&table).unwrap()).unwrap();
        let bytes = fs::read(&table).unwrap();
        folder.manifest(&format!("{:x}", Sha256::digest(&bytes)), bytes.len());
        folder
    }

    fn path(&self, name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(self.dir.path().join(name)).unwrap()
    }

    /// Lists the table with this digest and size.
    fn manifest(&self, sha256: &str, size: usize) {
        let manifest = serde_json::json!({
            "schema": 1,
            "generated_at": "2026-10-05T00:00:00Z",
            "tables": {
                "game": {
                    "file": "game-2026-10-05.lhdb",
                    "sha256": sha256,
                    "entries": 1,
                    "key_width": 8,
                    "size_bytes": size,
                    "version": "2026-10-05",
                    "format_version": 1,
                },
            },
        });
        // The list of the tables this format reads, as the release publishes it.
        fs::write(self.path("manifest-v1.json"), manifest.to_string()).unwrap();
    }

    fn size(&self) -> u64 {
        fs::metadata(self.path("game-2026-10-05.lhdb"))
            .unwrap()
            .len()
    }
}

impl Source for Folder {
    fn open(&self, url: &str) -> Result<Box<dyn Read + Send>, AppError> {
        let name = url.strip_prefix(&format!("{BASE}/")).unwrap();
        // A file the release lacks fails as the web's 404 does.
        let file = fs::File::open(self.path(name)).map_err(|error| AppError::IndexFetch {
            detail: error.to_string(),
        })?;
        Ok(Box::new(file))
    }
}

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join("hashes")).unwrap();
    (dir, path)
}

fn update(
    folder: &Folder,
    cache: &Utf8Path,
    cancel: &AtomicBool,
) -> Result<Vec<HashesProgress>, AppError> {
    let heard = Mutex::new(Vec::new());
    let release = Release::new(folder, BASE, cancel);
    hashes::update(cache, &release, &|seen| heard.lock().unwrap().push(seen))?;
    Ok(heard.into_inner().unwrap())
}

fn behind(folder: &Folder, cache: &Utf8Path) -> Vec<String> {
    let never = AtomicBool::new(false);
    hashes::check(cache, &Release::new(folder, BASE, &never))
        .unwrap()
        .behind
}

#[test]
fn a_cache_with_no_tables_lacks_every_table_of_the_release() {
    let folder = Folder::new();
    let (_dir, cache) = scratch();
    let never = AtomicBool::new(false);
    let check = hashes::check(&cache, &Release::new(&folder, BASE, &never)).unwrap();
    assert_eq!(check.behind, ["game"]);
    assert_eq!(check.bytes, Some(folder.size()));
}

#[test]
fn an_update_installs_the_tables_the_names_read() {
    let folder = Folder::new();
    let (_dir, cache) = scratch();
    let heard = update(&folder, &cache, &AtomicBool::new(false)).unwrap();
    let last = heard.last().expect("the download is heard");
    assert_eq!((last.done, last.total), (folder.size(), folder.size()));

    let names = Names::builder().mimir(&cache).build().unwrap();
    assert_eq!(names.path(path_hash(PATH)).as_deref(), Some(PATH));
    assert!(
        behind(&folder, &cache).is_empty(),
        "the cache holds the release"
    );
}

#[test]
fn a_stopped_update_installs_nothing() {
    let folder = Folder::new();
    let (_dir, cache) = scratch();
    let error = update(&folder, &cache, &AtomicBool::new(true)).unwrap_err();
    assert_eq!(error, AppError::Cancelled);
    assert_eq!(behind(&folder, &cache), ["game"]);
}

#[test]
fn a_table_of_other_bytes_than_the_release_lists_is_not_installed() {
    let folder = Folder::new();
    folder.manifest(&"0".repeat(64), usize::try_from(folder.size()).unwrap());
    let (_dir, cache) = scratch();
    let error = update(&folder, &cache, &AtomicBool::new(false)).unwrap_err();
    assert!(matches!(error, AppError::HashesFetch { .. }), "{error}");
    assert_eq!(behind(&folder, &cache), ["game"]);
}

/// The newest published release installed in a scratch cache, and its path table read.
#[test]
#[ignore = "downloads the published hash tables, some 65 MB"]
fn the_published_tables_install_in_a_cache() {
    let (_dir, cache) = scratch();
    let web = reforge_app::index::Web::new().unwrap();
    let never = AtomicBool::new(false);
    let release = Release::new(&web, hashes::TABLES, &never);
    let check = hashes::check(&cache, &release).unwrap();
    eprintln!("{} tables, {:?} bytes", check.behind.len(), check.bytes);
    let started = std::time::Instant::now();
    hashes::update(&cache, &release, &|_| {}).unwrap();
    eprintln!("installed in {:.1?}", started.elapsed());

    let names = Names::builder().mimir(&cache).build().unwrap();
    assert!(names.has_mimir());
    let path = "data/characters/ahri/ahri.bin";
    assert_eq!(names.path(path_hash(path)).as_deref(), Some(path));
    assert!(hashes::check(&cache, &release).unwrap().behind.is_empty());
}

#[test]
fn a_cache_another_program_updates_is_busy() {
    let folder = Folder::new();
    let (_dir, cache) = scratch();
    let store = HashStore::at(cache.as_std_path());
    let _lock = store.try_lock_update().unwrap().expect("the lock is free");
    let error = update(&folder, &cache, &AtomicBool::new(false)).unwrap_err();
    assert_eq!(error, AppError::HashesBusy);
}
