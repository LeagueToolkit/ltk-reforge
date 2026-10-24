//! The prebuilt history index, fetched from a release of the census history and unpacked on
//! the user's machine (#ADR-0047; `docs/design/HISTORY.md`, "Distribution").
//!
//! A release holds the three files of an index for each format published, named with the
//! format ([`ReleaseNames`]), each with the SHA-256 GitHub lists for it, and may hold the census
//! history as an archive (#ADR-0060). The app reads only its own format's. Every byte is
//! checked against its digest: the database downloads into a file that a later download
//! resumes, and is unpacked once it is whole. An index is installed in a folder named for its
//! release and format, and opened before the settings point at it, so an index the app holds
//! open is never replaced.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::{BUILDS_FILE, DB_FILE, FORMAT, STAMP_FILE, SqliteHistory};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zstd::stream::read::Decoder;

use crate::download::Download;
use crate::error::{AppError, chain};

/// The releases of the census history, newest first.
pub const RELEASES: &str = "https://api.github.com/repos/LeagueToolkit/ltk-census-history/releases";

/// The census history as a release holds it: a bare repository, in tar, in zstd.
pub const ARCHIVE_FILE: &str = "ltk-census-history.git.tar.zst";

/// The largest window, as a power of two, a frame of the database may ask for: 2 GiB, as
/// `zstd --long=31` writes it. Unpacking holds a window of memory.
const WINDOW_LOG_MAX: u32 = 31;

/// The names a release gives the files of an index of one format: each file's name in the
/// index's directory with `-v<format>` before its extension, and the database's compressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseNames {
    /// The stamp's, `stamp-v<format>.json`.
    pub stamp: String,
    /// The list of builds', `builds-v<format>.tsv`.
    pub builds: String,
    /// The database's, in zstd, `history-v<format>.sqlite.zst`.
    pub db: String,
}

impl ReleaseNames {
    /// The names of the files of an index of `format`.
    pub fn of(format: u32) -> Self {
        let versioned = |file: &str| match file.split_once('.') {
            Some((stem, extension)) => format!("{stem}-v{format}.{extension}"),
            None => format!("{file}-v{format}"),
        };
        Self {
            stamp: versioned(STAMP_FILE),
            builds: versioned(BUILDS_FILE),
            db: format!("{}.zst", versioned(DB_FILE)),
        }
    }
}

/// One file of a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// Its name.
    pub name: String,
    /// Its size in bytes.
    pub size: u64,
    /// Its SHA-256, as hex digits.
    pub sha256: String,
    /// Where it is downloaded from.
    pub url: String,
}

/// A release that holds an index of the app's format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRelease {
    /// The release's tag.
    pub tag: String,
    /// The index's stamp.
    pub stamp: Asset,
    /// Its list of builds.
    pub builds: Asset,
    /// Its database, compressed.
    pub db: Asset,
    /// The census history at the index's head, when the release holds it.
    pub archive: Option<Asset>,
    /// The delta that takes the index to the history's tip, when the release holds one
    /// (#ADR-0109).
    pub delta: Option<Asset>,
}

/// The name of a release's delta for the app's format, to the head `head`:
/// `history-v<format>-delta-<head12>.tar`.
pub fn delta_name(head: &str) -> String {
    format!(
        "history-v{FORMAT}-delta-{}.tar",
        head.get(..12).unwrap_or(head)
    )
}

/// The head a delta's name says it reaches, when the name is a delta's of the app's format.
pub fn delta_head(name: &str) -> Option<&str> {
    let head = name
        .strip_prefix(&format!("history-v{FORMAT}-delta-"))?
        .strip_suffix(".tar")?;
    (head.len() == 12 && head.bytes().all(|b| b.is_ascii_hexdigit())).then_some(head)
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    assets: Vec<ApiAsset>,
}

impl ApiRelease {
    fn asset(&self, name: &str) -> Option<Asset> {
        let asset = self.assets.iter().find(|asset| asset.name == name)?;
        let sha256 = asset.digest.as_deref()?.strip_prefix("sha256:")?;
        Some(Asset {
            name: asset.name.clone(),
            size: asset.size,
            sha256: sha256.to_ascii_lowercase(),
            url: asset.browser_download_url.clone(),
        })
    }

    fn index(&self, names: &ReleaseNames) -> Option<IndexRelease> {
        if self.draft {
            return None;
        }
        Some(IndexRelease {
            tag: self.tag_name.clone(),
            stamp: self.asset(&names.stamp)?,
            builds: self.asset(&names.builds)?,
            db: self.asset(&names.db)?,
            archive: self.asset(ARCHIVE_FILE),
            delta: self
                .assets
                .iter()
                .find(|asset| delta_head(&asset.name).is_some())
                .and_then(|asset| self.asset(&asset.name)),
        })
    }
}

/// The newest release in GitHub's list of releases that holds a whole index of the app's
/// format, each file with its digest. None when no release does: a release of other formats
/// only holds no index for the app.
pub fn pick(releases_json: &str) -> Result<Option<IndexRelease>, AppError> {
    let releases: Vec<ApiRelease> =
        serde_json::from_str(releases_json).map_err(|error| AppError::IndexFetch {
            detail: format!("the list of releases does not read: {error}"),
        })?;
    let names = ReleaseNames::of(FORMAT);
    Ok(releases.iter().find_map(|release| release.index(&names)))
}

/// Where the files of a release are read from: the web, or a test's folder.
pub trait Source {
    /// The bytes at a URL.
    fn open(&self, url: &str) -> Result<Box<dyn Read + Send>, AppError>;

    /// The bytes at a URL from `from` on, and what names this version of them. A source that
    /// cannot start there sends them from the start.
    fn open_from(&self, url: &str, from: u64) -> Result<Ranged, AppError> {
        let _ = from;
        Ok(Ranged {
            reader: self.open(url)?,
            start: 0,
            etag: None,
        })
    }
}

/// The bytes of a file from a place in it.
pub struct Ranged {
    /// The bytes.
    pub reader: Box<dyn Read + Send>,
    /// Where in the file they start: the place asked for, or 0 when the whole file was sent.
    pub start: u64,
    /// The entity tag of this version of the file, when the source names one.
    pub etag: Option<String>,
}

/// How long a connection may take to open, as LTK Manager allows.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// How long a check may take in all, as LTK Manager allows. The app checks unasked, where a
/// request that hangs is worse than one that gives up.
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);

/// How long a download may wait for its next bytes. A download of 900 MB takes as long as it
/// takes, but a connection that falls silent would otherwise hold the job, and its Stop
/// button, for good: a stop is seen between two reads.
const READ_TIMEOUT: Duration = Duration::from_secs(120);

/// The most bytes GitHub's list of releases is read to. A page of the list holds thirty
/// releases of a handful of files each, some hundred KB; a body past the bound is refused.
const RELEASES_MAX: u64 = 16 << 20;

/// The web, as GitHub serves a release.
pub struct Web {
    client: reqwest::blocking::Client,
}

impl Web {
    /// A client for downloads, which names the app, as GitHub's API asks.
    pub fn new() -> Result<Self, AppError> {
        // The blocking client's timeout bounds each send and each read of a body on its own, not
        // the whole download (reqwest 0.12, `blocking::Response` as `Read`), so it is the read
        // timeout, and the blocking builder has no other.
        Self::with_timeout(READ_TIMEOUT)
    }

    /// A client for checks, whose requests give up after 20 seconds.
    pub fn for_checks() -> Result<Self, AppError> {
        Self::with_timeout(CHECK_TIMEOUT)
    }

    fn with_timeout(timeout: Duration) -> Result<Self, AppError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("reforge-app/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(timeout)
            .build()
            .map_err(|error| AppError::IndexFetch {
                detail: chain(&error),
            })?;
        Ok(Self { client })
    }

    /// The client, for a request other than a plain read.
    pub(crate) fn client(&self) -> &reqwest::blocking::Client {
        &self.client
    }

    /// The newest release that holds an index of the app's format.
    pub fn latest(&self) -> Result<Option<IndexRelease>, AppError> {
        let mut text = String::new();
        self.open(RELEASES)?
            .take(RELEASES_MAX + 1)
            .read_to_string(&mut text)
            .map_err(|error| AppError::IndexFetch {
                detail: error.to_string(),
            })?;
        if text.len() as u64 > RELEASES_MAX {
            return Err(AppError::IndexFetch {
                detail: format!(
                    "the list of releases is larger than {} MB",
                    RELEASES_MAX >> 20
                ),
            });
        }
        pick(&text)
    }
}

impl Source for Web {
    fn open(&self, url: &str) -> Result<Box<dyn Read + Send>, AppError> {
        Ok(self.open_from(url, 0)?.reader)
    }

    fn open_from(&self, url: &str, from: u64) -> Result<Ranged, AppError> {
        let fetch = |error: reqwest::Error| AppError::IndexFetch {
            detail: chain(&error),
        };
        let mut request = self.client.get(url);
        if from > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={from}-"));
        }
        let response = request.send().map_err(fetch)?;
        let response = response.error_for_status().map_err(fetch)?;
        let partial = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|etag| etag.to_str().ok())
            .map(str::to_owned);
        Ok(Ranged {
            reader: Box::new(response),
            start: if partial { from } else { 0 },
            etag,
        })
    }
}

/// What a download of a release is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum InstallStep {
    /// What a download holds already is read again for its digest: the bytes a stopped download
    /// kept, before the rest is asked for, or a file downloaded whole before.
    Check,
    /// Its files are downloading.
    Download,
    /// What was downloaded is unpacking.
    Unpack,
}

/// How far an install has got, for the window to show.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type, tauri_specta::Event,
)]
pub struct InstallProgress {
    /// What the install is doing.
    pub step: InstallStep,
    /// The bytes done: of the download checked, of the release downloaded, or of the download
    /// unpacked.
    pub done: u64,
    /// The bytes to do.
    pub total: u64,
}

/// A reader that counts what passes through it, reports the count, and stops when asked.
pub(crate) struct Counted<'a, R> {
    inner: R,
    read: u64,
    cancel: &'a AtomicBool,
    report: &'a mut dyn FnMut(u64),
}

impl<'a, R> Counted<'a, R> {
    pub(crate) fn new(inner: R, cancel: &'a AtomicBool, report: &'a mut dyn FnMut(u64)) -> Self {
        Self {
            inner,
            read: 0,
            cancel,
            report,
        }
    }
}

impl<R: Read> Read for Counted<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other(Cancelled));
        }
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        (self.report)(self.read);
        Ok(n)
    }
}

/// A reader that hashes and counts what passes through it, reports the count, and stops when
/// asked.
struct Checked<'a, R> {
    inner: R,
    hasher: Sha256,
    read: u64,
    cancel: &'a AtomicBool,
    report: &'a mut dyn FnMut(u64),
}

impl<R: Read> Read for Checked<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            // Not `Interrupted`: `io::copy` retries that kind.
            return Err(io::Error::other(Cancelled));
        }
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        self.read += n as u64;
        (self.report)(self.read);
        Ok(n)
    }
}

#[derive(Debug)]
struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// Installs an index.
pub struct Installer<'a> {
    source: &'a dyn Source,
    cancel: &'a AtomicBool,
    progress: &'a mut dyn FnMut(InstallProgress),
    done: u64,
    total: u64,
}

impl<'a> Installer<'a> {
    /// An installer that reads from `source`, reports to `progress`, and stops when `cancel`
    /// is set.
    pub fn new(
        source: &'a dyn Source,
        cancel: &'a AtomicBool,
        progress: &'a mut dyn FnMut(InstallProgress),
    ) -> Self {
        Self {
            source,
            cancel,
            progress,
            done: 0,
            total: 0,
        }
    }

    /// Installs a release's index in `root/<tag>-v<format>` and answers that folder. The
    /// database downloads into `downloads` first, under the release's tag and the database's
    /// name, where a download that stops is kept for the next (#ADR-0060). The folder is written
    /// beside with `.partial` after its name and renamed once the index opens, so a folder of
    /// that name is a whole index.
    pub fn install(
        &mut self,
        release: &IndexRelease,
        root: &Utf8Path,
        downloads: &Utf8Path,
    ) -> Result<Utf8PathBuf, AppError> {
        let io = |error: io::Error| AppError::IndexUnwritable {
            detail: error.to_string(),
        };
        let name = folder_name(&release.tag);
        let target = root.join(&name);
        if target.join(STAMP_FILE).is_file() && SqliteHistory::open(&target).is_ok() {
            return Ok(target);
        }

        self.total = release.stamp.size + release.builds.size + release.db.size;
        let db = downloads.join(format!("{}-{}", path_safe(&release.tag), release.db.name));
        let (cancel, total) = (self.cancel, self.total);
        let progress = &mut *self.progress;
        // The database comes first, so its bytes downloaded are the release's.
        let mut heard = |seen: InstallProgress| match seen.step {
            InstallStep::Download => progress(InstallProgress { total, ..seen }),
            _ => progress(seen),
        };
        Download::new(self.source, cancel, &mut heard, |detail| {
            AppError::IndexFetch { detail }
        })
        .fetch(&release.db, &db)?;
        self.done = release.db.size;

        let partial = root.join(format!("{name}.partial"));
        if partial.exists() {
            fs::remove_dir_all(&partial).map_err(io)?;
        }
        fs::create_dir_all(&partial).map_err(io)?;
        let written = self.write_all(release, &partial, &db);
        if let Err(error) = written {
            // A partial index is no index; nothing of it is kept.
            let _ = fs::remove_dir_all(&partial);
            return Err(error);
        }

        SqliteHistory::open(&partial).map_err(|error| AppError::IndexUnreadable {
            detail: chain(&error),
        })?;
        if target.exists() {
            fs::remove_dir_all(&target).map_err(io)?;
        }
        fs::rename(&partial, &target).map_err(io)?;
        let _ = fs::remove_file(&db);
        Ok(target)
    }

    fn write_all(
        &mut self,
        release: &IndexRelease,
        dir: &Utf8Path,
        db: &Utf8Path,
    ) -> Result<(), AppError> {
        self.copy(&release.stamp, &dir.join(STAMP_FILE))?;
        self.copy(&release.builds, &dir.join(BUILDS_FILE))?;
        self.unpack(db, &dir.join(DB_FILE))
    }

    /// Unpacks the database downloaded at `from` into `to`.
    fn unpack(&mut self, from: &Utf8Path, to: &Utf8Path) -> Result<(), AppError> {
        let unwritable = |error: io::Error| AppError::IndexUnwritable {
            detail: error.to_string(),
        };
        let file = fs::File::open(from).map_err(unwritable)?;
        let total = file.metadata().map_err(unwritable)?.len();
        let mut out = fs::File::create(to).map_err(unwritable)?;
        let progress = &mut *self.progress;
        let mut report = |done: u64| {
            progress(InstallProgress {
                step: InstallStep::Unpack,
                done,
                total,
            })
        };
        let mut counted = Counted::new(file, self.cancel, &mut report);
        if let Err(error) = unpack_zst(&mut counted, &mut out) {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(AppError::Cancelled);
            }
            return Err(AppError::IndexUnreadable {
                detail: error.to_string(),
            });
        }
        out.flush().map_err(unwritable)
    }

    /// Downloads an asset to `path` and checks its digest.
    fn copy(&mut self, asset: &Asset, path: &Utf8Path) -> Result<(), AppError> {
        let reader = self.source.open(&asset.url)?;
        let mut out = fs::File::create(path).map_err(|error| AppError::IndexUnwritable {
            detail: error.to_string(),
        })?;
        let (start, total) = (self.done, self.total);
        let progress = &mut *self.progress;
        let mut report = |read: u64| {
            progress(InstallProgress {
                step: InstallStep::Download,
                done: start + read,
                total,
            })
        };
        let mut checked = Checked {
            inner: reader,
            hasher: Sha256::new(),
            read: 0,
            cancel: self.cancel,
            report: &mut report,
        };

        if let Err(error) = io::copy(&mut checked, &mut out) {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(AppError::Cancelled);
            }
            return Err(AppError::IndexFetch {
                detail: format!("{}: {error}", asset.name),
            });
        }
        out.flush().map_err(|error| AppError::IndexUnwritable {
            detail: error.to_string(),
        })?;

        let read = checked.read;
        let digest = format!("{:x}", checked.hasher.finalize());
        self.done = start + read;
        if digest != asset.sha256 {
            return Err(AppError::IndexDigest {
                file: asset.name.clone(),
            });
        }
        Ok(())
    }
}

/// The zstd frames read from `from`, unpacked, with a window of up to 2 GiB.
pub(crate) fn zst_reader<R: Read>(from: R) -> io::Result<Decoder<'static, io::BufReader<R>>> {
    let mut decoder = Decoder::new(from)?;
    decoder.window_log_max(WINDOW_LOG_MAX)?;
    Ok(decoder)
}

/// Unpacks the zstd frames read from `from` into `to`.
fn unpack_zst(from: &mut impl Read, to: &mut impl Write) -> io::Result<()> {
    io::copy(&mut zst_reader(from)?, to).map(drop)
}

/// A release that holds an index, as the inputs page offers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IndexReleaseView {
    /// The release's tag.
    pub tag: String,
    /// The bytes to download.
    pub size: u64,
    /// The commit of the build the index took in last.
    pub head: String,
    /// The newest build it holds.
    pub newest_build: String,
    /// How many builds it holds.
    pub builds: usize,
    /// The bytes of the census history's archive, when the release holds it.
    pub archive_size: Option<u64>,
    /// The head the release's delta reaches, when it holds one (#ADR-0109).
    pub delta_head: Option<String>,
    /// The bytes of that delta.
    pub delta_size: Option<u64>,
}

#[derive(Deserialize)]
struct StampHead {
    head: String,
    newest_build: String,
    builds: usize,
}

impl IndexRelease {
    /// The release as the inputs page offers it, its stamp read from `source`. The stamp is
    /// small, and checked against its digest as an install checks it.
    pub fn view(&self, source: &dyn Source) -> Result<IndexReleaseView, AppError> {
        let mut bytes = Vec::new();
        // One byte past the size the release lists fails the digest below.
        source
            .open(&self.stamp.url)?
            .take(self.stamp.size + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| AppError::IndexFetch {
                detail: error.to_string(),
            })?;
        if format!("{:x}", Sha256::digest(&bytes)) != self.stamp.sha256 {
            return Err(AppError::IndexDigest {
                file: self.stamp.name.clone(),
            });
        }
        let stamp: StampHead =
            serde_json::from_slice(&bytes).map_err(|error| AppError::IndexUnreadable {
                detail: error.to_string(),
            })?;
        Ok(IndexReleaseView {
            tag: self.tag.clone(),
            size: self.stamp.size + self.builds.size + self.db.size,
            head: stamp.head,
            newest_build: stamp.newest_build,
            builds: stamp.builds,
            archive_size: self.archive.as_ref().map(|archive| archive.size),
            delta_head: self
                .delta
                .as_ref()
                .and_then(|delta| delta_head(&delta.name))
                .map(str::to_string),
            delta_size: self.delta.as_ref().map(|delta| delta.size),
        })
    }
}

/// The folder an index of the app's format is installed in: `name`, with nothing a path would
/// read as a separator, and `-v<format>`. Two versions of the app that read two formats write
/// no folder in common.
pub fn folder_name(name: &str) -> String {
    format!("{}-v{FORMAT}", path_safe(name))
}

/// `text` with nothing a path would read as a separator.
pub(crate) fn path_safe(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The folder an index taken forward to `head` is written in.
pub fn head_folder(root: &Utf8Path, head: &str) -> Utf8PathBuf {
    let short = head.get(..12).unwrap_or(head);
    root.join(folder_name(&format!("head-{short}")))
}

/// The folders of indexes taken forward under `root` other than `keep`, removed. A release's
/// index stays: it is the base of the next delta (#ADR-0109). A folder that does not go, as
/// one an earlier run still holds open, is left for the next time.
pub fn remove_others(root: &Utf8Path, keep: &Utf8Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        let taken_forward = path
            .file_name()
            .is_some_and(|name| name.starts_with("head-"));
        if path != keep && path.is_dir() && taken_forward {
            let _ = fs::remove_dir_all(&path);
        }
    }
}
