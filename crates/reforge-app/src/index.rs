//! The prebuilt history index, fetched from a release of the census history and unpacked on
//! the user's machine (#ADR-0047; `docs/design/ROADMAP-UX.md`, "The history index").
//!
//! A release that holds an index holds its three files: `stamp.json`, `builds.tsv` and
//! `history.sqlite.xz`, each with the SHA-256 GitHub lists for it. Every byte is checked
//! against it: the database is unpacked as it downloads, and the copy is thrown away when its
//! digest is not the release's. An index is installed in a folder named for its release, and
//! opened before the settings point at it, so an index the app holds open is never replaced.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use liblzma::read::XzDecoder;
use reforge::history::{BUILDS_FILE, DB_FILE, STAMP_FILE, SqliteHistory};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AppError, chain};

/// The releases of the census history, newest first.
pub const RELEASES: &str = "https://api.github.com/repos/LeagueToolkit/ltk-census-history/releases";

/// The database as a release holds it.
pub const DB_XZ_FILE: &str = "history.sqlite.xz";

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

/// A release that holds an index.
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

    fn index(&self) -> Option<IndexRelease> {
        if self.draft {
            return None;
        }
        Some(IndexRelease {
            tag: self.tag_name.clone(),
            stamp: self.asset(STAMP_FILE)?,
            builds: self.asset(BUILDS_FILE)?,
            db: self.asset(DB_XZ_FILE)?,
        })
    }
}

/// The newest release in GitHub's list of releases that holds a whole index, each file with
/// its digest. None when no release does.
pub fn pick(releases_json: &str) -> Result<Option<IndexRelease>, AppError> {
    let releases: Vec<ApiRelease> =
        serde_json::from_str(releases_json).map_err(|error| AppError::IndexFetch {
            detail: format!("the list of releases does not read: {error}"),
        })?;
    Ok(releases.iter().find_map(ApiRelease::index))
}

/// Where the files of a release are read from: the web, or a test's folder.
pub trait Source {
    /// The bytes at a URL.
    fn open(&self, url: &str) -> Result<Box<dyn Read + Send>, AppError>;
}

/// The web, as GitHub serves a release.
pub struct Web {
    client: reqwest::blocking::Client,
}

impl Web {
    /// A client that names the app, as GitHub's API asks.
    pub fn new() -> Result<Self, AppError> {
        let agent = concat!("reforge-app/", env!("CARGO_PKG_VERSION"));
        let client = reqwest::blocking::Client::builder()
            .user_agent(agent)
            // A download of 900 MB takes as long as it takes.
            .timeout(None)
            .build()
            .map_err(|error| AppError::IndexFetch {
                detail: chain(&error),
            })?;
        Ok(Self { client })
    }

    /// The newest release that holds an index.
    pub fn latest(&self) -> Result<Option<IndexRelease>, AppError> {
        let mut text = String::new();
        self.open(RELEASES)?
            .read_to_string(&mut text)
            .map_err(|error| AppError::IndexFetch {
                detail: error.to_string(),
            })?;
        pick(&text)
    }
}

impl Source for Web {
    fn open(&self, url: &str) -> Result<Box<dyn Read + Send>, AppError> {
        let fetch = |error: reqwest::Error| AppError::IndexFetch {
            detail: chain(&error),
        };
        let response = self.client.get(url).send().map_err(fetch)?;
        let response = response.error_for_status().map_err(fetch)?;
        Ok(Box::new(response))
    }
}

/// How far an install has got, for the window to show.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type, tauri_specta::Event,
)]
pub struct InstallProgress {
    /// The bytes of the release downloaded.
    pub done: u64,
    /// The bytes of the release in all.
    pub total: u64,
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

    /// Installs a release's index in `root/<tag>` and answers that folder. The folder is
    /// written beside as `<tag>.partial` and renamed once the index opens, so a folder of that
    /// name is a whole index.
    pub fn install(
        &mut self,
        release: &IndexRelease,
        root: &Utf8Path,
    ) -> Result<Utf8PathBuf, AppError> {
        let io = |error: io::Error| AppError::IndexUnwritable {
            detail: error.to_string(),
        };
        let name = folder_name(&release.tag);
        let target = root.join(&name);
        if target.join(STAMP_FILE).is_file() && SqliteHistory::open(&target).is_ok() {
            return Ok(target);
        }
        let partial = root.join(format!("{name}.partial"));
        if partial.exists() {
            fs::remove_dir_all(&partial).map_err(io)?;
        }
        fs::create_dir_all(&partial).map_err(io)?;

        self.total = release.stamp.size + release.builds.size + release.db.size;
        let written = self.write_all(release, &partial);
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
        Ok(target)
    }

    fn write_all(&mut self, release: &IndexRelease, dir: &Utf8Path) -> Result<(), AppError> {
        self.copy(&release.stamp, &dir.join(STAMP_FILE), false)?;
        self.copy(&release.builds, &dir.join(BUILDS_FILE), false)?;
        self.copy(&release.db, &dir.join(DB_FILE), true)
    }

    /// Downloads an asset to `path`, unpacking it as `.xz` when `unpack`, and checks its
    /// digest.
    fn copy(&mut self, asset: &Asset, path: &Utf8Path, unpack: bool) -> Result<(), AppError> {
        let reader = self.source.open(&asset.url)?;
        let mut out = fs::File::create(path).map_err(|error| AppError::IndexUnwritable {
            detail: error.to_string(),
        })?;
        let (start, total) = (self.done, self.total);
        let progress = &mut *self.progress;
        let mut report = |read: u64| {
            progress(InstallProgress {
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

        let result = if unpack {
            let mut decoder = XzDecoder::new(&mut checked);
            io::copy(&mut decoder, &mut out).and_then(|_| {
                // The stream may end before the file does; the digest is of the whole file.
                io::copy(decoder.get_mut(), &mut io::sink()).map(drop)
            })
        } else {
            io::copy(&mut checked, &mut out).map(drop)
        };
        if let Err(error) = result {
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
        source
            .open(&self.stamp.url)?
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
        })
    }
}

/// The folder a release's index is installed in: its tag, with nothing a path would read as a
/// separator.
pub fn folder_name(tag: &str) -> String {
    tag.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The index folders under `root` other than `keep`, removed. A folder that does not go, as
/// one an earlier run still holds open, is left for the next time.
pub fn remove_others(root: &Utf8Path, keep: &Utf8Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(path) = Utf8PathBuf::from_path_buf(entry.path()) else {
            continue;
        };
        if path != keep && path.is_dir() {
            let _ = fs::remove_dir_all(&path);
        }
    }
}
