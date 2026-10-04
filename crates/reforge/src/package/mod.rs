//! The output of a run and everything after it: the package, its builders, the checks and the
//! writers.
//!
//! A [`Package`] is built in memory, by [`InPlaceBuilder`] for a repair and by [`PortBuilder`]
//! for a port. An [`Overlay`] is a port's patches applied to the game's bins. A [`CheckSet`]
//! reads both and raises what it finds. A [`PackageWriter`] puts the package on disk. Nothing
//! a check finds stops the write (#ADR-0003): a caller's [`permit`] does, when its policy
//! says so. An [`EvidenceDoc`] is what the run established of the mod, which a writer embeds
//! in the package (#ADR-0041).
//!
//! `docs/design/ARCHITECTURE.md`, "Package".

mod check;
mod config;
mod evidence;
mod findings;
mod inplace;
mod overlay;
mod port;
mod ptch;
mod texture;
mod thumbnail;
mod wired;
mod write;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;

use ltk_meta::BinObject;
use serde::Serialize;

pub use self::check::{
    Apply, Check, CheckCx, CheckId, CheckInfo, CheckSelection, CheckSet, RoundTrip, Types,
};
pub use self::config::{Made, ProjectConfig};
pub use self::evidence::{
    ArchiveRef, EVIDENCE_FORMAT, EvidenceBound, EvidenceDating, EvidenceDoc, EvidenceError,
    EvidenceHistory, EvidenceInputs, EvidencePiece, EvidencePolicy, EvidencePort, EvidenceUnit,
    MAX_README_BYTES, PieceKind, Program, README_FILE,
};
pub use self::findings::{
    Against, ApplyFailed, Blocked, Coverage, Done, Empty, EntryWhole, FieldKept, FieldUndeclared,
    FileMissing, InPlace, LinkUnresolved, ManifestUnreadable, NameUnresolved, NewBinFailed,
    NotWritten, Place, Ported, RecordSkipped, ResolverFrozen, RoundTripFailed, RoundTripProblem,
    Shape, TextureContained, ThumbnailDropped, TypeMismatch, TypeRow, UndeclaredRow, UnitPorted,
    Written,
};
pub use self::inplace::{BuildError, InPlaceBuilder};
pub use self::overlay::{AppliedBin, AppliedPatch, Held, Loaded, Outcome, Overlay};
pub use self::port::PortBuilder;
pub use self::ptch::{PtchBuilder, Unnamed};
pub use self::texture::{TexContainer, TexRefusal, dds_to_tex};
pub use self::thumbnail::{THUMBNAIL_FILE, Thumbnail, ThumbnailRefusal};
pub use self::wired::{LeftBehind, LeftOut, Wired};
pub use self::write::{
    MANIFEST_FILE, MAX_NAME_BYTES, Modpkg, PackageWriter, ProjectDir, REPORT_FILE, Refused,
    WriteError, WriterKind, permit,
};
use crate::diag::{Code, Diag};
pub use crate::model::Layer;
use crate::model::entity::{CharId, FileId, UnitId};
use crate::model::{BinHash, ChunkPath, Dump, DumpCx, WadHash, WadName};

/// Every diagnostic code this module raises.
pub const CODES: &[Code] = &[
    NotWritten::CODE,
    NewBinFailed::CODE,
    UnitPorted::CODE,
    FieldKept::CODE,
    EntryWhole::CODE,
    ResolverFrozen::CODE,
    Ported::CODE,
    TextureContained::CODE,
    InPlace::CODE,
    Empty::CODE,
    ThumbnailDropped::CODE,
    ManifestUnreadable::CODE,
    ApplyFailed::CODE,
    RecordSkipped::CODE,
    LinkUnresolved::CODE,
    NameUnresolved::CODE,
    FileMissing::CODE,
    TypeMismatch::CODE,
    FieldUndeclared::CODE,
    RoundTripFailed::CODE,
    Done::CODE,
    Written::CODE,
    Blocked::CODE,
];

/// Where a chunk goes: a WAD and a path in it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ChunkKey {
    /// The WAD.
    pub wad: WadName,
    /// The path in the WAD.
    pub path: ChunkPath,
}

impl ChunkKey {
    /// The chunk at a path of a WAD.
    pub fn new(wad: WadName, path: ChunkPath) -> Self {
        Self { wad, path }
    }

    /// The hash the game finds the chunk by: the hash of its path, or for a chunk no table
    /// names the hash its name at the WAD's root spells (#MOD-ARTIFACT-0007).
    pub fn hash(&self) -> WadHash {
        let name = self.path.as_str();
        let stem = name.split('.').next().unwrap_or(name);
        let spelled = !name.contains('/') && stem.len() == 16;
        let spelled = spelled
            .then(|| u64::from_str_radix(stem, 16).ok())
            .flatten();
        spelled.map_or_else(|| self.path.hash(), WadHash)
    }
}

/// How a bin was written again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BinWrite {
    /// Through `ltk_meta`'s delta writer: every object no fixup changed is copied byte for
    /// byte.
    Delta,
    /// Every object encoded again: the delta writer refuses a bin of the old numbering.
    Whole,
}

/// What a bin written again was built to hold: what [`RoundTrip`] reads it back against.
#[derive(Debug, Clone, PartialEq)]
pub struct WrittenBin {
    /// How it was written.
    pub how: BinWrite,
    /// The entry hash of every object the bin holds.
    pub entries: BTreeSet<BinHash>,
    /// The objects a fixup changed, as they were written.
    pub edited: Vec<BinObject>,
}

/// Where a chunk's bytes come from.
#[derive(Debug, Clone, PartialEq)]
pub enum Origin {
    /// A file of the mod, with the bytes it has.
    Kept(FileId),
    /// A bin of the mod, written again.
    Rewritten {
        /// The file the bin was read from.
        file: FileId,
        /// What the bytes were built to hold.
        written: WrittenBin,
    },
    /// A DDS texture of the mod, in TEX's container.
    Contained(FileId),
    /// A bin a port made: its carried objects, its graph, or a skin bin the game does not
    /// ship. No file of the mod holds it.
    Made(WrittenBin),
    /// A bank a port made: the own clips of a bank of the mod, a header beside a package, or
    /// the events that play them.
    Bank(FileId),
}

impl Origin {
    /// The file of the mod the chunk was built from. None for a bin a port made.
    pub fn file(&self) -> Option<FileId> {
        match self {
            Origin::Kept(file)
            | Origin::Contained(file)
            | Origin::Rewritten { file, .. }
            | Origin::Bank(file) => Some(*file),
            Origin::Made(_) => None,
        }
    }

    /// What a bin written by the run was built to hold.
    pub fn written(&self) -> Option<&WrittenBin> {
        match self {
            Origin::Rewritten { written, .. } | Origin::Made(written) => Some(written),
            Origin::Kept(_) | Origin::Contained(_) | Origin::Bank(_) => None,
        }
    }

    /// The origin's name in a report and a dump.
    pub fn name(&self) -> &'static str {
        match self {
            Origin::Kept(_) => "kept",
            Origin::Rewritten { written, .. } => match written.how {
                BinWrite::Delta => "delta",
                BinWrite::Whole => "whole",
            },
            Origin::Contained(_) => "contained",
            Origin::Made(_) => "made",
            Origin::Bank(_) => "bank",
        }
    }
}

/// What a patch is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "of", rename_all = "lowercase")]
pub enum PatchOf {
    /// A unit's skin bin at its target.
    Skin {
        /// The unit.
        unit: UnitId,
        /// The entry hash of the skin entry it patches.
        #[serde(skip)]
        entry: BinHash,
    },
    /// A champion's root bin.
    Root {
        /// The champion.
        character: CharId,
    },
}

/// A patch of a game bin (#ADR-0026): the PTCH and the entry of the layer's manifest that
/// applies it and adds its links.
#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    /// The game bin it patches.
    pub target: ChunkPath,
    /// Where the PTCH is, in its layer.
    pub path: ChunkPath,
    /// The PTCH.
    pub bytes: Arc<[u8]>,
    /// The bins the manifest links to the patched bin.
    pub links: Vec<ChunkPath>,
    /// What it is of.
    pub of: PatchOf,
    /// How many records and whole objects the PTCH holds.
    pub records: usize,
}

/// One chunk of a package.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    /// Its bytes, as a writer writes them.
    pub bytes: Arc<[u8]>,
    /// Where they come from.
    pub origin: Origin,
}

impl Chunk {
    /// A file of the mod with the bytes it has.
    pub fn kept(file: FileId, bytes: Arc<[u8]>) -> Self {
        Self {
            bytes,
            origin: Origin::Kept(file),
        }
    }
}

/// The names a package ships for the hashes it holds: what a project's hash tables list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HashTables {
    paths: BTreeSet<String>,
    entries: BTreeSet<String>,
}

impl HashTables {
    /// Whether a table file holds the name: printable ASCII with no backslash. `ltk_hashtable`
    /// refuses a table with any other line, and a project with it does not pack.
    fn holds(name: &str) -> bool {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| (0x20..=0x7e).contains(&byte) && byte != b'\\')
    }

    /// Adds a chunk path. A name a table file cannot hold is left out.
    pub fn add_path(&mut self, path: &str) -> bool {
        Self::holds(path) && self.paths.insert(path.to_string())
    }

    /// Adds an entry name. A name a table file cannot hold is left out.
    pub fn add_entry(&mut self, name: &str) -> bool {
        Self::holds(name) && self.entries.insert(name.to_string())
    }

    /// The chunk paths, sorted.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.paths.iter().map(String::as_str)
    }

    /// The entry names, sorted.
    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(String::as_str)
    }
}

/// What a run writes: layers of chunks, the hash tables, the project's config, the evidence
/// of the run and the thumbnail. Only checked paths are in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    /// The project's config.
    pub config: ProjectConfig,
    /// The names shipped for the hashes the package holds.
    pub tables: HashTables,
    /// The evidence of the run that built the package (#ADR-0041), which a writer embeds. None
    /// for a package no run built.
    pub evidence: Option<EvidenceDoc>,
    /// The mod's image as a writer writes it (#ADR-0051). None for a mod with no image, or
    /// with one that did not convert.
    pub thumbnail: Option<Thumbnail>,
    layers: BTreeMap<Layer, BTreeMap<ChunkKey, Chunk>>,
    patches: BTreeMap<Layer, Vec<Patch>>,
}

impl Package {
    /// A package with its base layer, no chunk and no evidence.
    pub fn new(config: ProjectConfig) -> Self {
        Self {
            config,
            tables: HashTables::default(),
            evidence: None,
            thumbnail: None,
            layers: BTreeMap::from([(Layer::Base, BTreeMap::new())]),
            patches: BTreeMap::new(),
        }
    }

    /// Adds a patch to a layer, after the ones the layer holds.
    pub fn patch(&mut self, layer: Layer, patch: Patch) {
        self.layers.entry(layer.clone()).or_default();
        self.patches.entry(layer).or_default().push(patch);
    }

    /// The patches of a layer, in the order its manifest lists them.
    pub fn patches(&self, layer: &Layer) -> &[Patch] {
        self.patches.get(layer).map_or(&[], Vec::as_slice)
    }

    /// Every patch, layer by layer.
    pub fn all_patches(&self) -> impl Iterator<Item = (&Layer, &Patch)> {
        self.patches
            .iter()
            .flat_map(|(layer, patches)| patches.iter().map(move |patch| (layer, patch)))
    }

    /// A layer's manifest, `game_data.yaml`: one module per patch, which applies the PTCH to
    /// the game's bin and adds the links. None for a layer with no patch. A writer writes
    /// this text, and the apply check loads it.
    pub fn manifest(&self, layer: &Layer) -> Option<String> {
        let patches = self.patches(layer);
        if patches.is_empty() {
            return None;
        }
        // A JSON string is a YAML double-quoted scalar: a path may hold a space or a comma.
        let quoted = |path: &ChunkPath| serde_json::Value::from(path.as_str()).to_string();
        let mut text = String::from(
            "# Written by reforge: each module applies a PTCH to a game bin and links the \
             port's bins.\nversion: 1\nmodules:\n",
        );
        for patch in patches {
            let _ = writeln!(text, "  - target: {}", quoted(&patch.target));
            let _ = writeln!(text, "    edits:");
            let _ = writeln!(text, "      - overrides: [{}]", quoted(&patch.path));
            if !patch.links.is_empty() {
                let links: Vec<String> = patch.links.iter().map(quoted).collect();
                let _ = writeln!(text, "        links: [{}]", links.join(", "));
            }
        }
        Some(text)
    }

    /// Puts a chunk in a layer. The chunk the layer held at the key comes back.
    pub fn insert(&mut self, layer: Layer, key: ChunkKey, chunk: Chunk) -> Option<Chunk> {
        self.layers.entry(layer).or_default().insert(key, chunk)
    }

    /// The layers, the base layer first.
    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.layers.keys()
    }

    /// The chunks of a layer, by WAD and path.
    pub fn layer(&self, layer: &Layer) -> impl Iterator<Item = (&ChunkKey, &Chunk)> {
        self.layers.get(layer).into_iter().flatten()
    }

    /// Every chunk, layer by layer, by WAD and path.
    pub fn chunks(&self) -> impl Iterator<Item = (&Layer, &ChunkKey, &Chunk)> {
        self.layers
            .iter()
            .flat_map(|(layer, chunks)| chunks.iter().map(move |(key, chunk)| (layer, key, chunk)))
    }

    /// Every chunk, to replace its bytes.
    pub fn chunks_mut(&mut self) -> impl Iterator<Item = (&Layer, &ChunkKey, &mut Chunk)> {
        self.layers.iter_mut().flat_map(|(layer, chunks)| {
            chunks
                .iter_mut()
                .map(move |(key, chunk)| (layer, key, chunk))
        })
    }

    /// The chunks at a path ending `.bin`: what the client reads as bins.
    pub fn bins(&self) -> impl Iterator<Item = (&Layer, &ChunkKey, &Chunk)> {
        self.chunks()
            .filter(|(_, key, _)| key.path.extension() == Some("bin"))
    }

    /// The chunk of the base layer at a path of a WAD.
    pub fn get(&self, wad: &WadName, path: &ChunkPath) -> Option<&Chunk> {
        let key = ChunkKey::new(wad.clone(), path.clone());
        self.layers.get(&Layer::Base)?.get(&key)
    }

    /// How many chunks the package holds.
    pub fn len(&self) -> usize {
        self.layers.values().map(BTreeMap::len).sum()
    }

    /// Whether the package holds no chunk and no patch.
    pub fn is_empty(&self) -> bool {
        self.len() == 0 && self.patches.values().all(Vec::is_empty)
    }

    /// The chunk of any layer the client finds by a path hash.
    pub fn by_hash(&self, hash: WadHash) -> Option<&Chunk> {
        let mut chunks = self.chunks();
        chunks
            .find(|(_, key, _)| key.hash() == hash)
            .map(|(_, _, chunk)| chunk)
    }
}

impl Dump for Package {
    fn dump(&self, _: &DumpCx<'_>) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "package {} {} chunks={}",
            self.config.name,
            self.config.version,
            self.len()
        );
        for (layer, key, chunk) in self.chunks() {
            let file = chunk.origin.file();
            let file = file.map_or(String::new(), |file| format!(" {file}"));
            let _ = writeln!(
                out,
                "     {} {}/{} {} {}{file}",
                layer.name(),
                key.wad,
                key.path,
                chunk.bytes.len(),
                chunk.origin.name(),
            );
        }
        for (layer, patch) in self.all_patches() {
            let _ = writeln!(
                out,
                "     {} patch {} on {} records={} links={}",
                layer.name(),
                patch.path,
                patch.target,
                patch.records,
                patch.links.len()
            );
        }
        for path in self.tables.paths() {
            let _ = writeln!(out, "     names path {path}");
        }
        for entry in self.tables.entries() {
            let _ = writeln!(out, "     names entry {entry}");
        }
        if let Some(evidence) = &self.evidence {
            let _ = writeln!(
                out,
                "     evidence {EVIDENCE_FORMAT} fixups={} restored={}",
                evidence.fixups.len(),
                evidence.restored.len()
            );
        }
        out
    }
}
