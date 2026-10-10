//! Stale copies (#MOD-ARTIFACT-0004): which files of a mod are the game's own older bytes at
//! the game's own path.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::{Fault, LoadOptions};
use crate::env::Env;
use crate::game::GameSource;
use crate::history::History;
use crate::model::entity::FileId;
use crate::model::hash::wad_hash_of_hex;
use crate::model::{ChunkPath, ContentId, FileKey, Mod, PathError, WadHash, WadName, path_hash};

/// A file kept where the run would drop it as a stale copy (#ADR-0098): by the name of its WAD
/// and its path in that WAD, or its path hash as sixteen hex digits for a file no table names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct KeptFile {
    /// The WAD's name, lowercase: `lux.wad.client`.
    pub wad: String,
    /// The file's path, lowercase, `/` between its parts.
    pub path: String,
}

/// Why a text names no file to keep.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeptFileError {
    /// The text has no `/` between a WAD's name and a path.
    #[error("{0:?} names no file: <wad>/<path>")]
    NoPath(String),
    /// The WAD's name or the path is not one a mod's file has.
    #[error(transparent)]
    Path(#[from] PathError),
}

impl KeptFile {
    /// The file at `path` in the WAD named `wad`.
    pub fn new(wad: &str, path: &str) -> Result<Self, KeptFileError> {
        let wad = WadName::parse(wad)?;
        let path = ChunkPath::parse(path)?;
        Ok(Self {
            wad: wad.lower().to_string(),
            path: path.as_str().to_string(),
        })
    }

    /// The file `<wad>/<path>` names: a WAD's name holds no separator, so the first `/` ends it.
    pub fn parse(text: &str) -> Result<Self, KeptFileError> {
        let (wad, path) = text
            .split_once(['/', '\\'])
            .ok_or_else(|| KeptFileError::NoPath(text.to_string()))?;
        Self::new(wad, path)
    }

    /// The hash the WAD holds the file under.
    pub fn hash(&self) -> WadHash {
        let stem = self
            .path
            .split_once('.')
            .map_or(&*self.path, |(stem, _)| stem);
        wad_hash_of_hex(stem).unwrap_or_else(|| path_hash(&self.path))
    }

    /// Whether it names this file of the mod.
    pub fn names(&self, m: &Mod, file: FileId) -> bool {
        let node = &m.files[file];
        node.hash == self.hash() && m.wads[node.wad].name.lower() == self.wad
    }
}

impl fmt::Display for KeptFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.wad, self.path)
    }
}

/// Judges whether a file is a stale copy.
pub struct Staleness<'a> {
    game: &'a dyn GameSource,
    history: &'a dyn History,
    keep: bool,
    kept: Vec<KeptFile>,
}

impl<'a> Staleness<'a> {
    /// A judge against an environment, under the options of `load`.
    pub fn new(env: &'a Env, options: &LoadOptions) -> Self {
        Self {
            game: env.game(),
            history: env.history(),
            keep: options.keep_stale,
            kept: options.kept_files.clone(),
        }
    }

    /// Whether a file is kept whatever it holds: every stale copy is, a WAD that restores a
    /// model from before its champion's rework holds none, and a file named to be kept is
    /// kept, by its own name or by that of the file a fixup made it from.
    fn kept(&self, m: &Mod, file: FileId) -> bool {
        let named = |file: FileId| self.kept.iter().any(|kept| kept.names(m, file));
        self.keep
            || m.restorations.contains(&m.files[file].wad)
            || named(file)
            || named(m.origin_of(file))
    }

    /// Whether a file is a stale copy: at a path the game ships, with other bytes than the
    /// game's, which the game shipped at that same path at an earlier build.
    ///
    /// Bytes the game held under another path are a file the author chose and are not stale
    /// (#MOD-ARTIFACT-0003). A file a fixup made is judged by the file it was made from. A WAD
    /// that restores a model from before its champion's rework holds no stale copy. Without a
    /// history nothing is known to be stale.
    pub fn of(&self, m: &Mod, file: FileId) -> Result<bool, Fault> {
        let node = &m.files[file];
        if self.kept(m, file) {
            return Ok(false);
        }
        if !self.game.contains(node.hash) || self.game.holds(node.hash, node.size, node.content)? {
            return Ok(false);
        }
        let origin = &m.files[m.origin_of(file)];
        let key = FileKey::from_sha256(&origin.content.0);
        Ok(self.history.held(origin.hash, key)?.is_some())
    }

    /// Whether bytes a later stage writes at the path of a file are a stale copy there, as
    /// [`Staleness::of`] judges a file. A texture is judged again in the container it ships
    /// in: a DDS made of the game's older TEX is that TEX once it is back in TEX's container.
    pub fn of_bytes(&self, m: &Mod, file: FileId, bytes: &[u8]) -> Result<bool, Fault> {
        let node = &m.files[file];
        if self.kept(m, file) {
            return Ok(false);
        }
        let content = ContentId::of(bytes);
        if !self.game.contains(node.hash)
            || self.game.holds(node.hash, bytes.len() as u64, content)?
        {
            return Ok(false);
        }
        let key = FileKey::from_sha256(&content.0);
        Ok(self.history.held(node.hash, key)?.is_some())
    }
}
