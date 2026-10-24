//! The prebuilt history index: the release of the app's format picked from GitHub's list, its
//! files downloaded and checked against their digests, the database unpacked, and the index
//! installed only whole, in a folder of its release and format. The files are read from a
//! test's folder through the same `Source` the web is.

use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::{BUILDS_FILE, FORMAT, History, STAMP_FILE, SqliteHistory, Stamp};
use reforge_app::error::AppError;
use reforge_app::index::{
    self, Asset, IndexRelease, InstallProgress, InstallStep, Installer, Ranged, ReleaseNames,
    Source,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const COMMIT: &str = "5b121f51ad752b06ce0cb0ea17d5859f4b20c645";

const TAG: &str = "history-v2-index-16.19";

/// The format of another version of the app.
const OTHER: u32 = FORMAT + 1;

fn names() -> ReleaseNames {
    ReleaseNames::of(FORMAT)
}

/// The folder an index of the app's format from the release `TAG` is installed in.
fn installed_name() -> String {
    format!("{TAG}-v{FORMAT}")
}

fn stamp() -> String {
    stamp_of(FORMAT)
}

fn stamp_of(format: u32) -> String {
    format!(
        r#"{{
  "format": {format},
  "repo": "https://github.com/LeagueToolkit/ltk-census-history.git",
  "head": "{COMMIT}",
  "newest_build": "16.19.8207193",
  "builds": 1,
  "families": ["files", "objects", "clips", "rigs", "textures"],
  "writer": "0.1.0",
  "rows": {{}},
  "unparsed_texts": 0,
  "objectless_texts": 0,
  "objects": {{ "level": 19, "keyframe_every": 8, "dict_cap": 1048576, "zstd": "1.5.7" }}
}}"#
    )
}

fn builds() -> String {
    format!("0\t{COMMIT}\t16.19.8207193\te39c38647162cf28\t2026-09-30\t16.19\trman\t\t\t\t0\tNA1\n")
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
        let names = names();
        folder.put(&names.stamp, stamp().as_bytes());
        folder.put(&names.builds, builds().as_bytes());
        folder.put(&names.db, &db_zst());
        folder
    }

    /// The files of an index of another version of the app beside the app's: a stamp of that
    /// format, and a list of builds and a database the app's reader would not open.
    fn put_other_format(&self) {
        let names = ReleaseNames::of(OTHER);
        self.put(&names.stamp, stamp_of(OTHER).as_bytes());
        self.put(&names.builds, b"another format's builds\n");
        self.put(&names.db, b"another format's database");
    }

    /// GitHub's list of releases, one release `tag` holding the files `names` of this folder,
    /// each with its size and digest, at a URL that is its name.
    fn releases(&self, tag: &str, names: &[&str]) -> String {
        let assets: Vec<_> = names
            .iter()
            .map(|name| {
                let asset = self.asset(name);
                serde_json::json!({
                    "name": asset.name,
                    "size": asset.size,
                    "digest": format!("sha256:{}", asset.sha256),
                    "browser_download_url": asset.url,
                })
            })
            .collect();
        serde_json::json!([{ "tag_name": tag, "draft": false, "assets": assets }]).to_string()
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
        let names = names();
        IndexRelease {
            tag: TAG.to_string(),
            stamp: self.asset(&names.stamp),
            builds: self.asset(&names.builds),
            db: self.asset(&names.db),
            archive: None,
            delta: None,
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

fn api_release(tag: &str, draft: bool, files: &[(String, Option<&str>)]) -> serde_json::Value {
    let assets: Vec<_> = files
        .iter()
        .map(|(name, digest)| api_asset(name, *digest))
        .collect();
    serde_json::json!({ "tag_name": tag, "draft": draft, "assets": assets })
}

/// The three files of an index of `format`, each with a digest.
fn whole(format: u32) -> [(String, Option<&'static str>); 3] {
    let names = ReleaseNames::of(format);
    [
        (names.stamp, Some("sha256:AA")),
        (names.builds, Some("sha256:bb")),
        (names.db, Some("sha256:cc")),
    ]
}

#[test]
fn a_release_names_each_file_of_an_index_with_its_format() {
    let names = ReleaseNames::of(7);

    assert_eq!(names.stamp, "stamp-v7.json");
    assert_eq!(names.builds, "builds-v7.tsv");
    assert_eq!(names.db, "history-v7.sqlite.zst");
}

#[test]
fn the_newest_release_holding_a_whole_index_is_picked() {
    let [stamp, builds, db] = whole(FORMAT);
    let releases = serde_json::json!([
        api_release("draft", true, &whole(FORMAT)),
        api_release("no-builds", false, &[stamp.clone(), db.clone()]),
        api_release("no-digest", false, &[stamp, builds, (db.0, None)]),
        api_release("other-format", false, &whole(OTHER)),
        api_release("whole", false, &whole(FORMAT)),
        api_release("older", false, &whole(FORMAT)),
    ]);

    let picked = index::pick(&releases.to_string())
        .unwrap()
        .expect("a release");

    assert_eq!(picked.tag, "whole");
    assert_eq!(
        picked.stamp.sha256, "aa",
        "a digest is kept as lower-case hex"
    );
    assert_eq!(
        picked.db.url,
        format!("https://example.invalid/{}", names().db)
    );
}

/// #ADR-0109: a release's delta is the asset named for the app's format and a head.
#[test]
fn a_release_holding_a_delta_of_the_apps_format_is_picked_with_it() {
    let head = "5970301fb64afa82f106d3d69dd9550c8da65e22";
    let name = index::delta_name(head);
    assert_eq!(name, format!("history-v{FORMAT}-delta-5970301fb64a.tar"));
    assert_eq!(index::delta_head(&name), Some("5970301fb64a"));
    assert_eq!(
        index::delta_head(&format!("history-v{OTHER}-delta-5970301fb64a.tar")),
        None
    );
    assert_eq!(index::delta_head("history-v1-delta-nothex000000.tar"), None);
    let mut files = whole(FORMAT).to_vec();
    files.push((
        format!("history-v{OTHER}-delta-aaaaaaaaaaaa.tar"),
        Some("sha256:ee"),
    ));
    files.push((name.clone(), Some("sha256:dd")));
    let releases = serde_json::json!([api_release("whole", false, &files)]);

    let picked = index::pick(&releases.to_string())
        .unwrap()
        .expect("a release");

    let delta = picked.delta.expect("the delta");
    assert_eq!((delta.name, delta.sha256.as_str()), (name, "dd"));
    let without = serde_json::json!([api_release("whole", false, &whole(FORMAT))]);
    assert_eq!(
        index::pick(&without.to_string()).unwrap().unwrap().delta,
        None
    );
}

#[test]
fn no_release_holding_an_index_picks_none() {
    let releases = serde_json::json!([api_release(
        "code",
        false,
        &[("app.zip".to_string(), Some("sha256:aa"))]
    )]);

    assert_eq!(index::pick(&releases.to_string()).unwrap(), None);
}

#[test]
fn a_release_of_other_formats_only_holds_no_index_for_the_app() {
    let mut files = whole(OTHER).to_vec();
    // The names a release gave the files before they carried a format.
    files.extend([
        (STAMP_FILE.to_string(), Some("sha256:aa")),
        (BUILDS_FILE.to_string(), Some("sha256:bb")),
        ("history.sqlite.zst".to_string(), Some("sha256:cc")),
    ]);
    let releases = serde_json::json!([api_release("next", false, &files)]);

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
fn a_release_of_the_apps_format_installs_an_index_that_opens_in_a_folder_of_its_tag_and_format() {
    let folder = Folder::new();
    let names = names();
    let releases = folder.releases(TAG, &[&names.stamp, &names.builds, &names.db]);
    let release = index::pick(&releases).unwrap().expect("a release");

    let installed = install(&folder, &release, &AtomicBool::new(false)).unwrap();

    assert_eq!(installed, folder.root().join(installed_name()));
    let history = SqliteHistory::open(&installed).unwrap();
    assert_eq!(
        history.stamp().map(|stamp| stamp.head.as_str()),
        Some(COMMIT)
    );
    assert_eq!(entries(&folder.root()), [installed_name()]);
    assert_eq!(
        entries(&installed),
        ["builds.tsv", "history.sqlite", "stamp.json"],
        "the files keep the names of an index's directory"
    );
}

#[test]
fn a_release_of_two_formats_installs_the_files_of_the_apps() {
    let folder = Folder::new();
    folder.put_other_format();
    let (ours, other) = (names(), ReleaseNames::of(OTHER));
    let releases = folder.releases(
        TAG,
        &[
            &other.stamp,
            &other.builds,
            &other.db,
            &ours.stamp,
            &ours.builds,
            &ours.db,
        ],
    );
    let release = index::pick(&releases).unwrap().expect("a release");

    let installed = install(&folder, &release, &AtomicBool::new(false)).unwrap();

    assert_eq!(installed, folder.root().join(installed_name()));
    assert_eq!(Stamp::read(&installed).unwrap().format, FORMAT);
    assert_eq!(
        fs::read_to_string(installed.join(BUILDS_FILE)).unwrap(),
        builds()
    );
    SqliteHistory::open(&installed).unwrap();
}

#[test]
fn a_file_with_other_bytes_than_the_release_lists_leaves_no_index() {
    let folder = Folder::new();
    let release = folder.release();
    folder.put(&names().builds, b"0\tsomething else\n");

    let Err(AppError::IndexDigest { file }) = install(&folder, &release, &AtomicBool::new(false))
    else {
        panic!("a file of other bytes was taken");
    };

    assert_eq!(file, names().builds);
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
    fs::remove_file(folder.path(&names().db)).unwrap();

    let installed = install(&folder, &release, &AtomicBool::new(false));

    assert_eq!(installed.unwrap(), folder.root().join(installed_name()));
}

#[test]
fn a_database_download_that_stopped_resumes_from_where_it_stopped() {
    let folder = Folder::new();
    folder.put(&names().db, &long_db_zst(400_000));
    let release = folder.release();
    let stopped = install_stopping(&folder, &release, &AtomicBool::new(false), Some(100_000));
    assert!(matches!(stopped, Err(AppError::Cancelled)), "{stopped:?}");

    let installed = install(&folder, &release, &AtomicBool::new(false)).unwrap();

    assert_eq!(installed, folder.root().join(installed_name()));
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
    folder.put(&names().db, &long_db_zst(400_000));

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
    folder.put(&names().db, &long_db_zst(400_000));
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
    folder.put(&names().db, &long_db_zst(400_000));
    let release = folder.release();
    install_stopping(&folder, &release, &AtomicBool::new(false), Some(100_000)).unwrap_err();
    *folder.etag.lock().unwrap() = "\"two\"".to_string();

    install(&folder, &release, &AtomicBool::new(false)).unwrap();

    let asked = folder.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 3, "{asked:?}");
    assert_eq!(asked[2], 0, "the download started again: {asked:?}");
}

#[test]
fn a_database_under_another_name_downloads_from_the_start() {
    let folder = Folder::new();
    let bytes = long_db_zst(400_000);
    folder.put(&names().db, &bytes);
    // The same bytes, size and digest under the name a release gave them before they carried a
    // format.
    folder.put("history.sqlite.zst", &bytes);
    let unformatted = IndexRelease {
        db: folder.asset("history.sqlite.zst"),
        ..folder.release()
    };
    install_stopping(
        &folder,
        &unformatted,
        &AtomicBool::new(false),
        Some(100_000),
    )
    .unwrap_err();

    install(&folder, &folder.release(), &AtomicBool::new(false)).unwrap();

    let asked = folder.asked.lock().unwrap().clone();
    assert_eq!(asked, [0, 0], "the download started from the start");
}

#[test]
fn a_database_of_other_bytes_than_the_release_lists_is_not_kept() {
    let folder = Folder::new();
    let release = folder.release();
    let mut other = db_zst();
    *other.last_mut().unwrap() ^= 0xff;
    folder.put(&names().db, &other);

    let Err(AppError::IndexDigest { file }) = install(&folder, &release, &AtomicBool::new(false))
    else {
        panic!("a database of other bytes was taken");
    };

    assert_eq!(file, names().db);
    assert!(entries(&folder.root()).is_empty());
    assert!(
        entries(&folder.downloads()).is_empty(),
        "{:?}",
        entries(&folder.downloads())
    );
}

/// The newest published index of the app's format installed in a scratch folder and opened.
/// Its database is compressed with a window of 2 GiB, which the unpacking takes.
#[test]
#[ignore = "downloads the published index, some 1.7 GB, and unpacks it, 4.8 GB"]
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
    eprintln!(
        "{} ({}) installed in {:.0?} as {}",
        release.tag,
        release.db.name,
        started.elapsed(),
        installed.file_name().unwrap()
    );
    assert_eq!(installed, root.join(index::folder_name(&release.tag)));
    let history = SqliteHistory::open(&installed).unwrap();
    let stamp = history.stamp().expect("a stamp");
    eprintln!("{} builds, up to {}", stamp.builds, stamp.newest_build);
}

#[test]
fn a_tag_becomes_a_folder_name_with_no_separator_and_the_format() {
    assert_eq!(
        index::folder_name("a/b\\c:d v1.2"),
        format!("a_b_c_d_v1.2-v{FORMAT}")
    );
}
