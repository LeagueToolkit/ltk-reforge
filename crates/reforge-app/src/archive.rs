//! The app's whole clone of the census history, bootstrapped from a release's archive
//! (#ADR-0060). The archive downloads into the app's data folder, where a download that stops is
//! kept for the next, and is read as a stream: the pack's files are written into a new bare clone
//! under names built from the pack's hash, `packed-refs` gives the builds' branch, and nothing
//! else of the archive is written. The new clone takes the place of the whole clone held, once it
//! holds that commit, and the clone fetched for updates (#ADR-0057) is removed: updates fetch
//! into the whole clone from then on.

use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge_census::{CensusError, Seed, format};
use serde::{Deserialize, Serialize};

use crate::download::Download;
use crate::error::{AppError, chain};
use crate::index::{Counted, IndexRelease, InstallStep, Source, folder_name, zst_reader};

/// The file a clone bootstrapped from an archive notes it in.
pub const ARCHIVE_NOTE: &str = "archive.json";

/// The files of a pack the archive holds that the clone takes.
const PACK_FILES: [&str; 4] = ["pack", "idx", "rev", "keep"];

/// The most of `packed-refs` read: the archive's lists a branch or two and a tag.
const REFS_MAX: u64 = 1 << 20;

/// What a clone bootstrapped from an archive was made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveNote {
    /// The release's tag.
    pub tag: String,
    /// The commit the builds' branch was at in the archive.
    pub head: String,
}

/// The app's clone of the census history, as the inputs page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CloneView {
    /// No clone.
    None,
    /// A clone fetched as far as appends read it (#ADR-0057).
    Partial,
    /// A clone of the whole history, bootstrapped from a release's archive.
    Whole {
        /// The release the archive was of.
        tag: String,
    },
}

/// The app's clones of the census history, each in a folder of its own: the whole one a
/// bootstrap makes, and the one updates fetch as far as they read while there is no whole one.
#[derive(Debug, Clone)]
pub struct Clones {
    /// The whole clone's folder.
    pub whole: Utf8PathBuf,
    /// The fetched clone's folder.
    pub partial: Utf8PathBuf,
}

impl Clones {
    /// The clone updates fetch into: the whole one when there is one.
    pub fn for_updates(&self) -> &Utf8Path {
        if note_of(&self.whole).is_some() {
            &self.whole
        } else {
            &self.partial
        }
    }

    /// The clone updates read, as the inputs page shows it.
    pub fn view(&self) -> CloneView {
        if let Some(note) = note_of(&self.whole) {
            return CloneView::Whole { tag: note.tag };
        }
        if self.partial.join("HEAD").is_file() {
            return CloneView::Partial;
        }
        CloneView::None
    }
}

/// What the whole clone at `dir` was made from. None when no bootstrap made one there.
fn note_of(dir: &Utf8Path) -> Option<ArchiveNote> {
    if !dir.join("HEAD").is_file() {
        return None;
    }
    serde_json::from_slice(&fs::read(dir.join(ARCHIVE_NOTE)).ok()?).ok()
}

/// How far a bootstrap has got, for the window to show.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type, tauri_specta::Event,
)]
pub struct ArchiveProgress {
    /// What the bootstrap is doing.
    pub step: InstallStep,
    /// The bytes done: of the archive downloaded, or of the download unpacked.
    pub done: u64,
    /// The bytes to do.
    pub total: u64,
}

/// Bootstraps the app's clone from a release's archive.
pub struct Bootstrap<'a> {
    source: &'a dyn Source,
    cancel: &'a AtomicBool,
    progress: &'a mut dyn FnMut(ArchiveProgress),
}

fn unwritable(error: impl std::fmt::Display) -> AppError {
    AppError::CensusUnwritable {
        detail: error.to_string(),
    }
}

fn unreadable(detail: impl Into<String>) -> AppError {
    AppError::ArchiveUnreadable {
        detail: detail.into(),
    }
}

impl<'a> Bootstrap<'a> {
    /// A bootstrap that reads from `source`, reports to `progress`, and stops when `cancel` is
    /// set.
    pub fn new(
        source: &'a dyn Source,
        cancel: &'a AtomicBool,
        progress: &'a mut dyn FnMut(ArchiveProgress),
    ) -> Self {
        Self {
            source,
            cancel,
            progress,
        }
    }

    /// Makes the whole clone from the archive of `release`, downloaded into `downloads`, in
    /// place of the whole clone held, and removes the fetched clone.
    pub fn run(
        &mut self,
        release: &IndexRelease,
        clones: &Clones,
        downloads: &Utf8Path,
    ) -> Result<ArchiveNote, AppError> {
        let census = clones.whole.as_path();
        let asset = release.archive.as_ref().ok_or(AppError::NoArchive)?;
        let file = downloads.join(format!("{}-{}", folder_name(&release.tag), asset.name));
        let total = asset.size;
        let progress = &mut *self.progress;
        let mut heard = |done: u64| {
            progress(ArchiveProgress {
                step: InstallStep::Download,
                done,
                total,
            })
        };
        Download::new(self.source, self.cancel, &mut heard, |detail| {
            AppError::CensusDownload { detail }
        })
        .fetch(asset, &file)?;

        let fresh = Utf8PathBuf::from(format!("{census}.new"));
        if fresh.exists() {
            fs::remove_dir_all(&fresh).map_err(unwritable)?;
        }
        let head = match self.unpack(&file, &fresh) {
            Ok(head) => head,
            Err(error) => {
                // A clone short of the archive's pack is no clone; nothing of it is kept.
                let _ = fs::remove_dir_all(&fresh);
                return Err(error);
            }
        };
        let note = ArchiveNote {
            tag: release.tag.clone(),
            head,
        };
        let json = serde_json::to_vec_pretty(&note).map_err(unwritable)?;
        fs::write(fresh.join(ARCHIVE_NOTE), json).map_err(unwritable)?;

        let old = Utf8PathBuf::from(format!("{census}.old"));
        if old.exists() {
            fs::remove_dir_all(&old).map_err(unwritable)?;
        }
        if census.exists() {
            fs::rename(census, &old).map_err(unwritable)?;
        }
        fs::rename(&fresh, census).map_err(unwritable)?;
        let _ = fs::remove_dir_all(&old);
        let _ = fs::remove_file(&file);
        // Updates fetch into the whole clone from now on, and nothing reads the fetched one.
        if clones.partial.exists() {
            fs::remove_dir_all(&clones.partial).map_err(unwritable)?;
        }
        Ok(note)
    }

    /// Writes the pack of the archive at `from` into a new clone at `to`, and answers the
    /// commit of the builds' branch.
    fn unpack(&mut self, from: &Utf8Path, to: &Utf8Path) -> Result<String, AppError> {
        let file = fs::File::open(from).map_err(unwritable)?;
        let total = file.metadata().map_err(unwritable)?.len();
        let progress = &mut *self.progress;
        let mut report = |done: u64| {
            progress(ArchiveProgress {
                step: InstallStep::Unpack,
                done,
                total,
            })
        };
        let cancel = self.cancel;
        let read_error = |error: io::Error| {
            if cancel.load(Ordering::Relaxed) {
                AppError::Cancelled
            } else {
                unreadable(error.to_string())
            }
        };
        let decoder = zst_reader(Counted::new(file, cancel, &mut report)).map_err(read_error)?;
        let seed = Seed::create(to).map_err(|error| unwritable(chain(&error)))?;
        let pack_dir = seed.pack_dir();

        let mut packs = Vec::new();
        let mut head = None;
        let mut archive = tar::Archive::new(decoder);
        for entry in archive.entries().map_err(read_error)? {
            let mut entry = entry.map_err(read_error)?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let path = entry.path_bytes().into_owned();
            let parts: Vec<&[u8]> = path
                .split(|&b| b == b'/')
                .filter(|p| !p.is_empty())
                .collect();
            match parts.as_slice() {
                [_, b"objects", b"pack", name] => {
                    let Some(name) = pack_file(name) else {
                        continue;
                    };
                    let mut out = fs::File::create(pack_dir.join(&name)).map_err(unwritable)?;
                    io::copy(&mut entry, &mut out).map_err(read_error)?;
                    packs.push(name);
                }
                [_, b"packed-refs"] => {
                    let mut text = String::new();
                    entry
                        .by_ref()
                        .take(REFS_MAX)
                        .read_to_string(&mut text)
                        .map_err(read_error)?;
                    head = branch_in(&text);
                }
                _ => {}
            }
        }

        let whole = |name: &str| {
            let stem = name.strip_suffix(".pack").unwrap_or_default();
            packs.contains(&format!("{stem}.idx"))
        };
        if !packs
            .iter()
            .any(|name| name.ends_with(".pack") && whole(name))
        {
            return Err(unreadable("the archive holds no pack with its index"));
        }
        let head = head.ok_or_else(|| {
            unreadable(format!(
                "the archive's packed-refs holds no refs/heads/{}",
                format::BRANCH
            ))
        })?;
        seed.finish(&head).map_err(|error| match error {
            CensusError::NoCommit(commit) => {
                unreadable(format!("the archive's pack holds no commit {commit}"))
            }
            error => unwritable(chain(&error)),
        })?;
        Ok(head)
    }
}

/// A pack's file name, `pack-<40 hex digits>.<pack|idx|rev|keep>`, as written anew from its
/// parts. None for any other name.
fn pack_file(name: &[u8]) -> Option<String> {
    let name = std::str::from_utf8(name).ok()?;
    let (stem, extension) = name.split_once('.')?;
    let hash = stem.strip_prefix("pack-")?;
    let hex = hash.len() == 40
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
    let known = PACK_FILES.iter().find(|known| **known == extension)?;
    hex.then(|| format!("pack-{hash}.{known}"))
}

/// The commit `packed-refs` gives the builds' branch.
fn branch_in(text: &str) -> Option<String> {
    let branch = format!("refs/heads/{}", format::BRANCH);
    text.lines().find_map(|line| {
        let (id, name) = line.split_once(' ')?;
        let hex = id.len() == 40 && id.bytes().all(|b| b.is_ascii_hexdigit());
        (hex && name == branch).then(|| id.to_ascii_lowercase())
    })
}
