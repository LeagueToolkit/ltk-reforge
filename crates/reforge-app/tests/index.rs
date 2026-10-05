//! The prebuilt history index: the release picked from GitHub's list, its files downloaded and
//! checked against their digests, the database unpacked, and the index installed only whole.
//! The files are read from a test's folder through the same `Source` the web is.

use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::{History, SqliteHistory};
use reforge_app::error::AppError;
use reforge_app::index::{
    self, Asset, IndexRelease, InstallProgress, InstallStep, Installer, Ranged, Source,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const COMMIT: &str = "5b121f51ad752b06ce0cb0ea17d5859f4b20c645";

fn stamp() -> String {
    format!(
        r#"{{
  "format": 0,
  "repo": "https://github.com/LeagueToolkit/ltk-census-history.git",
  "head": "{COMMIT}",
  "newest_build": "16.19.8207193",
  "builds": 1,
  "families": ["files", "objects", "clips", "rigs", "textures"],
  "writer": "0.1.0",
  "rows": {{}},
  "unparsed_texts": 0
}}"#
    )
}

fn builds() -> String {
    format!("0\t{COMMIT}\t16.19.8207193\te39c38647162cf28\t2026-09-30\n")
}

/// An empty database, as `.zst`: SQLite reads an empty file as a database of no tables.
fn db_zst() -> Vec<u8> {
    zstd::encode_all(&b""[..], 19).unwrap()
}

/// The empty database followed by a skippable frame of `len` bytes, which zstd reads past: a
/// file a download takes many reads of.
fn long_db_zst(len: u32) -> Vec<u8> {
    let mut bytes = db_zst();
    bytes.extend_from_slice(&0x184D_2A50_u32.to_le_bytes());
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend(std::iter::repeat_n(0x5a, len as usize));
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The files of a release in a folder, served by their URLs, which are their names, from any
/// place, under one entity tag.
struct Folder {
    dir: TempDir,
    etag: Mutex<String>,
    /// Where each ranged read started.
    asked: Mutex<Vec<u64>>,
}

impl Folder {
    fn new() -> Self {
        let folder = Self {
            dir: tempfile::tempdir().unwrap(),
            etag: Mutex::new("\"one\"".to_string()),
            asked: Mutex::new(Vec::new()),
        };
        folder.put("stamp.json", stamp().as_bytes());
        folder.put("builds.tsv", builds().as_bytes());
        folder.put("history.sqlite.zst", &db_zst());
        folder
    }

    fn path(&self, name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(self.dir.path().join(name)).unwrap()
    }

    fn put(&self, name: &str, bytes: &[u8]) {
        fs::write(self.path(name), bytes).unwrap();
    }

    fn asset(&self, name: &str) -> Asset {
        let bytes = fs::read(self.path(name)).unwrap();
        Asset {
            name: name.to_string(),
            size: bytes.len() as u64,
            sha256: sha256(&bytes),
            url: name.to_string(),
        }
    }

    fn release(&self) -> IndexRelease {
        IndexRelease {
            tag: "history-v2-index-16.19".to_string(),
            stamp: self.asset("stamp.json"),
            builds: self.asset("builds.tsv"),
            db: self.asset("history.sqlite.zst"),
            archive: None,
        }
    }

    fn root(&self) -> Utf8PathBuf {
        self.path("indexes")
    }

    fn downloads(&self) -> Utf8PathBuf {
        self.path("downloads")
    }
}

impl Source for Folder {
    fn open(&self, url: &str) -> Result<Box<dyn Read + Send>, AppError> {
        Ok(Box::new(fs::File::open(self.path(url)).unwrap()))
    }

    fn open_from(&self, url: &str, from: u64) -> Result<Ranged, AppError> {
        self.asked.lock().unwrap().push(from);
        let mut file = fs::File::open(self.path(url)).unwrap();
        file.seek(SeekFrom::Start(from)).unwrap();
        Ok(Ranged {
            reader: Box::new(file),
            start: from,
            etag: Some(self.etag.lock().unwrap().clone()),
        })
    }
}

fn install(
    folder: &Folder,
    release: &IndexRelease,
    cancel: &AtomicBool,
) -> Result<Utf8PathBuf, AppError> {
    install_stopping(folder, release, cancel, None)
}

/// Installs, and sets `cancel` once the database's download holds `stop_after` bytes.
fn install_stopping(
    folder: &Folder,
    release: &IndexRelease,
    cancel: &AtomicBool,
    stop_after: Option<u64>,
) -> Result<Utf8PathBuf, AppError> {
    let mut last = None;
    let mut progress = |seen: InstallProgress| {
        if seen.step == InstallStep::Download && stop_after.is_some_and(|at| seen.done >= at) {
            cancel.store(true, Ordering::Relaxed);
        }
        last = Some(seen);
    };
    let result = Installer::new(folder, cancel, &mut progress).install(
        release,
        &folder.root(),
        &folder.downloads(),
    );
    // An install that downloaded reports the whole of its last step done at its end.
    if let (Ok(_), Some(last)) = (&result, last) {
        assert_eq!(last.done, last.total, "the last step was done whole");
    }
    result
}

/// Installs, and answers what the install reported.
fn install_seen(
    folder: &Folder,
    release: &IndexRelease,
) -> (Result<Utf8PathBuf, AppError>, Vec<InstallProgress>) {
    let mut seen = Vec::new();
    let mut progress = |report: InstallProgress| seen.push(report);
    let result = Installer::new(folder, &AtomicBool::new(false), &mut progress).install(
        release,
        &folder.root(),
        &folder.downloads(),
    );
    (result, seen)
}

fn entries(dir: &Utf8Path) -> Vec<String> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn api_asset(name: &str, digest: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "size": 10,
        "digest": digest,
        "browser_download_url": format!("https://example.invalid/{name}"),
    })
}

fn api_release(tag: &str, draft: bool, files: &[(&str, Option<&str>)]) -> serde_json::Value {
    let assets: Vec<_> = files
        .iter()
        .map(|(name, digest)| api_asset(name, *digest))
        .collect();
    serde_json::json!({ "tag_name": tag, "draft": draft, "assets": assets })
}

const WHOLE: [(&str, Option<&str>); 3] = [
    ("stamp.json", Some("sha256:AA")),
    ("builds.tsv", Some("sha256:bb")),
    ("history.sqlite.zst", Some("sha256:cc")),
];

#[test]
fn the_newest_release_holding_a_whole_index_is_picked() {
    let releases = serde_json::json!([
        api_release("draft", true, &WHOLE),
        api_release("no-builds", false, &[WHOLE[0], WHOLE[2]]),
        api_release(
            "no-digest",
            false,
            &[WHOLE[0], WHOLE[1], ("history.sqlite.zst", None)]
        ),
        api_release("whole", false, &WHOLE),
        api_release("older", false, &WHOLE),
    ]);

    let picked = index::pick(&releases.to_string())
        .unwrap()
        .expect("a release");

    assert_eq!(picked.tag, "whole");
    assert_eq!(
        picked.stamp.sha256, "aa",
        "a digest is kept as lower-case hex"
    );
    assert_eq!(picked.db.url, "https://example.invalid/history.sqlite.zst");
}

#[test]
fn no_release_holding_an_index_picks_none() {
    let releases = serde_json::json!([api_release(
        "code",
        false,
        &[("app.zip", Some("sha256:aa"))]
    )]);

    assert_eq!(index::pick(&releases.to_string()).unwrap(), None);
}

#[test]
fn a_list_that_does_not_read_is_a_fetch_error() {
    assert!(matches!(
        index::pick("<html>rate limited</html>"),
        Err(AppError::IndexFetch { .. })
    ));
}

#[test]
fn a_whole_release_installs_an_index_that_opens_in_a_folder_named_for_its_tag() {
    let folder = Folder::new();

    let installed = install(&folder, &folder.release(), &AtomicBool::new(false)).unwrap();

    assert_eq!(installed, folder.root().join("history-v2-index-16.19"));
    let history = SqliteHistory::open(&installed).unwrap();
    assert_eq!(
        history.stamp().map(|stamp| stamp.head.as_str()),
        Some(COMMIT)
    );
    assert_eq!(entries(&folder.root()), ["history-v2-index-16.19"]);
}

#[test]
fn a_file_with_other_bytes_than_the_release_lists_leaves_no_index() {
    let folder = Folder::new();
    let release = folder.release();
    folder.put("builds.tsv", b"0\tsomething else\n");

    let Err(AppError::IndexDigest { file }) = install(&folder, &release, &AtomicBool::new(false))
    else {
        panic!("a file of other bytes was taken");
    };

    assert_eq!(file, "builds.tsv");
    assert!(
        entries(&folder.root()).is_empty(),
        "{:?}",
        entries(&folder.root())
    );
}

#[test]
fn a_cancelled_install_leaves_no_index() {
    let folder = Folder::new();
    let cancel = AtomicBool::new(false);
    cancel.store(true, Ordering::Relaxed);

    let result = install(&folder, &folder.release(), &cancel);

    assert!(matches!(result, Err(AppError::Cancelled)), "{result:?}");
    assert!(entries(&folder.root()).is_empty());
}

#[test]
fn an_index_installed_already_is_not_downloaded_again() {
    let folder = Folder::new();
    let release = folder.release();
    install(&folder, &release, &AtomicBool::new(false)).unwrap();
    // The files are gone from the source: a second download would fail.
    fs::remove_file(folder.path("history.sqlite.zst")).unwrap();

    let installed = install(&folder, &release, &AtomicBool::new(false));

    assert_eq!(
        installed.unwrap(),
        folder.root().join("history-v2-index-16.19")
    );
}

#[test]
fn a_database_download_that_stopped_resumes_from_where_it_stopped() {
    let folder = Folder::new();
    folder.put("history.sqlite.zst", &long_db_zst(400_000));
    let release = folder.release();
    let stopped = install_stopping(&folder, &release, &AtomicBool::new(false), Some(100_000));
    assert!(matches!(stopped, Err(AppError::Cancelled)), "{stopped:?}");

    let installed = install(&folder, &release, &AtomicBool::new(false)).unwrap();

    assert_eq!(installed, folder.root().join("history-v2-index-16.19"));
    let asked = folder.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(
        asked[1] >= 100_000,
        "the second download resumed: {asked:?}"
    );
    assert!(
        entries(&folder.downloads()).is_empty(),
        "the download is removed"
    );
}

#[test]
fn a_download_is_checked_as_it_comes_with_no_step_of_its_own() {
    let folder = Folder::new();
    folder.put("history.sqlite.zst", &long_db_zst(400_000));

    let (installed, seen) = install_seen(&folder, &folder.release());

    installed.unwrap();
    assert!(
        seen.iter().all(|report| report.step != InstallStep::Check),
        "{:?}",
        seen.iter().find(|report| report.step == InstallStep::Check)
    );
}

#[test]
fn a_resumed_download_checks_the_bytes_it_kept_before_the_rest() {
    let folder = Folder::new();
    folder.put("history.sqlite.zst", &long_db_zst(400_000));
    let release = folder.release();
    install_stopping(&folder, &release, &AtomicBool::new(false), Some(100_000)).unwrap_err();

    let (installed, seen) = install_seen(&folder, &release);

    installed.unwrap();
    let kept = folder.asked.lock().unwrap()[1];
    let checked = seen
        .iter()
        .take_while(|report| report.step == InstallStep::Check)
        .count();
    assert!(checked > 0, "the kept bytes were checked first: {seen:?}");
    let last = seen[checked - 1];
    assert_eq!((last.done, last.total), (kept, kept), "all the kept bytes");
    assert!(
        seen[checked..]
            .iter()
            .all(|report| report.step != InstallStep::Check),
        "nothing else was read again"
    );
    assert_eq!(seen[checked].step, InstallStep::Download);
    assert_eq!(seen[checked].done, kept, "the download went on from them");
}

#[test]
fn a_database_download_of_another_version_starts_again() {
    let folder = Folder::new();
    folder.put("history.sqlite.zst", &long_db_zst(400_000));
    let release = folder.release();
    install_stopping(&folder, &release, &AtomicBool::new(false), Some(100_000)).unwrap_err();
    *folder.etag.lock().unwrap() = "\"two\"".to_string();

    install(&folder, &release, &AtomicBool::new(false)).unwrap();

    let asked = folder.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert_eq!(asked[2], 0, "the download started again: {asked:?}");
}

#[test]
fn a_database_of_other_bytes_than_the_release_lists_is_not_kept() {
    let folder = Folder::new();
    let release = folder.release();
    let mut other = db_zst();
    *other.last_mut().unwrap() ^= 0xff;
    folder.put("history.sqlite.zst", &other);

    let Err(AppError::IndexDigest { file }) = install(&folder, &release, &AtomicBool::new(false))
    else {
        panic!("a database of other bytes was taken");
    };

    assert_eq!(file, "history.sqlite.zst");
    assert!(entries(&folder.root()).is_empty());
    assert!(
        entries(&folder.downloads()).is_empty(),
        "{:?}",
        entries(&folder.downloads())
    );
}

/// The newest published index installed in a scratch folder and opened. Its database is
/// compressed with a window of 2 GiB, which the unpacking takes.
#[test]
#[ignore = "downloads the published index, some 770 MB, and unpacks it, 2.75 GB"]
fn the_published_index_installs() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().join("indexes")).unwrap();
    let web = reforge_app::index::Web::new().unwrap();
    let release = web.latest().unwrap().expect("a release holds an index");
    let mut progress = |_| {};
    let started = std::time::Instant::now();
    let installed = Installer::new(&web, &AtomicBool::new(false), &mut progress)
        .install(&release, &root, &root.join("downloads"))
        .unwrap();
    eprintln!("{} installed in {:.0?}", release.tag, started.elapsed());
    let history = SqliteHistory::open(&installed).unwrap();
    let stamp = history.stamp().expect("a stamp");
    eprintln!("{} builds, up to {}", stamp.builds, stamp.newest_build);
}

#[test]
fn a_tag_becomes_a_folder_name_with_no_separator() {
    assert_eq!(index::folder_name("a/b\\c:d v1.2"), "a_b_c_d_v1.2");
}
