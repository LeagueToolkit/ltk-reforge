//! A package's embedded evidence (#ADR-0041) opened as a kept run (#ADR-0096): a run with no
//! load and no snapshots. Its report is the one the writer put beside the package when that
//! reads and is of the package's archive, else the evidence alone.
//!
//! The package is untrusted input (`docs/design/ARCHITECTURE.md`, "Untrusted input"): its size
//! is bounded, the counts its header declares are held to its bytes before `ltk_modpkg`
//! reserves room for them, the meta chunks are read under a cap, and the reader runs where its
//! panic is caught. Nothing of it is written to disk.

use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::panic::{AssertUnwindSafe, catch_unwind};

use camino::Utf8Path;
use fs_err as fs;
use ltk_modpkg::{METADATA_CHUNK_PATH, Modpkg, ModpkgChunk, README_CHUNK_PATH};
use reforge::archive::Caps;
use reforge::model::{ContentId, clean, clean_json};
use reforge::package::{EvidenceDoc, MAX_README_BYTES, Made, REPORT_FILE};
use rusqlite::params;
use serde_json::{Value, json};

use super::read::Origin;
use super::{AppStore, json_of, kind_text, number, pack};
use crate::error::AppError;
use crate::run::{self, ModView, RunKind, RunSettings, RunView};
use crate::selection::FixupChoice;

/// The schema of the report an opened package with no report beside it is given: its
/// evidence, wrapped.
pub const EVIDENCE_ONLY_SCHEMA: &str = "reforge.evidence-only/1";

/// The magic a `.modpkg` starts with, and the one format of its header read here.
const MAGIC: &[u8; 8] = b"_modpkg_";
const HEADER_VERSION: u32 = 1;

/// What the package's metadata and README give.
struct Packaged {
    name: Option<String>,
    author: Option<String>,
    version: Option<String>,
    description: Option<String>,
    evidence: EvidenceDoc,
}

/// A package that does not read, with why.
fn unreadable(path: &Utf8Path, detail: impl std::fmt::Display) -> AppError {
    AppError::ModUnreadable {
        path: path.to_string(),
        detail: clean(&detail.to_string()).into_owned(),
    }
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

/// Whether the counts a `.modpkg`'s header declares fit in its `len` bytes: the signature, the
/// chunk table, the layers, the chunk paths and the WAD names, each of which `ltk_modpkg`
/// 0.12 reserves room for before it reads one (format 1: magic, version, signature size,
/// chunk count, signature, layers, paths, WADs). A file of another magic or version fits:
/// the reader refuses it.
fn header_fits(reader: &mut (impl BufRead + Seek), len: u64) -> io::Result<bool> {
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC || read_u32(reader)? != HEADER_VERSION {
        return Ok(true);
    }
    let signature = u64::from(read_u32(reader)?);
    let chunks = u64::from(read_u32(reader)?);
    if signature > len || chunks.saturating_mul(ModpkgChunk::RECORD_SIZE as u64) > len {
        return Ok(false);
    }
    reader.seek(SeekFrom::Current(signature as i64))?;
    // A layer is its name's length, its name and its priority: eight bytes at least.
    let layers = u64::from(read_u32(reader)?);
    if layers.saturating_mul(8) > len {
        return Ok(false);
    }
    for _ in 0..layers {
        let name = i64::from(read_u32(reader)?);
        if name as u64 > len {
            return Ok(false);
        }
        reader.seek(SeekFrom::Current(name + 4))?;
    }
    // A path and a WAD name are text ending in a NUL: a byte at least.
    let paths = u64::from(read_u32(reader)?);
    if paths > len {
        return Ok(false);
    }
    let mut skipped = Vec::new();
    for _ in 0..paths {
        skipped.clear();
        if reader.read_until(0, &mut skipped)? == 0 {
            return Ok(false);
        }
    }
    Ok(u64::from(read_u32(reader)?) <= len)
}

/// Whether a package holds a meta chunk. Refused for one larger than [`MAX_README_BYTES`],
/// which its reader would take whole: a README past it is no evidence, and the metadata is a
/// few hundred bytes.
fn holds_meta(packed: &Modpkg<BufReader<fs::File>>, chunk: &str, len: u64) -> Result<bool, String> {
    let Ok(found) = packed.chunk(chunk, None) else {
        return Ok(false);
    };
    let (size, stored) = (found.uncompressed_size, found.compressed_size);
    if size > MAX_README_BYTES as u64 || stored > len {
        return Err(format!("its {chunk} is {size} bytes, over the cap"));
    }
    Ok(true)
}

/// What a package's metadata and README give. Refused when the package does not read, and
/// when it holds no evidence.
fn packaged(path: &Utf8Path, caps: &Caps) -> Result<Packaged, AppError> {
    let file = fs::File::open(path).map_err(|error| unreadable(path, error))?;
    let len = file.metadata().map_err(|e| unreadable(path, e))?.len();
    if len > caps.archive_bytes {
        let over = format!("{len} bytes, over the cap of {}", caps.archive_bytes);
        return Err(unreadable(path, over));
    }
    let mut reader = BufReader::new(file);
    let fits = header_fits(&mut reader, len).map_err(|e| unreadable(path, e))?;
    if !fits {
        return Err(unreadable(
            path,
            "its header declares more than its bytes hold",
        ));
    }
    reader.rewind().map_err(|e| unreadable(path, e))?;
    // `ltk_modpkg` is a reader from another crate: its panic on a package is caught.
    let read = catch_unwind(AssertUnwindSafe(|| -> Result<_, String> {
        let mut packed = Modpkg::mount_from_reader(reader).map_err(|e| e.to_string())?;
        if !holds_meta(&packed, METADATA_CHUNK_PATH, len)? {
            return Err("it holds no metadata".to_string());
        }
        let metadata = packed.load_metadata().map_err(|e| e.to_string())?;
        let readme = match holds_meta(&packed, README_CHUNK_PATH, len)? {
            true => Some(packed.load_readme().map_err(|e| e.to_string())?),
            false => None,
        };
        Ok((metadata, readme))
    }));
    let (metadata, readme) = match read {
        Ok(read) => read.map_err(|detail| unreadable(path, detail))?,
        Err(_) => return Err(unreadable(path, "its reader panicked")),
    };
    let no_evidence = || AppError::NoEvidence {
        path: path.to_string(),
    };
    let readme = readme.ok_or_else(no_evidence)?;
    let readme = String::from_utf8(readme).map_err(|e| unreadable(path, e))?;
    let evidence = EvidenceDoc::parse(&readme).map_err(|e| unreadable(path, e))?;
    let evidence = evidence.ok_or_else(no_evidence)?;
    let text = |text: &str| (!text.is_empty()).then(|| text.to_string());
    let name = text(metadata.display_name()).or_else(|| text(metadata.name()));
    let authors: Vec<&str> = metadata.authors().iter().map(|a| a.name()).collect();
    Ok(Packaged {
        name,
        author: text(&authors.join(", ")),
        version: Some(metadata.version().to_string()),
        description: metadata.description().and_then(text),
        evidence,
    })
}

/// The report the writer put beside the package, `<stem>.reforge-report.json`, with the view
/// it gives. None when it is absent, over the cap, does not read, or is of another archive
/// than the package's evidence names.
fn report_beside(
    path: &Utf8Path,
    kind: RunKind,
    sha256: &str,
    caps: &Caps,
) -> Option<(String, RunView)> {
    let stem = path.file_stem()?;
    let report = path.with_file_name(format!("{stem}.{REPORT_FILE}"));
    let len = fs::metadata(&report).ok()?.len();
    if len > caps.entry_bytes {
        return None;
    }
    let text = fs::read_to_string(&report).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let archive = &value["evidence"]["archive"]["sha256"];
    if archive.as_str() != Some(sha256) {
        return None;
    }
    let view = run::view_of_report(kind, &text)?;
    Some((clean_json(&text).into_owned(), view))
}

impl AppStore {
    /// Opens a package's embedded evidence as a kept run, and answers its number (#ADR-0096).
    ///
    /// The run has no load and no snapshots: its report is the one beside the package when it
    /// reads, else the evidence wrapped as `reforge.evidence-only/1`, and its view what that
    /// gives. Its settings are the default but for the fixups, which are the ones the evidence
    /// names: the evidence holds no other setting, and a package opened was asked for by no
    /// request here. It has no schema build: the evidence names the game's build, not the
    /// schema database's. Refused for a package that does not read and for one with no
    /// evidence.
    pub fn open_package(&self, path: &Utf8Path) -> Result<u64, AppError> {
        let caps = Caps::default();
        let packaged = packaged(path, &caps)?;
        let evidence = &packaged.evidence;
        let sha256 = ContentId::parse(&evidence.archive.sha256)
            .ok_or_else(|| unreadable(path, "its evidence names no archive"))?;
        let kind = match evidence.run {
            Made::Repaired => RunKind::Repair,
            Made::Ported => RunKind::Port,
        };
        let settings = RunSettings {
            fixups: FixupChoice::of_ids(&evidence.fixups),
            ..RunSettings::default()
        };
        let file_name = path.file_name().unwrap_or(path.as_str()).to_string();
        let (report, view) = match report_beside(path, kind, &evidence.archive.sha256, &caps) {
            Some(beside) => beside,
            None => {
                let meta = ModView {
                    file_name: file_name.clone(),
                    mod_id: evidence.archive.sha256[..12].to_string(),
                    name: packaged.name.clone(),
                    author: packaged.author.clone(),
                    version: packaged.version.clone(),
                    description: packaged.description.clone(),
                };
                let report = json!({ "schema": EVIDENCE_ONLY_SCHEMA, "evidence": evidence });
                let report = clean_json(&report.to_string()).into_owned();
                (report, run::view_of_evidence(kind, evidence, meta))
            }
        };
        self.change(|_, tx| {
            // The evidence names the archive by its SHA-256 alone (#ADR-0041): the row's file
            // name is the package's, and it names no reading, caps or path.
            tx.execute(
                "INSERT INTO raw (sha256, version, caps, path, file_name, name, author,
                   mod_version)
                 VALUES (?1, 0, '{}', '', ?2, ?3, ?4, ?5) ON CONFLICT (sha256) DO NOTHING",
                params![
                    sha256.0.as_slice(),
                    file_name,
                    packaged.name,
                    packaged.author,
                    packaged.version,
                ],
            )?;
            tx.execute(
                "INSERT INTO run (sha256, origin, opened_from, kind, settings, schema_build,
                   program_version, package, kept, report, view)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?3, 1, ?7, ?8)",
                params![
                    sha256.0.as_slice(),
                    Origin::Package.as_str(),
                    path.as_str(),
                    kind_text(kind),
                    json_of(&settings),
                    evidence.program.version,
                    pack(&report)?,
                    pack(&json_of(&view))?,
                ],
            )?;
            Ok((number(tx.last_insert_rowid()), 0))
        })
    }
}
