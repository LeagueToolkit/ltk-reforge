//! The typed questions a stage asks the game it is judged against (#ADR-0080), and the answers
//! that are types of their own: a game bin, a skin's bins, the copies of a file's bytes.
//!
//! `docs/design/ARCHITECTURE.md`, "The environment".

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::catalog::SkinCatalog;
use super::{Build, GameError};
use crate::model::bin::{self, ParsedBin, ParsedObject};
use crate::model::rig::{MeshShape, Skeleton};
use crate::model::{
    BankClip, BinHash, ChunkPath, ContentId, EventsBank, Slot, WadHash, WadName, value,
};

/// How many game chunks of a file's size are read to look for its bytes under another path.
// #ADR-0070, measured 2026-10-06: a file of the datasets meets at most 1,167 chunks of its size
// in its champion's WAD at 16.19.
pub const MAX_SAME_SIZE_READS: usize = 2048;

/// The paths of one WAD where the game holds a file's bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Copies {
    /// The paths, in the order the source found them.
    pub paths: Vec<WadHash>,
    /// Whether [`MAX_SAME_SIZE_READS`] stopped the search, so that more paths may hold them.
    pub capped: bool,
}

/// The game a run is judged against, asked typed questions (#ADR-0080). `GameView` answers them
/// from the target install. No question gives a chunk's bytes.
///
/// A file that is absent, and one that does not read as what is asked, answer `None`. A file
/// the game holds and does not give is a [`GameError`]: a run judged against a game that does
/// not answer does not answer as if the file were absent.
pub trait GameSource: Send + Sync {
    /// The game's build, when it says.
    fn build(&self) -> Option<&Build>;

    /// Where the game was read from, for the report.
    fn describe(&self) -> String;

    /// Every WAD holding the path, the one it is read from first. Empty when none holds it.
    fn wads(&self, path: WadHash) -> Vec<WadName>;

    /// Whether the game has a file at a path.
    fn contains(&self, path: WadHash) -> bool {
        !self.wads(path).is_empty()
    }

    /// The WAD the game reads a path from.
    fn wad_of(&self, path: WadHash) -> Option<WadName> {
        self.wads(path).into_iter().next()
    }

    /// Whether the game holds these bytes at a path: a file of this size and digest.
    fn holds(&self, path: WadHash, size: u64, content: ContentId) -> Result<bool, GameError>;

    /// The paths of the champion WAD `wad`, other than `besides`, at which the game holds these
    /// bytes. None of a WAD that is no champion's.
    fn copies(
        &self,
        wad: &WadName,
        size: u64,
        content: ContentId,
        besides: WadHash,
    ) -> Result<Copies, GameError>;

    /// The game's bin at a path, parsed. `None` when the game has no file there, or the file
    /// is no bin.
    fn bin(&self, path: WadHash) -> Result<Option<Arc<GameBin>>, GameError>;

    /// The game's bins of a character's skin at a slot. `None` when the game has no skin bin
    /// there, or the file there is no bin.
    fn skin(&self, character: &str, slot: Slot) -> Result<Option<Arc<GameSkin>>, GameError>;

    /// The skins the game has for a character. Empty for a character the game has no skin bin
    /// of.
    fn catalog(&self, character: &str) -> Result<Arc<SkinCatalog>, GameError>;

    /// The game's skeleton at a path. `None` when the game has no file there, or the file reads
    /// as no skeleton.
    fn skeleton(&self, path: WadHash) -> Result<Option<Arc<Skeleton>>, GameError>;

    /// The shape of the game's mesh at a path. `None` when the game has no file there, or the
    /// file reads as no mesh.
    fn mesh_shape(&self, path: WadHash) -> Result<Option<Arc<MeshShape>>, GameError>;

    /// The hashes of the joints the game's clip at a path animates. `None` when the game has no
    /// file there, or the file reads as no clip.
    fn clip_joints(&self, path: WadHash) -> Result<Option<Arc<[u32]>>, GameError>;

    /// The header and the objects of the game's events bank at a path. `None` when the game has
    /// no file there, or the file reads as no sound bank.
    fn events_bank(&self, path: WadHash) -> Result<Option<Arc<EventsBank>>, GameError>;

    /// The clips of the game's bank or package at a path, each by its id, its size and the key
    /// of its bytes. `None` when the game has no file there, or the file reads as neither.
    fn bank_clips(&self, path: WadHash) -> Result<Option<Arc<[BankClip]>>, GameError>;
}

/// A bin of the game, parsed.
#[derive(Debug)]
pub struct GameBin {
    /// Its bytes.
    pub bytes: Arc<[u8]>,
    /// Its links and its objects.
    pub parsed: ParsedBin,
    /// The files its objects name.
    pub files: BTreeSet<WadHash>,
    /// The place of each object in `parsed.objects`, by entry hash.
    places: BTreeMap<BinHash, usize>,
}

impl GameBin {
    /// The bin of a file's bytes. `None` for bytes that are no bin.
    pub fn read(bytes: Vec<u8>) -> Option<Self> {
        let parsed = bin::parse(&bytes).ok()?;
        let named = parsed.objects.iter();
        let files = named
            .flat_map(|object| value::file_refs(&object.value))
            .map(|file| file.hash())
            .collect();
        let placed = parsed.objects.iter().enumerate();
        let places = placed.map(|(at, o)| (o.value.path_hash, at)).collect();
        Some(Self {
            bytes: bytes.into(),
            parsed,
            files,
            places,
        })
    }

    /// The object of the bin under an entry hash.
    pub fn object(&self, entry: BinHash) -> Option<&ParsedObject> {
        self.parsed.objects.get(*self.places.get(&entry)?)
    }
}

/// The game's bins of one skin of a character.
#[derive(Debug)]
pub struct GameSkin {
    /// Whether the skin is a chroma: its skin entry names a parent.
    pub chroma: bool,
    /// The skin bin, then the bins it links in the character's own folder, in link order: its
    /// animation bin and the bins shared between skins. The root bin is left out: it holds the
    /// character's and its spells' data.
    pub bins: Vec<(ChunkPath, Arc<GameBin>)>,
}
