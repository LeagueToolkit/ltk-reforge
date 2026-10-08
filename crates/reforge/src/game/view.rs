//! The game a chunk store answers, with what callers ask of it again and again kept: a path's
//! content id, a bin parsed, the bins of a character's skin. A run compares hundreds of a mod's
//! files with the game's and reads the same skin bins at every stage, and a chunk is inflated
//! for each read.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, PoisonError};

use super::catalog::{MAX_CHAIN, SkinCatalog};
use super::install::{Install, SizeIndex};
use super::source::{Copies, GameBin, GameSkin, GameSource, MAX_SAME_SIZE_READS};
use super::{Build, Game, GameError};
use crate::model::hash::skin_paths;
use crate::model::rig::{self, MeshShape, Skeleton};
use crate::model::{
    BankClip, ChunkPath, ContentId, EventsBank, MAX_SLOT, SkinEntry, Slot, WadHash, WadName,
    name_hash, path_hash,
};

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

/// Where the game's chunks by size come from.
enum Sizes {
    /// An index a caller built from the game's chunks, or an empty one.
    Given(SizeIndex),
    /// The install's own.
    Install(Arc<Install>),
}

/// A [`Game`] asked typed questions, which keeps what it was asked.
///
/// A chunk the game holds and does not read is a fault, and every question that needs it fails:
/// a run judged against an install that does not read does not answer as if the file were
/// absent.
pub struct GameView {
    game: Box<dyn Game>,
    sizes: Sizes,
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
    /// A view of a game, with no chunk by size: a copy of a game file under another path is
    /// then not found.
    pub fn new(game: Box<dyn Game>) -> Self {
        Self::with(game, Sizes::Given(SizeIndex::new()))
    }

    /// A view of an install, with its own size index.
    pub fn install(install: Install) -> Self {
        let install = Arc::new(install);
        Self::with(Box::new(Arc::clone(&install)), Sizes::Install(install))
    }

    /// The chunks of the game's champion WADs by size, for a game that is no install.
    #[must_use]
    pub fn sizes(mut self, sizes: SizeIndex) -> Self {
        self.sizes = Sizes::Given(sizes);
        self
    }

    fn with(game: Box<dyn Game>, sizes: Sizes) -> Self {
        Self {
            game,
            sizes,
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

    fn size_index(&self) -> &SizeIndex {
        match &self.sizes {
            Sizes::Given(sizes) => sizes,
            Sizes::Install(install) => install.sizes(),
        }
    }

    /// The digest of the game's bytes at a path. `None` when the game has no chunk there.
    pub fn content(&self, path: WadHash) -> Result<Option<ContentId>, GameError> {
        self.contents.get_or(&path, || {
            let bytes = self.game.read(path)?;
            Ok(bytes.map(|bytes| ContentId::of(&bytes)))
        })
    }

    /// What a chunk's bytes read as, kept by path. `None` when the game has no chunk there.
    fn parsed<T: Clone>(
        &self,
        kept: &Kept<WadHash, Option<T>>,
        path: WadHash,
        parse: impl FnOnce(Vec<u8>) -> Option<T>,
    ) -> Result<Option<T>, GameError> {
        kept.get_or(&path, || Ok(self.game.read(path)?.and_then(parse)))
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
            if chain.len() >= MAX_CHAIN {
                let held = |path: WadHash| chain.iter().any(|(at, _)| *at == path);
                let links = chain.iter().flat_map(|(_, bin)| &bin.parsed.dependencies);
                let mut unread = links.map(|link| path_hash(link));
                if unread.any(|path| !held(path) && self.game.contains(path)) {
                    catalog.note_capped(slot);
                }
            }
            let entry = name_hash(&skin_paths::skin_entry(character, slot));
            catalog.add(slot, entry, chain);
        }
        catalog.close();
        Ok(Arc::new(catalog))
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

impl GameSource for GameView {
    fn build(&self) -> Option<&Build> {
        self.game.build()
    }

    fn describe(&self) -> String {
        self.game.describe()
    }

    fn wads(&self, path: WadHash) -> Vec<WadName> {
        self.game.wads(path)
    }

    fn contains(&self, path: WadHash) -> bool {
        self.game.contains(path)
    }

    fn holds(&self, path: WadHash, size: u64, content: ContentId) -> Result<bool, GameError> {
        Ok(self.game.size(path) == Some(size) && self.content(path)? == Some(content))
    }

    fn copies(
        &self,
        wad: &WadName,
        size: u64,
        content: ContentId,
        besides: WadHash,
    ) -> Result<Copies, GameError> {
        let same_size = self.size_index().same_size(wad, size);
        let others = same_size.iter().filter(|path| **path != besides);
        let mut copies = Copies {
            paths: Vec::new(),
            capped: others.clone().count() > MAX_SAME_SIZE_READS,
        };
        for &other in others.take(MAX_SAME_SIZE_READS) {
            if self.holds(other, size, content)? {
                copies.paths.push(other);
            }
        }
        Ok(copies)
    }

    fn bin(&self, path: WadHash) -> Result<Option<Arc<GameBin>>, GameError> {
        self.parsed(&self.bins, path, |bytes| GameBin::read(bytes).map(Arc::new))
    }

    fn skin(&self, character: &str, slot: Slot) -> Result<Option<Arc<GameSkin>>, GameError> {
        let key = (character.to_ascii_lowercase(), slot);
        self.skins.get_or(&key, || self.read_skin(&key.0, slot))
    }

    fn catalog(&self, character: &str) -> Result<Arc<SkinCatalog>, GameError> {
        let character = character.to_ascii_lowercase();
        self.catalogs
            .get_or(&character, || self.read_catalog(&character))
    }

    fn skeleton(&self, path: WadHash) -> Result<Option<Arc<Skeleton>>, GameError> {
        self.parsed(&self.skeletons, path, |bytes| {
            Skeleton::parse(&bytes).map(Arc::new)
        })
    }

    fn mesh_shape(&self, path: WadHash) -> Result<Option<Arc<MeshShape>>, GameError> {
        self.parsed(&self.meshes, path, |bytes| {
            MeshShape::parse(&bytes).map(Arc::new)
        })
    }

    fn clip_joints(&self, path: WadHash) -> Result<Option<Arc<[u32]>>, GameError> {
        self.parsed(&self.clips, path, |bytes| {
            rig::clip_joints(&bytes).map(Arc::from)
        })
    }

    fn events_bank(&self, path: WadHash) -> Result<Option<Arc<EventsBank>>, GameError> {
        self.parsed(&self.events_banks, path, |bytes| {
            EventsBank::read(&bytes).map(Arc::new)
        })
    }

    fn bank_clips(&self, path: WadHash) -> Result<Option<Arc<[BankClip]>>, GameError> {
        self.parsed(&self.bank_clips, path, |bytes| {
            BankClip::read(&bytes).map(Arc::from)
        })
    }
}

/// A game shared between its view and whoever else reads it: the install, whose size index the
/// view reads beside it.
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
