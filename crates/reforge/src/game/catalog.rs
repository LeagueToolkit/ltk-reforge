//! The skins the game has for one character: each skin's identity, rig files, links and clips,
//! and the objects its bins hold, by entry and by content, so that an object a mod pasted is
//! traced to the skin it came from (#ADR-0043).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use ltk_meta::BinObject;
use serde::Serialize;

use super::view::GameBin;
use crate::model::{AudioBlock, BinHash, ContentId, SkinEntry, Slot, WadHash, value};

/// How many bins of one skin are read for the catalog, the skin bin included: the skin bin and
/// what it links, breadth first.
pub const MAX_CHAIN: usize = 16;

/// The number a skin entry's classification holds for a chroma.
const CHROMA: u32 = 2;

/// What a skin of the game is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "class", content = "parent", rename_all = "lowercase")]
pub enum SkinClass {
    /// The base skin, which every player has.
    Base,
    /// A skin a player owns.
    Premium,
    /// A recolor of the skin at this slot.
    Chroma(Slot),
}

/// One skin of the game, as its skin entry describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSkin {
    /// Its slot.
    pub slot: Slot,
    /// What it is.
    pub class: SkinClass,
    /// The slot it counts as: a chroma's parent where the game ships the parent, else its own.
    pub canon: Slot,
    /// Its name, `LuxSkin07`.
    pub name: Option<String>,
    /// Its mesh file.
    pub mesh: Option<WadHash>,
    /// Its skeleton file.
    pub skeleton: Option<WadHash>,
    /// Its mesh's texture file.
    pub texture: Option<WadHash>,
    /// The resolver its entry names.
    pub resolver: Option<BinHash>,
    /// The graph its entry names.
    pub graph: Option<BinHash>,
    /// Its audio block: the tags and the bank units.
    pub audio: AudioBlock,
    /// Every animation file its graph names.
    pub clips: BTreeSet<WadHash>,
    /// The bins read for it, the skin bin first.
    pub chain: Vec<WadHash>,
}

impl CatalogSkin {
    /// The skin an entry describes at a slot.
    fn of(entry: &BinObject, slot: Slot) -> Self {
        let entry = SkinEntry::read(entry);
        let parent = || entry.parent().and_then(|p| u16::try_from(p).ok()).map(Slot);
        let class = match entry.classification() {
            _ if slot.is_base() => SkinClass::Base,
            Some(CHROMA) => match parent() {
                Some(parent) if parent != slot => SkinClass::Chroma(parent),
                _ => SkinClass::Premium,
            },
            _ => SkinClass::Premium,
        };
        let canon = match class {
            SkinClass::Chroma(parent) => parent,
            SkinClass::Base | SkinClass::Premium => slot,
        };
        Self {
            slot,
            class,
            canon,
            name: entry.skin_name().map(str::to_string),
            mesh: entry.mesh().map(|file| file.hash()),
            skeleton: entry.skeleton().map(|file| file.hash()),
            texture: entry.texture().map(|file| file.hash()),
            resolver: entry.resolver(),
            graph: entry.graph(),
            audio: entry.audio(),
            clips: BTreeSet::new(),
            chain: Vec::new(),
        }
    }
}

/// The animation files a graph names. A file named by its hash counts: a graph names no other
/// kind of file by hash.
pub fn graph_clips(graph: &BinObject) -> BTreeSet<WadHash> {
    let is_clip = |file: &value::FileRef| {
        let spelled = file.spelled();
        spelled.is_none_or(|path| path.to_ascii_lowercase().ends_with(".anm"))
    };
    let files = value::file_refs(graph).into_iter().filter(is_clip);
    files.map(|file| file.hash()).collect()
}

/// A game object and the lowest skin whose bins hold it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GameObject {
    /// Its entry hash.
    pub entry: BinHash,
    /// The lowest slot whose bins hold it.
    pub slot: Slot,
}

/// The skins the game has for one character.
#[derive(Debug, Default)]
pub struct SkinCatalog {
    /// The skins whose bin holds their entry, by slot.
    pub skins: BTreeMap<Slot, CatalogSkin>,
    /// The object of each entry: the bin of the first skin holding it, and its place there.
    objects: HashMap<BinHash, (Arc<GameBin>, usize)>,
    /// By the digest of an object's bytes from its entry hash on.
    by_raw: HashMap<ContentId, GameObject>,
    /// By the digest of an object's bytes after its entry hash.
    by_body: HashMap<ContentId, Vec<GameObject>>,
    /// The slots whose bins hold each entry.
    entry_slots: HashMap<BinHash, BTreeSet<Slot>>,
    /// The slots at which the game has a skin bin.
    bins: BTreeSet<Slot>,
}

impl SkinCatalog {
    /// The skin at a slot.
    pub fn skin(&self, slot: Slot) -> Option<&CatalogSkin> {
        self.skins.get(&slot)
    }

    /// The base skin.
    pub fn base(&self) -> Option<&CatalogSkin> {
        self.skin(Slot::BASE)
    }

    /// The slot a slot counts as: a chroma's parent, else itself.
    pub fn canon(&self, slot: Slot) -> Slot {
        self.skin(slot).map_or(slot, |skin| skin.canon)
    }

    /// Whether the skin at a slot is a chroma.
    pub fn is_chroma(&self, slot: Slot) -> bool {
        let class = self.skin(slot).map(|skin| skin.class);
        matches!(class, Some(SkinClass::Chroma(_)))
    }

    /// The chromas of the skin at a slot.
    pub fn chromas_of(&self, slot: Slot) -> Vec<Slot> {
        let chromas = self.skins.values();
        let chromas = chromas.filter(|skin| skin.class == SkinClass::Chroma(slot));
        chromas.map(|skin| skin.slot).collect()
    }

    /// Whether the game has a skin bin for the character at a slot.
    pub fn has_bin(&self, slot: Slot) -> bool {
        self.bins.contains(&slot)
    }

    /// The game's object under an entry hash, of the first skin whose bins hold one.
    pub fn object(&self, entry: BinHash) -> Option<&BinObject> {
        let (bin, at) = self.objects.get(&entry)?;
        bin.parsed.objects.get(*at).map(|object| &object.value)
    }

    /// How many entries the skins' bins hold.
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// The game object with these bytes from its entry hash on.
    pub fn exact(&self, raw: ContentId) -> Option<GameObject> {
        self.by_raw.get(&raw).copied()
    }

    /// The game objects with these bytes after their entry hash, in the order read.
    pub fn renamed(&self, body: ContentId) -> &[GameObject] {
        self.by_body.get(&body).map_or(&[], Vec::as_slice)
    }

    /// The slots whose bins hold an entry.
    pub fn slots_of(&self, entry: BinHash) -> Option<&BTreeSet<Slot>> {
        self.entry_slots.get(&entry)
    }

    /// Notes that the game has a skin bin at a slot.
    pub(super) fn note_bin(&mut self, slot: Slot) {
        self.bins.insert(slot);
    }

    /// Takes in the bins read for the skin at a slot, the skin bin first, and the skin when
    /// the skin bin holds the entry of `entry` hash.
    pub(super) fn add(&mut self, slot: Slot, entry: BinHash, chain: Vec<(WadHash, Arc<GameBin>)>) {
        let mut found = None;
        for (at, (_, bin)) in chain.iter().enumerate() {
            for (place, object) in bin.parsed.objects.iter().enumerate() {
                let hash = object.value.path_hash;
                let held = GameObject { entry: hash, slot };
                self.by_raw.entry(object.raw).or_insert(held);
                let same_body = self.by_body.entry(object.body).or_default();
                if !same_body.contains(&held) {
                    same_body.push(held);
                }
                self.entry_slots.entry(hash).or_default().insert(slot);
                let first = self.objects.entry(hash);
                first.or_insert_with(|| (Arc::clone(bin), place));
                if hash == entry && at == 0 {
                    found = Some(&object.value);
                }
            }
        }
        let Some(found) = found else {
            return;
        };
        let mut skin = CatalogSkin::of(found, slot);
        if let Some(graph) = skin.graph.and_then(|graph| self.object(graph)) {
            skin.clips = graph_clips(graph);
        }
        skin.chain = chain.into_iter().map(|(path, _)| path).collect();
        self.skins.insert(slot, skin);
    }

    /// Makes a chroma whose parent the game does not ship count as itself.
    pub(super) fn close(&mut self) {
        let slots: BTreeSet<Slot> = self.skins.keys().copied().collect();
        for skin in self.skins.values_mut() {
            if let SkinClass::Chroma(parent) = skin.class
                && !slots.contains(&parent)
            {
                skin.canon = skin.slot;
            }
        }
    }
}
