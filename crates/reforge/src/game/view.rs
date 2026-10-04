//! A game with what callers ask of it again and again kept: a path's content id, a bin parsed,
//! the bins of a character's skin. A run compares hundreds of a mod's files with the game's and
//! reads the same skin bins at every stage, and a chunk is inflated for each read.

use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;
use std::sync::{Arc, Mutex, PoisonError};

use super::catalog::{MAX_CHAIN, SkinCatalog};
use super::{Build, Game, GameError};
use crate::model::bin::{self, ParsedBin, ParsedObject};
use crate::model::hash::skin_paths;
use crate::model::rig::{self, MeshShape, Skeleton};
use crate::model::{
    BankClip, BinHash, ChunkPath, ContentId, EventsBank, MAX_SLOT, SkinEntry, Slot, WadHash,
    WadName, name_hash, path_hash, value,
};

/// A bin of the game, parsed.
#[derive(Debug)]
pub struct GameBin {
    /// Its bytes.
    pub bytes: Arc<[u8]>,
    /// Its links and its objects.
    pub parsed: ParsedBin,
    /// The files its objects name.
    pub files: BTreeSet<WadHash>,
}

impl GameBin {
    /// The object of the bin under an entry hash.
    pub fn object(&self, entry: BinHash) -> Option<&ParsedObject> {
        let mut objects = self.parsed.objects.iter();
        objects.find(|object| object.value.path_hash == entry)
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

/// Answers kept by key. An answer is computed outside the lock, so two threads may compute one
/// twice; both get the same value.
struct Kept<K, V> {
    answers: Mutex<HashMap<K, V>>,
}

impl<K, V> Default for Kept<K, V> {
    fn default() -> Self {
        Self {
            answers: Mutex::new(HashMap::new()),
        }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Kept<K, V> {
    /// The answer kept, else the one computed. A question that fails keeps nothing, so it is
    /// asked again.
    fn get_or<E>(&self, key: &K, compute: impl FnOnce() -> Result<V, E>) -> Result<V, E> {
        // A panic while the map is locked leaves it whole: an insert is all that happens there.
        let lock = || self.answers.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(answer) = lock().get(key) {
            return Ok(answer.clone());
        }
        let answer = compute()?;
        Ok(lock().entry(key.clone()).or_insert(answer).clone())
    }
}

/// A [`Game`] that keeps what it was asked. It answers as the game it wraps does, and adds the
/// questions that cost a read.
///
/// A chunk the game holds and does not read is a fault, and every question that needs it fails:
/// a run judged against an install that does not read does not answer as if the file were
/// absent.
pub struct GameView {
    game: Box<dyn Game>,
    contents: Kept<WadHash, Option<ContentId>>,
    bins: Kept<WadHash, Option<Arc<GameBin>>>,
    skins: Kept<(String, Slot), Option<Arc<GameSkin>>>,
    catalogs: Kept<String, Arc<SkinCatalog>>,
    skeletons: Kept<WadHash, Option<Arc<Skeleton>>>,
    meshes: Kept<WadHash, Option<Arc<MeshShape>>>,
    clips: Kept<WadHash, Option<Arc<[u32]>>>,
    events_banks: Kept<WadHash, Option<Arc<EventsBank>>>,
    bank_clips: Kept<WadHash, Option<Arc<[BankClip]>>>,
}

impl GameView {
    /// A view of a game.
    pub fn new(game: Box<dyn Game>) -> Self {
        Self {
            game,
            contents: Kept::default(),
            bins: Kept::default(),
            skins: Kept::default(),
            catalogs: Kept::default(),
            skeletons: Kept::default(),
            meshes: Kept::default(),
            clips: Kept::default(),
            events_banks: Kept::default(),
            bank_clips: Kept::default(),
        }
    }

    /// The skins the game has for a character (#ADR-0043). Empty for a character the game has
    /// no skin bin of.
    pub fn catalog(&self, character: &str) -> Result<Arc<SkinCatalog>, GameError> {
        let character = character.to_ascii_lowercase();
        self.catalogs
            .get_or(&character, || self.read_catalog(&character))
    }

    fn read_catalog(&self, character: &str) -> Result<Arc<SkinCatalog>, GameError> {
        let mut catalog = SkinCatalog::default();
        for slot in (0..=MAX_SLOT).map(Slot) {
            let skin_bin = path_hash(&skin_paths::skin_bin(character, slot));
            if !self.game.contains(skin_bin) {
                continue;
            }
            catalog.note_bin(slot);
            let Some(skin) = self.bin(skin_bin)? else {
                continue;
            };
            let mut chain = vec![(skin_bin, skin)];
            let mut at = 0;
            while at < chain.len() && chain.len() < MAX_CHAIN {
                let links = chain[at].1.parsed.dependencies.clone();
                for link in links {
                    let path = path_hash(&link);
                    let known = chain.iter().any(|(held, _)| *held == path);
                    if chain.len() >= MAX_CHAIN || known {
                        continue;
                    }
                    if let Some(bin) = self.bin(path)? {
                        chain.push((path, bin));
                    }
                }
                at += 1;
            }
            let entry = name_hash(&skin_paths::skin_entry(character, slot));
            catalog.add(slot, entry, chain);
        }
        catalog.close();
        Ok(Arc::new(catalog))
    }

    /// The game's skeleton at a path (#ADR-0043). `None` when the game has no file there, or
    /// the file reads as no skeleton.
    pub fn skeleton(&self, path: WadHash) -> Result<Option<Arc<Skeleton>>, GameError> {
        self.skeletons.get_or(&path, || {
            let bytes = self.game.read(path)?;
            Ok(bytes
                .and_then(|bytes| Skeleton::parse(&bytes))
                .map(Arc::new))
        })
    }

    /// The shape of the game's mesh at a path (#ADR-0043). `None` when the game has no file
    /// there, or the file reads as no mesh.
    pub fn mesh_shape(&self, path: WadHash) -> Result<Option<Arc<MeshShape>>, GameError> {
        self.meshes.get_or(&path, || {
            let bytes = self.game.read(path)?;
            Ok(bytes
                .and_then(|bytes| MeshShape::parse(&bytes))
                .map(Arc::new))
        })
    }

    /// The hashes of the joints the game's clip at a path animates (#ADR-0043). `None` when
    /// the game has no file there, or the file reads as no clip.
    pub fn clip_joints(&self, path: WadHash) -> Result<Option<Arc<[u32]>>, GameError> {
        self.clips.get_or(&path, || {
            let bytes = self.game.read(path)?;
            let joints = bytes.and_then(|bytes| rig::clip_joints(&bytes));
            Ok(joints.map(Arc::from))
        })
    }

    /// The header and the objects of the game's events bank at a path (#ADR-0043). `None`
    /// when the game has no file there, or the file reads as no sound bank.
    pub fn events_bank(&self, path: WadHash) -> Result<Option<Arc<EventsBank>>, GameError> {
        self.events_banks.get_or(&path, || {
            let bytes = self.game.read(path)?;
            let bank = bytes.and_then(|bytes| EventsBank::read(&bytes));
            Ok(bank.map(Arc::new))
        })
    }

    /// The clips of the game's bank or package at a path, each by its id, its size and the
    /// key of its bytes (#ADR-0043). `None` when the game has no file there, or the file
    /// reads as neither.
    pub fn bank_clips(&self, path: WadHash) -> Result<Option<Arc<[BankClip]>>, GameError> {
        self.bank_clips.get_or(&path, || {
            let bytes = self.game.read(path)?;
            let clips = bytes.and_then(|bytes| BankClip::read(&bytes));
            Ok(clips.map(Arc::from))
        })
    }

    /// The digest of the game's bytes at a path. `None` when the game has no chunk there.
    pub fn content(&self, path: WadHash) -> Result<Option<ContentId>, GameError> {
        self.contents.get_or(&path, || {
            let bytes = self.game.read(path)?;
            Ok(bytes.map(|bytes| ContentId::of(&bytes)))
        })
    }

    /// Whether the game holds these bytes at a path: a file of this size and digest. A chunk
    /// of another size is not read.
    pub fn holds(&self, path: WadHash, size: u64, content: ContentId) -> Result<bool, GameError> {
        Ok(self.game.size(path) == Some(size) && self.content(path)? == Some(content))
    }

    /// The WAD the game reads a path from.
    pub fn wad_of(&self, path: WadHash) -> Option<WadName> {
        self.game.wads(path).into_iter().next()
    }

    /// The game's bin at a path, parsed. `None` when the game has no file there, or the file
    /// is no bin.
    pub fn bin(&self, path: WadHash) -> Result<Option<Arc<GameBin>>, GameError> {
        self.bins.get_or(&path, || {
            let Some(bytes) = self.game.read(path)? else {
                return Ok(None);
            };
            let Ok(parsed) = bin::parse(&bytes) else {
                return Ok(None);
            };
            let named = parsed.objects.iter();
            let files = named
                .flat_map(|object| value::file_refs(&object.value))
                .map(|file| file.hash())
                .collect();
            Ok(Some(Arc::new(GameBin {
                bytes: bytes.into(),
                parsed,
                files,
            })))
        })
    }

    /// The game's bins of a character's skin at a slot. `None` when the game has no skin bin
    /// there, or the file there is no bin.
    pub fn skin(&self, character: &str, slot: Slot) -> Result<Option<Arc<GameSkin>>, GameError> {
        let character = character.to_ascii_lowercase();
        let key = (character, slot);
        self.skins.get_or(&key, || self.read_skin(&key.0, slot))
    }

    fn read_skin(&self, character: &str, slot: Slot) -> Result<Option<Arc<GameSkin>>, GameError> {
        let Ok(path) = ChunkPath::parse(&skin_paths::skin_bin(character, slot)) else {
            return Ok(None);
        };
        let Some(skin) = self.bin(path.hash())? else {
            return Ok(None);
        };
        let entries = skin.parsed.objects.iter();
        let chroma = entries
            .filter_map(|object| SkinEntry::of(&object.value))
            .any(|entry| entry.is_chroma());
        let folder = format!("data/characters/{character}/");
        let root = skin_paths::root_bin(character);
        let mut bins = Vec::new();
        for link in &skin.parsed.dependencies {
            let Ok(linked) = ChunkPath::parse(link) else {
                continue;
            };
            if !linked.as_str().starts_with(&folder) || linked.as_str() == root {
                continue;
            }
            if let Some(bin) = self.bin(linked.hash())? {
                bins.push((linked, bin));
            }
        }
        bins.insert(0, (path, skin));
        Ok(Some(Arc::new(GameSkin { chroma, bins })))
    }
}

impl Game for GameView {
    fn build(&self) -> Option<&Build> {
        self.game.build()
    }

    fn describe(&self) -> String {
        self.game.describe()
    }

    fn size(&self, path: WadHash) -> Option<u64> {
        self.game.size(path)
    }

    fn read(&self, path: WadHash) -> Result<Option<Vec<u8>>, GameError> {
        self.game.read(path)
    }

    fn wads(&self, path: WadHash) -> Vec<WadName> {
        self.game.wads(path)
    }

    fn contains(&self, path: WadHash) -> bool {
        self.game.contains(path)
    }
}

/// A game shared between its view and whoever else reads it: the install, whose size index a
/// run reads beside it.
impl<G: Game + ?Sized> Game for Arc<G> {
    fn build(&self) -> Option<&Build> {
        (**self).build()
    }

    fn describe(&self) -> String {
        (**self).describe()
    }

    fn size(&self, path: WadHash) -> Option<u64> {
        (**self).size(path)
    }

    fn read(&self, path: WadHash) -> Result<Option<Vec<u8>>, GameError> {
        (**self).read(path)
    }

    fn wads(&self, path: WadHash) -> Vec<WadName> {
        (**self).wads(path)
    }

    fn contains(&self, path: WadHash) -> bool {
        (**self).contains(path)
    }
}

impl std::fmt::Debug for GameView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameView")
            .field("game", &self.game.describe())
            .finish_non_exhaustive()
    }
}
