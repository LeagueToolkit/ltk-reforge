//! Downloads that resume (#ADR-0060). A release's file is written to a file of the app's data
//! folder as it comes, beside a note of what it is: the entity tag the source sent it under, and
//! its size and digest as the release lists them. A download that stops keeps both, and the next
//! asks for the rest while the release still lists that file and the source sends the same tag.
//! GitHub ignores `If-Range`, so the tag is compared here. The whole file is checked against its
//! digest before it is taken.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::index::{Asset, Source};

/// What a download that stopped is of, beside its bytes.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Note {
    etag: Option<String>,
    size: u64,
    sha256: String,
}

/// A download of one file, which resumes.
pub struct Download<'a> {
    source: &'a dyn Source,
    cancel: &'a AtomicBool,
    progress: &'a mut dyn FnMut(u64),
    failed: fn(String) -> AppError,
}

impl<'a> Download<'a> {
    /// A download read from `source`, which reports the bytes it holds to `progress`, stops when
    /// `cancel` is set, and names a failure to read or write with `failed`.
    pub fn new(
        source: &'a dyn Source,
        cancel: &'a AtomicBool,
        progress: &'a mut dyn FnMut(u64),
        failed: fn(String) -> AppError,
    ) -> Self {
        Self {
            source,
            cancel,
            progress,
            failed,
        }
    }

    /// Downloads `asset` to `path`, from where a download of it stopped, and checks it against
    /// its digest. A file at `path` with that digest is not downloaded again.
    pub fn fetch(&mut self, asset: &Asset, path: &Utf8Path) -> Result<(), AppError> {
        let fail = self.failed;
        let io = |error: io::Error| fail(error.to_string());
        if path.is_file() && digest(path).map_err(io)? == asset.sha256 {
            (self.progress)(asset.size);
            return Ok(());
        }
        let partial = Utf8PathBuf::from(format!("{path}.partial"));
        let note_path = Utf8PathBuf::from(format!("{path}.partial.json"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }

        let held = read_note(&note_path).filter(|note| {
            note.size == asset.size && note.sha256 == asset.sha256 && note.etag.is_some()
        });
        let kept = fs::metadata(&partial).map_or(0, |meta| meta.len());
        let mut have = if held.is_some() && kept <= asset.size {
            kept
        } else {
            0
        };
        if have < asset.size {
            let mut body = self.source.open_from(&asset.url, have)?;
            let same = held.as_ref().is_some_and(|note| note.etag == body.etag);
            if body.start > 0 && !same {
                // The bytes kept are of another version of the file.
                body = self.source.open_from(&asset.url, 0)?;
            }
            have = body.start;
            let note = Note {
                etag: body.etag,
                size: asset.size,
                sha256: asset.sha256.clone(),
            };
            let json = serde_json::to_vec(&note).map_err(|error| fail(error.to_string()))?;
            fs::write(&note_path, json).map_err(io)?;
            let file = if have == 0 {
                fs::File::create(&partial)
            } else {
                fs::OpenOptions::new().append(true).open(&partial)
            };
            self.copy(body.reader, &mut file.map_err(io)?, have, asset)?;
        }

        let size = fs::metadata(&partial).map_err(io)?.len();
        if size != asset.size {
            return Err(fail(format!(
                "{}: {size} of {} bytes",
                asset.name, asset.size
            )));
        }
        if digest(&partial).map_err(io)? != asset.sha256 {
            // Bytes that are not the release's are of no use to a later download.
            let _ = fs::remove_file(&partial);
            let _ = fs::remove_file(&note_path);
            return Err(AppError::IndexDigest {
                file: asset.name.clone(),
            });
        }
        fs::rename(&partial, path).map_err(io)?;
        let _ = fs::remove_file(&note_path);
        Ok(())
    }

    /// Appends what `from` reads to `to`, which holds `have` bytes already.
    fn copy(
        &mut self,
        mut from: Box<dyn Read + Send>,
        to: &mut fs::File,
        have: u64,
        asset: &Asset,
    ) -> Result<(), AppError> {
        let fail = self.failed;
        let mut buf = vec![0; 64 * 1024];
        let mut done = have;
        (self.progress)(done);
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(AppError::Cancelled);
            }
            let read = from
                .read(&mut buf)
                .map_err(|error| fail(format!("{}: {error}", asset.name)))?;
            if read == 0 {
                break;
            }
            to.write_all(&buf[..read])
                .map_err(|error| fail(error.to_string()))?;
            done += read as u64;
            (self.progress)(done);
        }
        to.flush().map_err(|error| fail(error.to_string()))
    }
}

fn read_note(path: &Utf8Path) -> Option<Note> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// The SHA-256 of a file, as hex digits.
fn digest(path: &Utf8Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    io::copy(&mut fs::File::open(path)?, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}
