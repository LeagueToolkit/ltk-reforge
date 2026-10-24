//! A delta: the bytes of an index taken forward, patched against the base it was taken from,
//! published beside the base's snapshot and applied to a copy of the base (#ADR-0109).
//!
//! An append rewrites pages of the database in place and adds pages at its end, so the two
//! files line up chunk for chunk. The delta is a tar: `delta.json` first, then the new
//! `builds.tsv` and `stamp.json` whole, then one zstd frame per chunk of the new database,
//! each compressed with the same chunk of the base as its dictionary. Applying decodes each
//! frame with the base's chunk, and the result is checked against the digest the note carries.

use std::io::{self, Read, Write};

use camino::Utf8Path;
use fs_err as fs;
use memmap2::Mmap;
use reforge::history::{BUILDS_FILE, BuildRow, DB_FILE, FORMAT, STAMP_FILE, Stamp};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zstd::zstd_safe::{CParameter, DictAttachPref};

use crate::error::CensusError;
use crate::write::{install, scratch};

/// The format of a delta. A delta of another format is refused.
pub const DELTA_FORMAT: u32 = 1;

/// The note in a delta, its first entry.
pub const NOTE_FILE: &str = "delta.json";

/// A chunk of the new database in a delta: `chunk-<n>.zst`, numbered from 0.
const CHUNK_STEM: &str = "chunk-";

/// The bytes of the database a frame holds: a power of two, so pages of 8 KB line up, and
/// small enough that a chunk and its dictionary sit in a window of one GiB.
pub const CHUNK: usize = 512 << 20;

/// The window a frame may reach back: the chunk and its dictionary.
const WINDOW_LOG: u32 = 30;

/// The compression level: the patch is made once, so a slow level costs little.
const LEVEL: i32 = 9;

/// What a delta says of itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeltaNote {
    /// The delta's format.
    pub format: u32,
    /// The format of the index it applies to and makes.
    pub index_format: u32,
    /// The head of the base: the stamp's `head` of the index the delta applies to.
    pub base_head: String,
    /// The head of the index the delta makes.
    pub head: String,
    /// How many builds the delta adds.
    pub builds: usize,
    /// The bytes of the database the delta makes.
    pub size: u64,
    /// The SHA-256 of that database, as hex digits.
    pub sha256: String,
    /// The bytes of a chunk.
    pub chunk: u64,
}

/// How far an apply has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaStep {
    /// The database is being written from the delta and the base.
    Write {
        /// The bytes written.
        done: u64,
        /// The bytes to write.
        total: u64,
    },
    /// The database written is being checked against the delta's digest.
    Check,
}

/// Writes the delta at `path` that takes the index at `base` to the index at `new`: the new
/// database patched against the base's, with the new index's builds and stamp. The new index
/// must have been taken forward from the base: its builds start with the base's.
pub fn write(path: &Utf8Path, base: &Utf8Path, new: &Utf8Path) -> Result<DeltaNote, CensusError> {
    let base_stamp = Stamp::read(base)?;
    let stamp = Stamp::read(new)?;
    let builds = BuildRow::read_all(new)?;
    let base_builds = BuildRow::read_all(base)?;
    let shared = builds.iter().zip(&base_builds).all(|(a, b)| a == b);
    if !shared || builds.len() < base_builds.len() {
        return Err(CensusError::DeltaBase {
            expected: base_stamp.head,
            found: stamp.head,
        });
    }
    let base_db = map(&base.join(DB_FILE))?;
    let new_db = map(&new.join(DB_FILE))?;
    let note = DeltaNote {
        format: DELTA_FORMAT,
        index_format: FORMAT,
        base_head: base_stamp.head,
        head: stamp.head.clone(),
        builds: builds.len() - base_builds.len(),
        size: new_db.len() as u64,
        sha256: hex(&Sha256::digest(&new_db[..])),
        chunk: CHUNK as u64,
    };
    let mut tar = tar::Builder::new(fs::File::create(path)?);
    put(&mut tar, NOTE_FILE, &serde_json::to_vec_pretty(&note)?)?;
    let lines: String = builds.iter().map(BuildRow::line).collect();
    put(&mut tar, BUILDS_FILE, lines.as_bytes())?;
    put(&mut tar, STAMP_FILE, &serde_json::to_vec_pretty(&stamp)?)?;
    let (base, new) = (base_db, new_db);
    for (n, chunk) in new.chunks(CHUNK).enumerate() {
        let dictionary = base
            .get(n * CHUNK..)
            .map(|rest| &rest[..rest.len().min(CHUNK)]);
        let frame = patch(chunk, dictionary)?;
        put(&mut tar, &format!("{CHUNK_STEM}{n}.zst"), &frame)?;
    }
    tar.into_inner()?.flush()?;
    Ok(note)
}

/// One chunk as a zstd frame, patched against `dictionary` when there is one: the dictionary
/// loaded whole into the window, as `zstd --patch-from` does, so long matches reach into it.
fn patch(chunk: &[u8], dictionary: Option<&[u8]>) -> Result<Vec<u8>, CensusError> {
    let mut out = Vec::new();
    let mut encoder = match dictionary {
        Some(dictionary) => zstd::Encoder::with_dictionary(&mut out, LEVEL, dictionary)?,
        None => zstd::Encoder::new(&mut out, LEVEL)?,
    };
    encoder.set_parameter(CParameter::WindowLog(WINDOW_LOG))?;
    encoder.set_parameter(CParameter::EnableLongDistanceMatching(true))?;
    if dictionary.is_some() {
        encoder.set_parameter(CParameter::ForceAttachDict(DictAttachPref::ForceLoad))?;
    }
    encoder.set_pledged_src_size(Some(chunk.len() as u64))?;
    encoder.write_all(chunk)?;
    encoder.finish()?;
    Ok(out)
}

fn put<W: Write>(tar: &mut tar::Builder<W>, name: &str, bytes: &[u8]) -> Result<(), CensusError> {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    Ok(tar.append_data(&mut header, name, bytes)?)
}

fn map(path: &Utf8Path) -> Result<Mmap, CensusError> {
    let file = fs::File::open(path)?;
    // The file is read, not written, while mapped.
    Ok(unsafe { Mmap::map(&file)? })
}

fn hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The note of the delta at `path`.
pub fn read_note(path: &Utf8Path) -> Result<DeltaNote, CensusError> {
    let mut archive = tar::Archive::new(fs::File::open(path)?);
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.path()?.as_os_str() == NOTE_FILE {
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            return Ok(serde_json::from_str(&text)?);
        }
    }
    Err(CensusError::DeltaEntry {
        path: path.to_owned(),
        entry: NOTE_FILE,
    })
}

/// Applies the delta at `delta` to the index at `base`, written into `out`: the index the
/// append the delta records wrote, byte for byte. The base's head must be the delta's base
/// head.
pub fn apply(
    delta: &Utf8Path,
    base: &Utf8Path,
    out: &Utf8Path,
    progress: &mut dyn FnMut(DeltaStep),
) -> Result<Stamp, CensusError> {
    let base_stamp = Stamp::read(base)?;
    let note = read_note(delta)?;
    if note.format != DELTA_FORMAT || note.index_format != FORMAT {
        return Err(CensusError::DeltaFormat {
            format: note.format,
            index_format: note.index_format,
        });
    }
    if note.base_head != base_stamp.head {
        return Err(CensusError::DeltaBase {
            expected: note.base_head,
            found: base_stamp.head,
        });
    }
    let chunk = usize::try_from(note.chunk).map_err(|_| CensusError::DeltaChunk(note.chunk))?;
    if chunk == 0 || !chunk.is_power_of_two() || chunk > CHUNK {
        return Err(CensusError::DeltaChunk(note.chunk));
    }

    let tmp = scratch(out)?;
    let base_db = map(&base.join(DB_FILE))?;
    let mut db = fs::File::create(tmp.join(DB_FILE))?;
    let mut digest = Sha256::new();
    let mut written = 0u64;
    let mut next_chunk = 0usize;
    let mut builds = None;
    let mut stamp = None;
    progress(DeltaStep::Write {
        done: 0,
        total: note.size,
    });
    let mut archive = tar::Archive::new(fs::File::open(delta)?);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().into_owned();
        if let Some(number) = name
            .strip_prefix(CHUNK_STEM)
            .and_then(|n| n.strip_suffix(".zst"))
        {
            let n: usize = number.parse().map_err(|_| CensusError::DeltaEntry {
                path: delta.to_owned(),
                entry: "a chunk in order",
            })?;
            if n != next_chunk {
                return Err(CensusError::DeltaEntry {
                    path: delta.to_owned(),
                    entry: "a chunk in order",
                });
            }
            next_chunk += 1;
            let dictionary = base_db
                .get(n * chunk..)
                .map(|rest| &rest[..rest.len().min(chunk)]);
            let mut decoder = match dictionary {
                Some(dictionary) => {
                    zstd::Decoder::with_dictionary(io::BufReader::new(&mut entry), dictionary)?
                }
                None => zstd::Decoder::new(&mut entry)?,
            };
            decoder.window_log_max(WINDOW_LOG)?;
            let mut buffer = vec![0u8; 8 << 20];
            loop {
                let read = decoder.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                db.write_all(&buffer[..read])?;
                digest.update(&buffer[..read]);
                written += read as u64;
                progress(DeltaStep::Write {
                    done: written,
                    total: note.size,
                });
            }
            continue;
        }
        match name.as_str() {
            BUILDS_FILE => {
                let mut text = String::new();
                entry.read_to_string(&mut text)?;
                builds = Some(text);
            }
            STAMP_FILE => {
                let mut text = String::new();
                entry.read_to_string(&mut text)?;
                stamp = Some(serde_json::from_str::<Stamp>(&text)?);
            }
            _ => {}
        }
    }
    db.flush()?;
    drop(db);
    progress(DeltaStep::Check);
    let sha256 = hex(&digest.finalize());
    if written != note.size || sha256 != note.sha256 {
        return Err(CensusError::DeltaDigest {
            expected: note.sha256,
            found: sha256,
        });
    }
    let missing = |entry: &'static str| CensusError::DeltaEntry {
        path: delta.to_owned(),
        entry,
    };
    let builds_text = builds.ok_or_else(|| missing(BUILDS_FILE))?;
    let stamp = stamp.ok_or_else(|| missing(STAMP_FILE))?;
    fs::write(tmp.join(BUILDS_FILE), &builds_text)?;
    let builds = BuildRow::read_all(&tmp)?;
    if stamp.head != note.head || builds.last().is_none_or(|row| row.commit != note.head) {
        return Err(CensusError::DeltaBase {
            expected: note.head,
            found: stamp.head,
        });
    }
    install(&tmp, out, &builds, &stamp)?;
    Ok(stamp)
}
