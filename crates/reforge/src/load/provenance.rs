//! Which of the mod's files the game ever shipped: for each file the client uses, whether its
//! bytes are the game's at its own path, at another path of the install, at earlier builds
//! only, or the mod's own.
//!
//! The install is asked first: the chunk at the file's path, then the chunks of its size in the
//! champion's WAD. The history is asked only for bytes the install does not hold there, and it
//! names the other paths the install may hold them at
//! (`docs/design/HISTORY.md`, "The questions").

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::ser::SerializeSeq as _;
use serde::{Serialize, Serializer};

use super::Fault;
use crate::diag::{Channel, Code, Diag, Message, Severity, Sink, Subject};
pub use crate::game::MAX_SAME_SIZE_READS;
use crate::game::{GameError, GameSource};
use crate::history::{History, Life};
use crate::model::entity::{FileId, FileNode, FileUse, Mod, WadId, WadRole};
use crate::model::hash::skin_paths;
use crate::model::{FileKey, SideTable, Slot, Spell, WadHash, WadName};

/// Every diagnostic code file provenance raises.
pub const CODES: &[Code] = &[OlderGameFiles::CODE, SameSizeCapped::CODE];

/// What the install holds at a file's own path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AtPath {
    /// The game has no chunk at the path.
    NotGamePath,
    /// The game's chunk at the path has the file's bytes.
    SameBytes,
    /// The game has a chunk at the path with other bytes.
    OtherBytes,
}

fn hash_hex<S: Serializer>(hash: &WadHash, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(&format_args!("{:016x}", hash.0))
}

fn hashes_hex<S: Serializer>(hashes: &[WadHash], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(hashes.iter().map(|hash| format!("{:016x}", hash.0)))
}

/// Whose bytes a file's are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "origin", rename_all = "kebab-case")]
pub enum BytesOrigin {
    /// The install holds the bytes.
    Install {
        /// The paths it holds them at, the best name first: the character's base skin folder,
        /// the character's folders, any named path, a path no table names.
        #[serde(serialize_with = "hashes_hex")]
        paths: Vec<WadHash>,
    },
    /// The game shipped the bytes at some build, and the install does not hold them.
    History {
        /// The builds the game held them.
        life: Life,
        /// The first path the game held them at.
        #[serde(serialize_with = "hash_hex")]
        path: WadHash,
        /// Whether the game holds them at the history's head, where the install does not.
        still_shipped: bool,
    },
    /// Neither: the mod's own, as far as the install and the history reach.
    Own,
}

impl BytesOrigin {
    /// The game path the bytes are best known by: the install's best, else the first the
    /// history saw.
    pub fn game_path(&self) -> Option<WadHash> {
        match self {
            BytesOrigin::Install { paths } => paths.first().copied(),
            BytesOrigin::History { path, .. } => Some(*path),
            BytesOrigin::Own => None,
        }
    }
}

/// A skin's folder: `assets/characters/<character>/skins/skin<slot>/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkinFolder {
    /// The character's directory name, lowercase.
    pub character: String,
    /// The slot the folder names.
    pub slot: Slot,
}

impl SkinFolder {
    /// The skin folder a path is under.
    pub fn of(path: &str) -> Option<Self> {
        skin_paths::skin_folder(path).map(|(character, slot)| Self { character, slot })
    }
}

/// What is known of one file's bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileFact {
    /// What the install holds at the file's own path.
    pub at_path: AtPath,
    /// Whose bytes they are.
    #[serde(flatten)]
    pub origin: BytesOrigin,
    /// The skin folder the file's path names, else the one the game path of its bytes names.
    /// Another champion's folder is ordinary material, as in Riot's own skins, and says nothing
    /// of the mod's skin.
    pub skin_folder: Option<SkinFolder>,
    /// Whether the file is effects material: its path, or the game path of its bytes, is under
    /// a `particles` folder. Another skin's effects are borrowed on purpose
    /// (#MOD-ARTIFACT-0003).
    pub vfx: bool,
}

impl FileFact {
    /// Whether the bytes were ever a game file.
    pub fn is_game(&self) -> bool {
        self.origin != BytesOrigin::Own
    }

    /// The install paths holding the bytes, the best name first. Empty when the install does
    /// not hold them.
    pub fn game_paths(&self) -> &[WadHash] {
        match &self.origin {
            BytesOrigin::Install { paths } => paths,
            _ => &[],
        }
    }

    /// The builds the game held the bytes, for bytes the install does not hold.
    pub fn shipped(&self) -> Option<Life> {
        match self.origin {
            BytesOrigin::History { life, .. } => Some(life),
            _ => None,
        }
    }
}

/// Whether a path is under a `particles` folder.
pub fn is_vfx_path(path: &str) -> bool {
    path.to_ascii_lowercase().contains("/particles/")
}

/// Files of a WAD whose bytes the game shipped at earlier builds and the install does not hold.
/// A stale copy is one of them (#MOD-ARTIFACT-0004).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OlderGameFiles {
    /// The WAD.
    pub wad: WadId,
    /// The files, in file order.
    pub files: Vec<FileId>,
    /// Their sizes together.
    pub bytes: u64,
    /// How many of them the game holds at the history's head, where the install does not.
    pub still_shipped: usize,
}

impl Diag for OlderGameFiles {
    const CODE: Code = Code::new("file.older-game", Channel::Input, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message) {
        f.count(self.files.len(), "file");
        let _ = write!(
            f,
            " ({:.1} MiB) of bytes the game shipped at earlier builds, which the install does \
             not hold",
            self.bytes as f64 / f64::from(1u32 << 20)
        );
        if self.still_shipped > 0 {
            let _ = write!(
                f,
                ", {} of them still shipped under another path",
                self.still_shipped
            );
        }
    }
}

/// Files that meet more game chunks of their size than are read: the chunks past the cap are
/// not looked at for their bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SameSizeCapped {
    /// The files, in file order.
    pub files: Vec<FileId>,
    /// The cap on chunks read for one file.
    pub cap: usize,
}

impl Diag for SameSizeCapped {
    const CODE: Code = Code::new("file.same-size-capped", Channel::Input, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message) {
        f.count(self.files.len(), "file");
        let _ = write!(
            f,
            " meet more than {} game chunks of their size; the rest are not read for their bytes",
            self.cap
        );
    }
}

/// What a mod's files are judged against.
#[derive(Clone, Copy)]
pub struct Against<'a> {
    /// The target game.
    pub game: &'a dyn GameSource,
    /// The build history.
    pub history: &'a dyn History,
    /// The names that spell a game path: a copy is ranked, and its skin folder told, by them.
    pub names: &'a dyn Spell,
}

/// For each file the client uses, whose bytes it holds.
#[derive(Default)]
pub struct FileProvenance {
    facts: SideTable<FileNode, FileFact>,
    older: Vec<OlderGameFiles>,
    capped: Vec<FileId>,
}

impl FileProvenance {
    /// The provenance of a mod's files. A file nothing loads is not judged. A game chunk that
    /// does not read, and a history that does not answer, are faults.
    pub fn find(m: &Mod, against: &Against<'_>) -> Result<Self, Fault> {
        let mut finder = Finder {
            against: *against,
            capped: Vec::new(),
        };
        let mut out = Self::default();
        let mut older: BTreeMap<WadId, OlderGameFiles> = BTreeMap::new();
        for (id, file) in m.files.iter() {
            if file.used == FileUse::Unused {
                continue;
            }
            let fact = finder.fact(m, id)?;
            if let BytesOrigin::History { still_shipped, .. } = fact.origin {
                let of_wad = older.entry(file.wad).or_insert_with(|| OlderGameFiles {
                    wad: file.wad,
                    files: Vec::new(),
                    bytes: 0,
                    still_shipped: 0,
                });
                of_wad.files.push(id);
                of_wad.bytes = of_wad.bytes.saturating_add(file.size);
                of_wad.still_shipped += usize::from(still_shipped);
            }
            out.facts.insert(id, fact);
        }
        out.older = older.into_values().collect();
        out.capped = finder.capped;
        Ok(out)
    }

    /// What is known of a file's bytes. None for a file nothing loads.
    pub fn fact(&self, file: FileId) -> Option<&FileFact> {
        self.facts.get(file)
    }

    /// Every judged file with its fact, in file order.
    pub fn iter(&self) -> impl Iterator<Item = (FileId, &FileFact)> {
        self.facts.iter()
    }

    /// Whether a file's bytes were ever a game file.
    pub fn is_game(&self, file: FileId) -> bool {
        self.fact(file).is_some_and(FileFact::is_game)
    }

    /// The install paths holding a file's bytes, the best name first.
    pub fn game_paths(&self, file: FileId) -> &[WadHash] {
        self.fact(file).map_or(&[], FileFact::game_paths)
    }

    /// What the install holds at a file's own path.
    pub fn at_path(&self, file: FileId) -> Option<AtPath> {
        self.fact(file).map(|fact| fact.at_path)
    }

    /// The builds the game held a file's bytes, for bytes the install does not hold.
    pub fn shipped(&self, file: FileId) -> Option<Life> {
        self.fact(file).and_then(FileFact::shipped)
    }

    /// The files of older game bytes, by WAD.
    pub fn older_game(&self) -> &[OlderGameFiles] {
        &self.older
    }

    /// Raises `file.older-game` for each WAD holding such files, then the files whose chunks of
    /// their size passed the cap.
    pub fn report(&self, sink: &mut Sink<'_>) {
        for of_wad in &self.older {
            sink.raise(of_wad.clone());
        }
        if !self.capped.is_empty() {
            sink.raise(SameSizeCapped {
                files: self.capped.clone(),
                cap: MAX_SAME_SIZE_READS,
            });
        }
    }
}

/// A file's fact with its id, as a report holds it.
#[derive(Serialize)]
struct Row<'a> {
    file: FileId,
    #[serde(flatten)]
    fact: &'a FileFact,
}

impl Serialize for FileProvenance {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut rows = s.serialize_seq(Some(self.facts.len()))?;
        for (file, fact) in self.iter() {
            rows.serialize_element(&Row { file, fact })?;
        }
        rows.end()
    }
}

/// The search for one mod's files.
struct Finder<'a> {
    against: Against<'a>,
    /// The files whose chunks of their size passed the cap.
    capped: Vec<FileId>,
}

impl Finder<'_> {
    /// Whether the game's file at a path has a file's bytes.
    fn holds(&self, path: WadHash, file: &FileNode) -> Result<bool, GameError> {
        self.against.game.holds(path, file.size, file.content)
    }

    /// Ranks equal copies so that the one named is the one that outlives the others: the
    /// character's base skin folder, then the character's folders, then any named path, then a
    /// path no table names.
    fn rank(&self, character: &str, path: WadHash) -> u8 {
        match self.against.names.path(path) {
            Some(p) if p.starts_with(&format!("assets/characters/{character}/skins/base/")) => 0,
            Some(p) if p.contains(&format!("/characters/{character}/")) => 1,
            Some(_) => 2,
            None => 3,
        }
    }

    fn fact(&mut self, m: &Mod, id: FileId) -> Result<FileFact, Fault> {
        let file = &m.files[id];
        let wad = &m.wads[file.wad];
        let Against {
            game,
            history,
            names,
        } = self.against;
        // A locale WAD's files are the champion's files: its copies are in the champion's WAD.
        let (home, character) = match &wad.role {
            WadRole::Champion { champion } | WadRole::Locale { champion, .. } => (
                WadName::parse(&format!("{champion}.wad.client")).ok(),
                champion.as_str(),
            ),
            WadRole::Other => (Some(wad.name.clone()), ""),
        };

        let at_path = match game.contains(file.hash) {
            false => AtPath::NotGamePath,
            true if self.holds(file.hash, file)? => AtPath::SameBytes,
            true => AtPath::OtherBytes,
        };
        let mut paths = Vec::new();
        if at_path == AtPath::SameBytes {
            paths.push(file.hash);
        }
        if let Some(home) = home {
            let copies = game.copies(&home, file.size, file.content, file.hash)?;
            if copies.capped {
                self.capped.push(id);
            }
            paths.extend(copies.paths);
        }
        paths.sort_by_cached_key(|path| (self.rank(character, *path), path.0));

        // #ADR-0037: the head of the file's digest.
        let key = FileKey::from_sha256(&file.content.0);
        let hit = if paths.is_empty() {
            history.file(key)?
        } else {
            None
        };
        if hit.is_some() {
            // The install may hold the bytes at a path the head has, outside the champion's
            // WAD.
            for held in history.paths_of(key)? {
                let at_head = history.reaches_head(held.life);
                if at_head && held.path != file.hash && self.holds(held.path, file)? {
                    paths.push(held.path);
                }
            }
        }
        let origin = match hit {
            _ if !paths.is_empty() => BytesOrigin::Install { paths },
            Some(hit) => BytesOrigin::History {
                life: hit.life,
                path: hit.path,
                still_shipped: history.reaches_head(hit.life),
            },
            None => BytesOrigin::Own,
        };

        // The mod's own spelling: a path a table gives a chunk of a packed WAD says where the
        // game keeps that hash, which the game path of the bytes says better.
        let own_path = m.spelled_path(id).map(|path| path.as_str());
        let game_path = origin.game_path().and_then(|path| names.path(path));
        let game_path = game_path.as_deref();
        Ok(FileFact {
            at_path,
            skin_folder: own_path
                .and_then(SkinFolder::of)
                .or_else(|| game_path.and_then(SkinFolder::of)),
            vfx: own_path.is_some_and(is_vfx_path) || game_path.is_some_and(is_vfx_path),
            origin,
        })
    }
}
