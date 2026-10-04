//! The writers: a package on disk as a mod project LTK Manager's workshop opens, and packed
//! as a `.modpkg`.
//!
//! ```text
//! <out>/<project>/mod.config.json
//!                 reforge-report.json
//!                 hashes/game.hashes.txt, hashes/binentries.hashes.txt
//!                 content/<layer>/<Champion>.wad.client/<path>
//! <out>/<project>_<version>.modpkg
//! <out>/<project>_<version>.reforge-report.json
//! ```
//!
//! A writer makes everything in a directory of its own beside the place, and renames the
//! result into place when it is whole. A write that fails leaves nothing behind. Every path
//! written is under the output directory, made of a project name, a [`WadName`] and the parts
//! of a [`ChunkPath`].

use std::io::{self, Write as _};

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use ltk_mod_project::modpkg::{ModpkgFormat, ModpkgPackError};
use ltk_mod_project::{
    ConfigFormat, IgnoreMode, ModProjectError, PackError, PackOptions, ProjectPacker,
};
use serde::Serialize;

use super::config::{ENTRIES_TABLE, PATHS_TABLE};
use super::findings::{Blocked, Written};
use super::{ChunkKey, HashTables, Package};
use crate::diag::{DiagId, DiagnosticPolicy, Diagnostics, Sink, Stage};
use crate::model::ChunkPath;

/// The report's file name beside a project's config.
pub const REPORT_FILE: &str = "reforge-report.json";

/// The longest file name a writer makes: what NTFS, ext4 and APFS hold in one name.
pub const MAX_NAME_BYTES: usize = 255;

/// The config's file name in a project.
const CONFIG_FILE: &str = "mod.config.json";

/// What a writer puts on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WriterKind {
    /// A mod project directory.
    Project,
    /// A `.modpkg` file.
    Modpkg,
}

impl WriterKind {
    /// The kind in a message.
    pub fn as_str(self) -> &'static str {
        match self {
            WriterKind::Project => "project",
            WriterKind::Modpkg => "package",
        }
    }
}

/// Why a package is not on disk: a fault of the machine or of the output directory, and no
/// diagnostic.
#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    /// A file or a directory did not write. The error names the path.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Something that is no mod project is where the project goes. It is left as it is.
    #[error("{0} is in the way and is no mod project: it is not replaced")]
    Occupied(Utf8PathBuf),
    /// The project's config did not serialise or read back.
    #[error("the project's config does not write")]
    Config(#[from] ModProjectError),
    /// The project did not pack.
    #[error("the project does not pack")]
    Pack(#[from] Box<PackError<ModpkgPackError>>),
}

/// The caller's policy refused the write, at this diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the write is refused at diagnostic {0}")]
pub struct Refused(pub DiagId);

/// Asks a caller's policy whether a package may be written. Where a diagnostic of the log
/// blocks, `write.blocked` is raised and names it. No code blocks by default (#ADR-0003).
pub fn permit(policy: &DiagnosticPolicy, log: &mut Diagnostics) -> Result<(), Refused> {
    let by = policy.blocking(log);
    if by.is_empty() {
        return Ok(());
    }
    Err(Refused(log.at(Stage::Write).raise(Blocked { by })))
}

/// Puts a package on disk.
pub trait PackageWriter {
    /// What it writes.
    fn kind(&self) -> WriterKind;

    /// Writes the package and, beside it, the report's text. What comes back is what was
    /// written, for the caller to raise.
    fn write(&self, package: &Package, report: &str) -> Result<Written, WriteError>;
}

impl Package {
    /// Writes the package with a writer, the report's text beside it, and raises what was
    /// written.
    pub fn write(
        &self,
        writer: &dyn PackageWriter,
        report: &str,
        sink: &mut Sink<'_>,
    ) -> Result<(), WriteError> {
        let _span = tracing::info_span!("write", chunks = self.len()).entered();
        sink.raise(writer.write(self, report)?);
        Ok(())
    }
}

/// Whether a part of a chunk's path is longer than a file name may be: such a chunk is named
/// by its hash on disk.
fn too_long(path: &ChunkPath) -> bool {
    path.parts().any(|part| part.len() > MAX_NAME_BYTES)
}

/// A chunk's place under its WAD's directory: the parts of its path, or for a path with a
/// part longer than a file name its hash, which a packer reads back as the hash
/// (#MOD-ARTIFACT-0007).
fn place(key: &ChunkKey) -> Vec<String> {
    if !too_long(&key.path) {
        return key.path.parts().map(str::to_string).collect();
    }
    let short = |ext: &&str| ext.len() <= 8 && ext.bytes().all(|b| b.is_ascii_alphanumeric());
    let extension = key.path.extension().filter(short).unwrap_or("dat");
    vec![format!("{:016x}.{extension}", key.hash().0)]
}

/// How much a tree holds.
#[derive(Debug, Clone, Copy, Default)]
struct Tally {
    chunks: usize,
    hash_named: usize,
    bytes: u64,
}

/// A package as the files of a mod project, under one directory.
struct Tree<'a> {
    package: &'a Package,
    root: &'a Utf8Path,
}

impl Tree<'_> {
    fn file(&self, parts: &[&str], bytes: &[u8]) -> Result<(), WriteError> {
        let mut path = self.root.to_path_buf();
        path.extend(parts);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        Ok(fs::write(&path, bytes)?)
    }

    /// Writes the chunks, the hash tables and the config.
    fn write(&self) -> Result<Tally, WriteError> {
        let mut tally = Tally::default();
        let mut tables: HashTables = self.package.tables.clone();
        for layer in self.package.layers() {
            let layer_name = layer.name();
            // A project's layer has its directory, whatever it holds.
            fs::create_dir_all(self.root.join("content").join(&layer_name))?;
            for (key, chunk) in self.package.layer(layer) {
                let place = place(key);
                let mut parts = vec!["content", &layer_name, key.wad.as_str()];
                parts.extend(place.iter().map(String::as_str));
                self.file(&parts, &chunk.bytes)?;
                tally.chunks += 1;
                tally.bytes += chunk.bytes.len() as u64;
                if too_long(&key.path) {
                    tally.hash_named += 1;
                    // The name the chunk lost on disk, for whoever reads the package.
                    tables.add_path(key.path.as_str());
                }
            }
        }
        let paths: String = tables.paths().map(|name| format!("{name}\n")).collect();
        let entries: String = tables.entries().map(|name| format!("{name}\n")).collect();
        self.file(
            &PATHS_TABLE.split('/').collect::<Vec<_>>(),
            paths.as_bytes(),
        )?;
        self.file(
            &ENTRIES_TABLE.split('/').collect::<Vec<_>>(),
            entries.as_bytes(),
        )?;
        let project = self.package.config.to_project(self.package.layers());
        let config = project.to_config_string(ConfigFormat::Json)?;
        self.file(&[CONFIG_FILE], config.as_bytes())?;
        Ok(tally)
    }
}

/// A directory a writer fills beside the place its result goes. It is removed when the
/// writer is done with it, so a write that fails leaves nothing.
struct Staging {
    path: Utf8PathBuf,
}

impl Staging {
    /// A new, empty directory. What an earlier run that was killed left there is removed.
    fn new(path: Utf8PathBuf) -> Result<Self, WriteError> {
        if path.exists() {
            fs::remove_dir_all(&path)?;
        }
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    /// Renames the directory into place, over the mod project that is there.
    fn into_place(self, target: &Utf8Path) -> Result<(), WriteError> {
        if !target.exists() {
            return Ok(fs::rename(&self.path, target)?);
        }
        let aside = Utf8PathBuf::from(format!("{target}.replaced"));
        if aside.exists() {
            fs::remove_dir_all(&aside)?;
        }
        fs::rename(target, &aside)?;
        if let Err(error) = fs::rename(&self.path, target) {
            // The project that was there goes back: a failed write changes nothing.
            let _ = fs::rename(&aside, target);
            return Err(error.into());
        }
        Ok(fs::remove_dir_all(&aside)?)
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Writes a package as a mod project directory, `<out>/<project>`, with the report in it.
#[derive(Debug, Clone)]
pub struct ProjectDir {
    out: Utf8PathBuf,
}

impl ProjectDir {
    /// A writer into an output directory, made when absent.
    pub fn new(out: impl Into<Utf8PathBuf>) -> Self {
        Self { out: out.into() }
    }

    /// Where a package's project goes.
    pub fn path_of(&self, package: &Package) -> Utf8PathBuf {
        self.out.join(&package.config.name)
    }
}

impl PackageWriter for ProjectDir {
    fn kind(&self) -> WriterKind {
        WriterKind::Project
    }

    fn write(&self, package: &Package, report: &str) -> Result<Written, WriteError> {
        let name = &package.config.name;
        let target = self.path_of(package);
        // Only a mod project is replaced: a directory of anything else is not this writer's.
        if target.exists() && !target.join(CONFIG_FILE).is_file() {
            return Err(WriteError::Occupied(target));
        }
        fs::create_dir_all(&self.out)?;
        let staging = Staging::new(self.out.join(format!("{name}.partial")))?;
        let tree = Tree {
            package,
            root: &staging.path,
        };
        let tally = tree.write()?;
        tree.file(&[REPORT_FILE], report.as_bytes())?;
        staging.into_place(&target)?;
        Ok(Written {
            kind: WriterKind::Project,
            name: name.clone(),
            chunks: tally.chunks,
            hash_named: tally.hash_named,
            bytes: tally.bytes,
        })
    }
}

/// Writes a package as `<out>/<project>_<version>.modpkg`, with the report beside it as
/// `<project>_<version>.reforge-report.json`.
#[derive(Debug, Clone)]
pub struct Modpkg {
    out: Utf8PathBuf,
}

impl Modpkg {
    /// A writer into an output directory, made when absent.
    pub fn new(out: impl Into<Utf8PathBuf>) -> Self {
        Self { out: out.into() }
    }

    /// Where a package's `.modpkg` goes.
    pub fn path_of(&self, package: &Package) -> Utf8PathBuf {
        self.out.join(package.config.modpkg_name())
    }

    /// Where the report of a package goes.
    pub fn report_path_of(&self, package: &Package) -> Utf8PathBuf {
        let stem = format!("{}_{}", package.config.name, package.config.version);
        self.out.join(format!("{stem}.{REPORT_FILE}"))
    }

    /// Packs a project directory into a file, as LTK Manager packs one.
    fn pack(project: &Utf8Path, file: &Utf8Path) -> Result<(), WriteError> {
        // A chunk of a mod named `.modignore` is no filter of this package's content.
        let options = PackOptions::default().with_ignore(IgnoreMode::Disabled);
        let packer = ProjectPacker::from_dir(project)?.with_options(options);
        let mut out = io::BufWriter::new(fs::File::create(file)?);
        packer.pack(ModpkgFormat::new(&mut out)).map_err(Box::new)?;
        Ok(out.flush()?)
    }
}

impl PackageWriter for Modpkg {
    fn kind(&self) -> WriterKind {
        WriterKind::Modpkg
    }

    fn write(&self, package: &Package, report: &str) -> Result<Written, WriteError> {
        let name = package.config.modpkg_name();
        fs::create_dir_all(&self.out)?;
        // The packer reads a project from disk: the project is staged, packed and removed.
        let staging = Staging::new(self.out.join(format!("{name}.partial")))?;
        let project = staging.path.join("project");
        let tree = Tree {
            package,
            root: &project,
        };
        let tally = tree.write()?;
        let packed = staging.path.join(&name);
        Self::pack(&project, &packed)?;
        let staged_report = staging.path.join(REPORT_FILE);
        fs::write(&staged_report, report)?;
        fs::rename(&packed, self.path_of(package))?;
        fs::rename(&staged_report, self.report_path_of(package))?;
        Ok(Written {
            kind: WriterKind::Modpkg,
            name,
            chunks: tally.chunks,
            hash_named: tally.hash_named,
            bytes: tally.bytes,
        })
    }
}
