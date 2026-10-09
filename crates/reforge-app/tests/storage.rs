//! What the app keeps on disk for the history, measured and removed (#ADR-0061): the indexes it
//! installed, its clones of the census history and the downloads kept. A folder the user chose
//! outside the app's own is measured and never removed.

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::{DB_FILE, FORMAT, STAMP_FILE};
use reforge_app::archive::Clones;
use reforge_app::index::folder_name;
use reforge_app::storage::{KeptIndex, Storage};
use tempfile::TempDir;

fn stamp(builds: usize) -> String {
    format!(
        r#"{{
  "format": {FORMAT},
  "repo": "https://github.com/LeagueToolkit/ltk-census-history.git",
  "head": "5b121f51ad752b06ce0cb0ea17d5859f4b20c645",
  "newest_build": "16.19.8207193",
  "builds": {builds},
  "families": ["files", "objects", "clips", "rigs", "textures"],
  "writer": "0.1.0",
  "rows": {{}},
  "unparsed_texts": 0,
  "objectless_texts": 0,
  "objects": {{ "level": 19, "keyframe_every": 8, "dict_cap": 1048576, "zstd": "1.5.7" }}
}}"#
    )
}

/// The app's data folder, in a temporary one.
struct Data {
    _dir: TempDir,
    root: Utf8PathBuf,
    clones: Clones,
}

impl Data {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let clones = Clones {
            whole: root.join("census-history"),
            partial: root.join("census-partial"),
        };
        Self {
            _dir: dir,
            root,
            clones,
        }
    }

    fn indexes(&self) -> Utf8PathBuf {
        self.root.join("history-indexes")
    }

    fn downloads(&self) -> Utf8PathBuf {
        self.root.join("downloads")
    }

    /// An index of `builds` builds in the folder `name` under `root`, its database `db` bytes.
    fn index(&self, root: &Utf8Path, name: &str, builds: usize, db: usize) -> Utf8PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(STAMP_FILE), stamp(builds)).unwrap();
        fs::write(dir.join(DB_FILE), vec![0; db]).unwrap();
        dir
    }

    fn file(&self, path: Utf8PathBuf, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0; bytes]).unwrap();
    }
}

#[test]
fn nothing_on_disk_measures_as_nothing() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    let view = storage.view(None);

    assert_eq!(view.index_bytes, 0);
    assert!(!view.index_owned);
    assert_eq!(view.kept, None);
    assert_eq!(view.census_bytes, 0);
    assert_eq!(view.downloads_bytes, 0);
}

#[test]
fn the_index_chosen_is_measured_and_owned_when_the_app_installed_it() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    let chosen = data.index(&indexes, &folder_name("history-v2-snapshot-0"), 2, 1000);
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    let view = storage.view(Some(&chosen));

    assert_eq!(view.index_bytes, 1000 + stamp(2).len() as u64);
    assert!(view.index_owned);
    assert_eq!(view.kept, None);
}

#[test]
fn an_index_in_a_folder_the_user_chose_is_not_the_apps() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    let chosen = data.index(&data.root.join("elsewhere"), "index", 2, 1000);
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    let view = storage.view(Some(&chosen));

    assert!(view.index_bytes > 0);
    assert!(!view.index_owned);
}

#[test]
fn the_index_kept_is_the_one_with_the_most_builds_the_settings_do_not_choose() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    data.index(&indexes, &folder_name("history-v2-snapshot-0"), 2, 10);
    let newer = data.index(&indexes, &folder_name("head-0123456789ab"), 5, 20);
    // An install that stopped holds a stamp and no whole database.
    let stopped = format!("{}.partial", folder_name("history-v2-snapshot-1"));
    data.index(&indexes, &stopped, 9, 5);
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    let view = storage.view(None);

    assert_eq!(
        view.kept,
        Some(KeptIndex {
            dir: newer.to_string(),
            newest_build: "16.19.8207193".to_string(),
            builds: 5,
            bytes: 20 + stamp(5).len() as u64,
        })
    );
    assert_eq!(storage.view(Some(&newer)).kept.unwrap().builds, 2);
}

#[test]
fn the_indexes_are_measured_together_the_chosen_and_a_stopped_install_among_them() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    let chosen = data.index(&indexes, &folder_name("history-v2-snapshot-0"), 2, 10);
    data.index(&indexes, &folder_name("head-0123456789ab"), 5, 20);
    let stopped = format!("{}.partial", folder_name("history-v2-snapshot-1"));
    data.index(&indexes, &stopped, 9, 5);
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    let view = storage.view(Some(&chosen));

    let stamps = (stamp(2).len() + stamp(5).len() + stamp(9).len()) as u64;
    assert_eq!(view.indexes_bytes, 10 + 20 + 5 + stamps);
    assert_eq!(view.index_bytes, 10 + stamp(2).len() as u64);
}

#[test]
fn the_clones_and_the_downloads_are_measured_with_all_they_hold() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    data.file(data.clones.whole.join("objects/pack/pack-1.pack"), 300);
    data.file(data.clones.partial.join("HEAD"), 20);
    data.file(data.root.join("census-history.new/HEAD"), 4);
    data.file(downloads.join("a.zst.partial"), 70);
    data.file(downloads.join("a.zst.partial.json"), 7);
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    let view = storage.view(None);

    assert_eq!(view.census_bytes, 324);
    assert_eq!(view.downloads_bytes, 77);
}

#[test]
fn removing_the_indexes_leaves_the_clones_the_downloads_and_a_chosen_folder() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    data.index(&indexes, &folder_name("history-v2-snapshot-0"), 2, 10);
    data.index(&indexes, &folder_name("head-0123456789ab"), 5, 20);
    let elsewhere = data.index(&data.root.join("elsewhere"), "index", 2, 10);
    data.file(data.clones.whole.join("HEAD"), 20);
    data.file(downloads.join("a.zst.partial"), 70);
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    storage.remove_indexes().unwrap();

    let view = storage.view(Some(&elsewhere));
    assert_eq!(view.kept, None);
    assert!(view.index_bytes > 0);
    assert_eq!(view.census_bytes, 20);
    assert_eq!(view.downloads_bytes, 70);
}

#[test]
fn removing_the_clones_leaves_the_indexes_and_the_downloads() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    let chosen = data.index(&indexes, &folder_name("history-v2-snapshot-0"), 2, 10);
    data.file(data.clones.whole.join("HEAD"), 20);
    data.file(data.clones.partial.join("HEAD"), 20);
    data.file(data.root.join("census-history.new/HEAD"), 4);
    data.file(data.root.join("census-history.old/HEAD"), 4);
    data.file(downloads.join("a.zst.partial"), 70);
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    storage.remove_clones().unwrap();

    let view = storage.view(Some(&chosen));
    assert_eq!(view.census_bytes, 0);
    assert!(!data.root.join("census-history.old").exists());
    assert!(view.index_bytes > 0);
    assert_eq!(view.downloads_bytes, 70);
}

#[test]
fn removing_the_downloads_leaves_the_indexes_and_the_clones() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    let chosen = data.index(&indexes, &folder_name("history-v2-snapshot-0"), 2, 10);
    data.file(data.clones.whole.join("HEAD"), 20);
    data.file(downloads.join("a.zst.partial"), 70);
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    storage.remove_downloads().unwrap();

    let view = storage.view(Some(&chosen));
    assert_eq!(view.downloads_bytes, 0);
    assert!(view.index_bytes > 0);
    assert_eq!(view.census_bytes, 20);
}

#[test]
fn removing_what_is_absent_removes_nothing_and_does_not_fail() {
    let data = Data::new();
    let (indexes, downloads) = (data.indexes(), data.downloads());
    let storage = Storage {
        indexes: &indexes,
        clones: &data.clones,
        downloads: &downloads,
    };

    storage.remove_indexes().unwrap();
    storage.remove_clones().unwrap();
    storage.remove_downloads().unwrap();
}
