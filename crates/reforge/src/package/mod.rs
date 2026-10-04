//! The output of a run and everything after it: the package, its builders, the checks and the
//! writers.
//!
//! A [`Package`] is built in memory, by [`InPlaceBuilder`] for a repair. A [`CheckSet`] reads
//! it and raises what it finds. A [`PackageWriter`] puts it on disk. Nothing a check finds
//! stops the write (#ADR-0003): a caller's [`permit`] does, when its policy says so.
//!
//! `docs/design/ARCHITECTURE.md`, "Package".

mod check;
mod config;
mod findings;
mod inplace;
mod texture;
mod wired;
mod write;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;

use ltk_meta::BinObject;
use serde::Serialize;

pub use self::check::{
    Check, CheckCx, CheckId, CheckInfo, CheckSelection, CheckSet, RoundTrip, Types,
};
pub use self::config::{Made, ProjectConfig};
pub use self::findings::{
    Against, Blocked, Coverage, Done, Empty, FieldUndeclared, InPlace, Place, RoundTripFailed,
    RoundTripProblem, Shape, TextureContained, TypeMismatch, TypeRow, UndeclaredRow, Written,
};
pub use self::inplace::{BuildError, InPlaceBuilder};
pub use self::texture::{TexContainer, TexRefusal, dds_to_tex};
pub use self::wired::{LeftBehind, LeftOut, Wired};
pub use self::write::{
    MAX_NAME_BYTES, Modpkg, PackageWriter, ProjectDir, REPORT_FILE, Refused, WriteError,
    WriterKind, permit,
};
use crate::diag::{Code, Diag};
use crate::model::entity::FileId;
use crate::model::{BinHash, ChunkPath, Dump, DumpCx, WadHash, WadName};

/// Every diagnostic code this module raises.
pub const CODES: &[Code] = &[
    TextureContained::CODE,
    InPlace::CODE,
    Empty::CODE,
    TypeMismatch::CODE,
    FieldUndeclared::CODE,
    RoundTripFailed::CODE,
    Done::CODE,
    Written::CODE,
    Blocked::CODE,
];

/// A part of a package a player turns on or off.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    /// What is always on.
    Base,
    /// The mod's own voice lines, of one locale when they come from a locale WAD.
    Voice(Option<String>),
    /// Spell icons and HUD textures.
    Hud,
}

impl Layer {
    /// The layer's name in a project and a package: `base`, `voice`, `voice-en-us`, `hud`.
    pub fn name(&self) -> String {
        match self {
            Layer::Base => "base".to_string(),
            Layer::Voice(None) => "voice".to_string(),
            Layer::Voice(Some(locale)) => format!("voice-{}", locale.replace('_', "-")),
            Layer::Hud => "hud".to_string(),
        }
    }

    /// The layer's name for a person.
    pub fn title(&self) -> String {
        match self {
            Layer::Base => "Base".to_string(),
            Layer::Voice(None) => "Voice".to_string(),
            Layer::Voice(Some(locale)) => format!("Voice ({locale})"),
            Layer::Hud => "Spell icons and HUD".to_string(),
        }
    }

    /// One line on what the layer holds.
    pub fn description(&self) -> String {
        match self {
            Layer::Base => "Base layer of the mod".to_string(),
            Layer::Voice(None) => "The mod's own voice lines".to_string(),
            Layer::Voice(Some(locale)) => {
                format!("The mod's own voice lines, from its {locale} WAD")
            }
            Layer::Hud => "The mod's spell icons and HUD textures; they show for the champion \
                           under every skin"
                .to_string(),
        }
    }
}

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
}

impl Origin {
    /// The file of the mod the chunk was built from.
    pub fn file(&self) -> FileId {
        match self {
            Origin::Kept(file) | Origin::Contained(file) | Origin::Rewritten { file, .. } => *file,
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
        }
    }
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

/// What a run writes: layers of chunks, the hash tables and the project's config. Only checked
/// paths are in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    /// The project's config.
    pub config: ProjectConfig,
    /// The names shipped for the hashes the package holds.
    pub tables: HashTables,
    layers: BTreeMap<Layer, BTreeMap<ChunkKey, Chunk>>,
}

impl Package {
    /// A package with its base layer and no chunk.
    pub fn new(config: ProjectConfig) -> Self {
        Self {
            config,
            tables: HashTables::default(),
            layers: BTreeMap::from([(Layer::Base, BTreeMap::new())]),
        }
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

    /// Whether the package holds no chunk.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
            let _ = writeln!(
                out,
                "     {} {}/{} {} {} {}",
                layer.name(),
                key.wad,
                key.path,
                chunk.bytes.len(),
                chunk.origin.name(),
                chunk.origin.file()
            );
        }
        for path in self.tables.paths() {
            let _ = writeln!(out, "     names path {path}");
        }
        for entry in self.tables.entries() {
            let _ = writeln!(out, "     names entry {entry}");
        }
        out
    }
}
