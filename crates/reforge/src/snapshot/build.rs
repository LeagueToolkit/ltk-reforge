//! The snapshot after `build` (#ADR-0094): the package with its chunks, patches and thumbnail
//! named by the content id of their bytes, and the report's package part. A run starts from it
//! and the snapshots before it, and reads the package back from the store.
//!
//! What a check and a writer read of a package that its bytes do not say is kept beside each
//! id: where a chunk's bytes come from, and for a bin the run wrote, the entries it was built
//! with and each object a fixup or the port wrote, as a bin of that object alone.

use serde::{Deserialize, Serialize};

use super::{Program, ResumeError, SNAPSHOT_FORMAT, Setting, StageSnapshot};
use crate::diag::{Logged, Stage};
use crate::model::entity::{CharId, FileId, UnitId};
use crate::model::hash::bin_hex;
use crate::model::{BinHash, ByteView, ChunkPath, ContentId, Layer, WadName, bin};
use crate::package::{
    BinWrite, Chunk, ChunkKey, EvidenceDoc, Origin, Package, Patch, PatchOf, ProjectConfig,
    Thumbnail, WrittenBin,
};
use crate::report::PackageRow;
use crate::store::BlobStore;

/// The settings `build` reads (#ADR-0094). It reads no input `load` does not: the install's
/// shader bin and shader cache a material is judged against, the game's skin bins a port
/// patches and the skin0 bin of a sub-character it copies, the game's files and the history a
/// repair's texture is judged stale against, and the hash tables that spell a repair's paths
/// and a patch's fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildSettings {
    /// Whether each DDS ships as it is, not put in TEX's container (#ADR-0055).
    pub keep_dds: bool,
    /// Whether stale copies are carried: a repair judges a texture again in the container it
    /// ships in.
    pub keep_stale: bool,
}

impl BuildSettings {
    fn differs(&self, other: &Self) -> Option<Setting> {
        if self.keep_dds != other.keep_dds {
            Some(Setting::KeepDds)
        } else if self.keep_stale != other.keep_stale {
            Some(Setting::KeepStale)
        } else {
            None
        }
    }
}

/// An entry hash, as eight hex digits.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Entry(#[serde(with = "bin_hex")] BinHash);

/// What a bin the run wrote was built to hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WrittenRecord {
    how: BinWrite,
    entries: Vec<Entry>,
    /// Each object written, as a bin of that object alone.
    edited: Vec<ContentId>,
}

impl WrittenRecord {
    fn of(written: &WrittenBin, store: &dyn BlobStore) -> Self {
        let edited = written.edited.iter();
        Self {
            how: written.how,
            entries: written.entries.iter().copied().map(Entry).collect(),
            edited: edited
                .map(|object| store.put(bin::write_one(object).into()))
                .collect(),
        }
    }

    fn written(&self, store: &dyn BlobStore) -> Result<WrittenBin, ResumeError> {
        let edited = self.edited.iter().map(|&id| {
            let bytes = store.get(id).ok_or(ResumeError::Store(id))?;
            let object = bin::read_one(&bytes);
            object.map_err(|why| ResumeError::Malformed(format!("{id:?}: {why}")))
        });
        Ok(WrittenBin {
            how: self.how,
            entries: self.entries.iter().map(|entry| entry.0).collect(),
            edited: edited.collect::<Result<_, _>>()?,
        })
    }
}

/// Where a chunk's bytes come from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "as", rename_all = "kebab-case")]
enum OriginRecord {
    Kept {
        file: FileId,
    },
    Rewritten {
        file: FileId,
        written: WrittenRecord,
    },
    Contained {
        file: FileId,
    },
    Made {
        written: WrittenRecord,
    },
    Bank {
        file: FileId,
    },
}

impl OriginRecord {
    fn of(origin: &Origin, store: &dyn BlobStore) -> Self {
        match origin {
            Origin::Kept(file) => OriginRecord::Kept { file: *file },
            Origin::Rewritten { file, written } => OriginRecord::Rewritten {
                file: *file,
                written: WrittenRecord::of(written, store),
            },
            Origin::Contained(file) => OriginRecord::Contained { file: *file },
            Origin::Made(written) => OriginRecord::Made {
                written: WrittenRecord::of(written, store),
            },
            Origin::Bank(file) => OriginRecord::Bank { file: *file },
        }
    }

    fn origin(&self, store: &dyn BlobStore) -> Result<Origin, ResumeError> {
        Ok(match self {
            OriginRecord::Kept { file } => Origin::Kept(*file),
            OriginRecord::Rewritten { file, written } => Origin::Rewritten {
                file: *file,
                written: written.written(store)?,
            },
            OriginRecord::Contained { file } => Origin::Contained(*file),
            OriginRecord::Made { written } => Origin::Made(written.written(store)?),
            OriginRecord::Bank { file } => Origin::Bank(*file),
        })
    }
}

/// A chunk, its bytes by content id.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkRecord {
    wad: WadName,
    path: ChunkPath,
    content: ContentId,
    origin: OriginRecord,
}

/// What a patch is of, with the skin entry it patches.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "of", rename_all = "lowercase")]
enum PatchOfRecord {
    Skin {
        unit: UnitId,
        #[serde(with = "bin_hex")]
        entry: BinHash,
    },
    Root {
        character: CharId,
    },
}

/// A patch, its PTCH by content id.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PatchRecord {
    target: ChunkPath,
    path: ChunkPath,
    content: ContentId,
    links: Vec<ChunkPath>,
    of: PatchOfRecord,
    records: usize,
}

impl PatchRecord {
    fn of(patch: &Patch, store: &dyn BlobStore) -> Self {
        Self {
            target: patch.target.clone(),
            path: patch.path.clone(),
            content: store.put(patch.bytes.clone()),
            links: patch.links.clone(),
            of: match patch.of {
                PatchOf::Skin { unit, entry } => PatchOfRecord::Skin { unit, entry },
                PatchOf::Root { character } => PatchOfRecord::Root { character },
            },
            records: patch.records,
        }
    }

    fn patch(&self, store: &dyn BlobStore) -> Result<Patch, ResumeError> {
        Ok(Patch {
            target: self.target.clone(),
            path: self.path.clone(),
            bytes: bytes(store, self.content)?,
            links: self.links.clone(),
            of: match self.of {
                PatchOfRecord::Skin { unit, entry } => PatchOf::Skin { unit, entry },
                PatchOfRecord::Root { character } => PatchOf::Root { character },
            },
            records: self.records,
        })
    }
}

/// A layer's chunks and patches.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LayerRecord {
    layer: Layer,
    chunks: Vec<ChunkRecord>,
    patches: Vec<PatchRecord>,
}

/// A package, every byte of it by content id.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PackageRecord {
    config: ProjectConfig,
    paths: Vec<String>,
    entries: Vec<String>,
    evidence: Option<EvidenceDoc>,
    thumbnail: Option<ContentId>,
    layers: Vec<LayerRecord>,
}

impl PackageRecord {
    fn of(package: &Package, store: &dyn BlobStore) -> Self {
        let layers = package.layers().map(|layer| LayerRecord {
            layer: layer.clone(),
            chunks: package
                .layer(layer)
                .map(|(key, chunk)| ChunkRecord {
                    wad: key.wad.clone(),
                    path: key.path.clone(),
                    content: store.put(chunk.bytes.clone()),
                    origin: OriginRecord::of(&chunk.origin, store),
                })
                .collect(),
            patches: package
                .patches(layer)
                .iter()
                .map(|patch| PatchRecord::of(patch, store))
                .collect(),
        });
        let thumbnail = package.thumbnail.as_ref();
        Self {
            config: package.config.clone(),
            paths: package.tables.paths().map(str::to_string).collect(),
            entries: package.tables.entries().map(str::to_string).collect(),
            evidence: package.evidence.clone(),
            thumbnail: thumbnail.map(|t| store.put(ByteView::from(t.bytes()))),
            layers: layers.collect(),
        }
    }

    fn package(&self, store: &dyn BlobStore) -> Result<Package, ResumeError> {
        let mut package = Package::new(self.config.clone());
        for path in &self.paths {
            package.tables.add_path(path);
        }
        for entry in &self.entries {
            package.tables.add_entry(entry);
        }
        package.evidence = self.evidence.clone();
        if let Some(id) = self.thumbnail {
            package.thumbnail = Some(Thumbnail::reopen(&bytes(store, id)?));
        }
        for layer in &self.layers {
            for chunk in &layer.chunks {
                let key = ChunkKey::new(chunk.wad.clone(), chunk.path.clone());
                let held = Chunk {
                    bytes: bytes(store, chunk.content)?,
                    origin: chunk.origin.origin(store)?,
                };
                if package.insert(layer.layer.clone(), key, held).is_some() {
                    let at = format!("{}/{}", chunk.wad, chunk.path);
                    return Err(ResumeError::Malformed(format!("two chunks at {at}")));
                }
            }
            for patch in &layer.patches {
                package.patch(layer.layer.clone(), patch.patch(store)?);
            }
        }
        Ok(package)
    }
}

fn bytes(store: &dyn BlobStore, id: ContentId) -> Result<ByteView, ResumeError> {
    store.get(id).ok_or(ResumeError::Store(id))
}

/// The snapshot after `build` (#ADR-0094). It names bytes by content id and holds none.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildSnapshot {
    format: u32,
    program: Program,
    settings: BuildSettings,
    package: PackageRecord,
    part: PackageRow,
    findings: Vec<Logged>,
}

impl StageSnapshot for BuildSnapshot {
    const STAGE: Stage = Stage::Build;
}

impl BuildSnapshot {
    /// The snapshot of the package `build` gave under `settings`, its bytes put into `store`,
    /// with its part of the report and the findings raised from the start of `read` to the
    /// end of `build`.
    pub fn of(
        package: &Package,
        part: PackageRow,
        (settings, findings): (BuildSettings, Vec<Logged>),
        store: &dyn BlobStore,
    ) -> Self {
        Self {
            format: SNAPSHOT_FORMAT,
            program: Program::this(),
            settings,
            package: PackageRecord::of(package, store),
            part,
            findings,
        }
    }

    /// The evidence the package embeds (#ADR-0041), when the run gave it one.
    pub fn evidence(&self) -> Option<&EvidenceDoc> {
        self.package.evidence.as_ref()
    }

    /// The program that wrote it.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The settings `build` read.
    pub fn settings(&self) -> &BuildSettings {
        &self.settings
    }

    /// The package as its report part holds it: every chunk and patch with the digest of its
    /// bytes, no patch applied.
    pub fn part(&self) -> &PackageRow {
        &self.part
    }

    /// Every chunk, layer by layer, by WAD and path, with the id of its bytes.
    pub fn chunks(&self) -> impl Iterator<Item = (&Layer, &WadName, &ChunkPath, ContentId)> {
        self.package.layers.iter().flat_map(|layer| {
            let chunks = layer.chunks.iter();
            chunks.map(move |chunk| (&layer.layer, &chunk.wad, &chunk.path, chunk.content))
        })
    }

    /// The findings from the start of `read` to the end of `build`, in order.
    pub fn findings(&self) -> &[Logged] {
        &self.findings
    }

    /// The package `build` gave, its bytes read back from `store`.
    pub fn package(&self, store: &dyn BlobStore) -> Result<Package, ResumeError> {
        self.package.package(store)
    }

    /// Refuses a run whose program or `build` settings are not the snapshot's.
    pub fn holds(&self, settings: &BuildSettings) -> Result<(), ResumeError> {
        if self.format != SNAPSHOT_FORMAT {
            return Err(ResumeError::Format { found: self.format });
        }
        self.program.resumes()?;
        if let Some(setting) = self.settings.differs(settings) {
            return Err(ResumeError::Setting(setting));
        }
        Ok(())
    }
}
