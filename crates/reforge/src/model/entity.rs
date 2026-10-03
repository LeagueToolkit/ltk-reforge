//! The mod: one archive as a run holds it. Every field is a fact `load` found. A target, a
//! destination and a dropped state belong to later stages and are not here.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ltk_meta::BinObject;
use serde::Serialize;

use super::hash::{BinHash, ContentId, MAX_SLOT, Slot, WadHash, path_hash, skin_paths};
use super::id::{Arena, Id, Sigil};
use super::path::{ChunkPath, WadName};
use super::text::Spelled;

/// The id of a WAD of the mod.
pub type WadId = Id<ModWad>;
/// The id of a file of the mod.
pub type FileId = Id<FileNode>;
/// The id of a bin the client loads.
pub type BinId = Id<BinNode>;
/// The id of an object of a loaded bin.
pub type ObjectId = Id<ObjectNode>;
/// The id of a character.
pub type CharId = Id<Character>;
/// The id of a skin unit.
pub type UnitId = Id<SkinUnit>;

impl Sigil for ModWad {
    const SIGIL: &'static str = "w";
}
impl Sigil for FileNode {
    const SIGIL: &'static str = "f";
}
impl Sigil for BinNode {
    const SIGIL: &'static str = "b";
}
impl Sigil for ObjectNode {
    const SIGIL: &'static str = "o";
}
impl Sigil for Character {
    const SIGIL: &'static str = "c";
}
impl Sigil for SkinUnit {
    const SIGIL: &'static str = "u";
}

/// What the archive says of itself, and what identifies it.
#[derive(Debug, Clone, Serialize)]
pub struct ModMeta {
    /// The archive's file name, for display and as the fallback mod name.
    pub file_name: Spelled,
    /// The digest of the archive file.
    pub sha256: ContentId,
    /// The first twelve hex digits of the archive's SHA-256.
    pub mod_id: String,
    /// `Name` of `META/info.json`.
    pub name: Option<Spelled>,
    /// `Author` of `META/info.json`.
    pub author: Option<Spelled>,
    /// `Version` of `META/info.json`.
    pub version: Option<Spelled>,
    /// `Description` of `META/info.json`.
    pub description: Option<Spelled>,
}

/// Whose WAD a WAD of the mod is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum WadRole {
    /// `<champion>.wad.client`.
    Champion {
        /// The champion's directory name, lowercase.
        champion: String,
    },
    /// `<champion>.<xx_XX>.wad.client`.
    Locale {
        /// The champion's directory name, lowercase.
        champion: String,
        /// The locale, lowercase: `en_us`.
        locale: String,
    },
    /// Nothing of a champion's skin.
    Other,
}

/// How the archive holds a WAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WadLayout {
    /// Chunks as zip entries under `WAD/<name>/`.
    Loose,
    /// The WAD itself as one zip entry.
    Packed,
}

/// A WAD of the mod.
#[derive(Debug, Clone, Serialize)]
pub struct ModWad {
    /// The name as the archive spells it.
    pub name: WadName,
    /// Whose it is.
    pub role: WadRole,
    /// How the archive holds it.
    pub layout: WadLayout,
    /// Its files, by path hash.
    pub files: Vec<FileId>,
    /// The locale of a locale WAD taken as its champion's because the author packed the whole
    /// mod into it. The files the game localizes keep it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packed_locale: Option<String>,
}

/// The kind of a file by its first bytes, for the kinds a run reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    /// A bin.
    Bin,
    /// A PTCH.
    Ptch,
    /// A skinned mesh.
    Skn,
    /// A skeleton.
    Skl,
    /// An animation clip.
    Anm,
    /// A static mesh, binary.
    Scb,
    /// A static mesh, text.
    Sco,
    /// A texture in TEX's container.
    Tex,
    /// A texture in the DDS container.
    Dds,
    /// A Wwise bank.
    Bnk,
    /// A Wwise package.
    Wpk,
    /// Anything else.
    Other,
}

impl FileKind {
    /// Whether the client reads the file as a bin: a bin, or a PTCH at a bin's path.
    pub fn is_bin(self) -> bool {
        matches!(self, FileKind::Bin | FileKind::Ptch)
    }

    /// The kind's name in dumps and as a file extension.
    pub fn as_str(self) -> &'static str {
        match self {
            FileKind::Bin => "bin",
            FileKind::Ptch => "ptch",
            FileKind::Skn => "skn",
            FileKind::Skl => "skl",
            FileKind::Anm => "anm",
            FileKind::Scb => "scb",
            FileKind::Sco => "sco",
            FileKind::Tex => "tex",
            FileKind::Dds => "dds",
            FileKind::Bnk => "bnk",
            FileKind::Wpk => "wpk",
            FileKind::Other => "other",
        }
    }
}

/// An archive entry outside its WADs: a readme, a cover image, whatever else a mod was packed
/// with. Only its name, size and first bytes are kept.
#[derive(Debug, Clone, Serialize)]
pub struct Extra {
    /// The entry's name.
    pub name: Spelled,
    /// Its inflated size.
    pub size: u64,
    /// Its first bytes, enough to tell what it is.
    #[serde(skip)]
    pub head: Vec<u8>,
}

/// The rule by which a file fixup made a file from another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MadeBy {
    /// A file in a format the game dropped, made the format it reads at the same stem.
    LegacyFormat,
    /// A file at a path the game shipped once, made where it ships the asset.
    MovedAsset,
}

impl MadeBy {
    /// The rule's name in dumps.
    pub fn as_str(self) -> &'static str {
        match self {
            MadeBy::LegacyFormat => "legacy-format",
            MadeBy::MovedAsset => "moved-asset",
        }
    }
}

/// Where a file of the mod comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum FileSource {
    /// The archive ships it.
    Archive,
    /// A file fixup made it from another file of the archive.
    Made {
        /// The file it was made from.
        from: FileId,
        /// The rule that made it.
        by: MadeBy,
    },
    /// The game's own bin, taken as the mod's for a mod that ships none. Not the author's: it
    /// dates nothing.
    Game,
}

/// How the client comes to use a file of the mod. Mods carry stale bins, working copies and
/// leftovers, and only what the client would load is judged and carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileUse {
    /// Nothing loads it.
    #[default]
    Unused,
    /// At a path the game shipped at earlier builds and does not ship.
    FormerPath,
    /// At a path the game ships, so the game's own bins may load it.
    Override,
    /// At a path the game ships, holding the bytes the game shipped there at an earlier build.
    Stale,
    /// An object the client loads names it.
    Referenced,
    /// A bin the client loads.
    Loaded,
}

impl FileUse {
    /// The use's name in dumps.
    pub fn as_str(self) -> &'static str {
        match self {
            FileUse::Unused => "unused",
            FileUse::FormerPath => "former-path",
            FileUse::Override => "override",
            FileUse::Stale => "stale",
            FileUse::Referenced => "referenced",
            FileUse::Loaded => "loaded",
        }
    }
}

/// A file of the mod: a chunk of one of its WADs.
#[derive(Debug, Clone)]
pub struct FileNode {
    /// The WAD it is in.
    pub wad: WadId,
    /// The hash the client finds it by.
    pub hash: WadHash,
    /// Its path, where the archive or a table spells it.
    pub path: Option<ChunkPath>,
    /// Its size in bytes.
    pub size: u64,
    /// The digest of its bytes.
    pub content: ContentId,
    /// What it is, by its first bytes.
    pub kind: FileKind,
    /// Its bytes.
    pub bytes: Arc<[u8]>,
    /// How the client comes to use it.
    pub used: FileUse,
    /// Where it comes from.
    pub source: FileSource,
}

/// Where a bin's linked path leads when the client loads it: the mod's file wins over the
/// game's, and a path neither holds is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "to", content = "file", rename_all = "lowercase")]
pub enum LinkTo {
    /// A file of the mod.
    Mod(FileId),
    /// The game's file.
    Game,
    /// Nothing.
    Missing,
}

/// One entry of a bin's list of linked bins.
#[derive(Debug, Clone, Serialize)]
pub struct Link {
    /// The path as the bin spells it.
    pub path: Spelled,
    /// Where it leads.
    pub to: LinkTo,
}

/// A bin the client loads, parsed.
#[derive(Debug, Clone, Serialize)]
pub struct BinNode {
    /// The file it was read from.
    pub file: FileId,
    /// The bins it links, resolved as the client resolves them.
    pub links: Vec<Link>,
    /// Its objects, in file order.
    pub objects: Vec<ObjectId>,
    /// Why the bin could not be read, when it could not.
    pub error: Option<String>,
}

/// An object of a loaded bin.
#[derive(Debug, Clone)]
pub struct ObjectNode {
    /// The bin it is in.
    pub bin: BinId,
    /// Its entry hash.
    pub entry: BinHash,
    /// Its class.
    pub class: BinHash,
    /// The digest of its bytes from its entry hash to its end, as the archive ships it.
    pub raw: ContentId,
    /// The digest of its bytes after its entry hash: the same object under another name has the
    /// same body.
    pub body: ContentId,
    /// Its properties.
    pub value: BinObject,
}

/// What a character is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CharKind {
    /// A character the game keeps in a WAD of its own name.
    Champion,
    /// A character whose bins ship in a champion's WAD.
    Sub {
        /// The champion.
        champion: CharId,
    },
}

/// A directory under `data/characters/`.
#[derive(Debug, Clone, Serialize)]
pub struct Character {
    /// The directory's name, lowercase.
    pub name: String,
    /// The spelling entry names use, `Characters/<Spelled>/...`, else the lowercase name.
    pub spelled: String,
    /// A champion or a sub-character.
    pub kind: CharKind,
    /// The WAD its bins are in.
    pub wad: WadId,
    /// Its skin units, by slot.
    pub units: Vec<UnitId>,
    /// Its root bin, when the mod ships one.
    pub root_bin: Option<FileId>,
}

/// One skin bin the mod ships: `data/characters/<c>/skins/skin<S>.bin`.
#[derive(Debug, Clone, Serialize)]
pub struct SkinUnit {
    /// Whose skin it is.
    pub character: CharId,
    /// S: the slot the bin sits in.
    pub source_slot: Slot,
    /// The bin's file.
    pub file: FileId,
    /// The bin, when it reads.
    pub bin: Option<BinId>,
    /// The readable bins the client loads for this skin: the skin bin, then what it links,
    /// breadth first. Objects are looked up here and nowhere else.
    pub loads: Vec<BinId>,
    /// The skin entry: the one under the slot's own name, else the first of the skin class.
    pub entry: Option<ObjectId>,
    /// The slot the entry's name spells. Copies often keep another slot's name.
    pub entry_name_slot: Option<Slot>,
}

/// A champion and its sub-characters, which load together.
#[derive(Debug, Clone, Serialize)]
pub struct Group {
    /// The champion's WAD.
    pub wad: WadId,
    /// The champion.
    pub champion: CharId,
    /// The champion first, then its sub-characters by name.
    pub characters: Vec<CharId>,
}

/// The file a WAD holds at a path where another was to be added. A WAD holds one chunk per path
/// hash: that is its format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the WAD holds the file {0} at the path")]
pub struct Occupied(pub FileId);

/// One mod archive as a run holds it.
pub struct Mod {
    /// What the archive says of itself.
    pub meta: ModMeta,
    /// Its WADs, by lowercase name.
    pub wads: Arena<ModWad>,
    /// Its files, by WAD and path hash.
    pub files: Arena<FileNode>,
    /// The bins the client loads, in load order.
    pub bins: Arena<BinNode>,
    /// The objects of those bins, in file order.
    pub objects: Arena<ObjectNode>,
    /// The characters, a champion before its sub-characters.
    pub characters: Arena<Character>,
    /// One group per champion WAD.
    pub groups: Vec<Group>,
    /// The skin units, by slot within a character.
    pub units: Arena<SkinUnit>,
    /// The archive's entries outside its WADs and `META/info.json`, in archive order.
    pub extras: Vec<Extra>,
    /// WADs that restore a champion's model from before its rework. Their copies of the game's
    /// older bytes are the mod's own.
    pub restorations: BTreeSet<WadId>,
    /// Sub-characters whose every skin bin is the game's byte for byte: the mod adds nothing to
    /// them.
    pub equal_to_game: BTreeSet<CharId>,
    /// Files whose path a table or a bin of the mod spells, and the archive does not: a chunk
    /// of a packed WAD, a chunk named by its hash. What a mod's own spelling says of a file
    /// is not read off such a path.
    pub named_by_table: BTreeSet<FileId>,
    file_at: BTreeMap<(WadId, WadHash), FileId>,
    bin_of: BTreeMap<FileId, BinId>,
}

impl Mod {
    /// A mod with nothing in it yet.
    pub fn new(meta: ModMeta) -> Self {
        Self {
            meta,
            wads: Arena::default(),
            files: Arena::default(),
            bins: Arena::default(),
            objects: Arena::default(),
            characters: Arena::default(),
            groups: Vec::new(),
            units: Arena::default(),
            extras: Vec::new(),
            restorations: BTreeSet::new(),
            equal_to_game: BTreeSet::new(),
            named_by_table: BTreeSet::new(),
            file_at: BTreeMap::new(),
            bin_of: BTreeMap::new(),
        }
    }

    /// Adds a file to its WAD. Every file is added through this: a WAD holds one file per path
    /// hash, so where it holds one at the path none is added, and the caller is told which is
    /// there.
    pub fn push_file(&mut self, file: FileNode) -> Result<FileId, Occupied> {
        let (wad, hash) = (file.wad, file.hash);
        if let Some(&there) = self.file_at.get(&(wad, hash)) {
            return Err(Occupied(there));
        }
        let id = self.files.push(file);
        self.wads[wad].files.push(id);
        self.file_at.insert((wad, hash), id);
        Ok(id)
    }

    /// Adds a loaded bin. Bins are added through this, so that a lookup by file finds them.
    pub fn push_bin(&mut self, bin: BinNode) -> BinId {
        let file = bin.file;
        let id = self.bins.push(bin);
        self.bin_of.insert(file, id);
        id
    }

    /// The file at a path of a WAD.
    pub fn file_at(&self, wad: WadId, hash: WadHash) -> Option<FileId> {
        self.file_at.get(&(wad, hash)).copied()
    }

    /// The file at a path as the client finds it for a bin of `wad`: in that WAD first, else in
    /// the first WAD that holds one.
    pub fn find_file(&self, wad: WadId, hash: WadHash) -> Option<FileId> {
        self.file_at(wad, hash)
            .or_else(|| self.wads.ids().find_map(|w| self.file_at(w, hash)))
    }

    /// The skin bins a WAD ships for a character, by slot: the files at
    /// `data/characters/<c>/skins/skin<N>.bin` for N up to [`MAX_SLOT`].
    pub fn skin_bins(&self, wad: WadId, character: &str) -> Vec<(Slot, FileId)> {
        (0..=MAX_SLOT)
            .map(Slot)
            .filter_map(|slot| {
                let path = path_hash(&skin_paths::skin_bin(character, slot));
                self.file_at(wad, path).map(|file| (slot, file))
            })
            .collect()
    }

    /// A file's path as the archive spells it, or as the step that made the file gave it. None
    /// for a path only a table or a bin spells.
    pub fn spelled_path(&self, file: FileId) -> Option<&ChunkPath> {
        let path = self.files[file].path.as_ref();
        path.filter(|_| !self.named_by_table.contains(&file))
    }

    /// The file a file's bytes are judged by: the file it was made from, else itself.
    pub fn origin_of(&self, file: FileId) -> FileId {
        match self.files[file].source {
            FileSource::Made { from, .. } => from,
            FileSource::Archive | FileSource::Game => file,
        }
    }

    /// The loaded bin read from a file.
    pub fn bin_of(&self, file: FileId) -> Option<BinId> {
        self.bin_of.get(&file).copied()
    }

    /// The object of a bin under an entry hash.
    pub fn object_in(&self, bin: BinId, entry: BinHash) -> Option<ObjectId> {
        self.bins[bin]
            .objects
            .iter()
            .copied()
            .find(|&o| self.objects[o].entry == entry)
    }

    /// The champion of a character: itself, or the champion it is a sub-character of.
    pub fn champion_of(&self, character: CharId) -> CharId {
        match self.characters[character].kind {
            CharKind::Champion => character,
            CharKind::Sub { champion } => champion,
        }
    }

    /// A unit as a person reads it: `varus skin0`.
    pub fn unit_name(&self, unit: UnitId) -> String {
        let u = &self.units[unit];
        format!(
            "{} skin{}",
            self.characters[u.character].name, u.source_slot
        )
    }
}
