//! Stale copies (#MOD-ARTIFACT-0004): which files of a mod are the game's own older bytes at
//! the game's own path.

use super::{Fault, LoadOptions};
use crate::env::Env;
use crate::game::{Game, GameView};
use crate::history::History;
use crate::model::entity::FileId;
use crate::model::{ContentId, FileKey, Mod};

/// Judges whether a file is a stale copy.
pub struct Staleness<'a> {
    game: &'a GameView,
    history: &'a dyn History,
    keep: bool,
}

impl<'a> Staleness<'a> {
    /// A judge against an environment, under the options of `load`.
    pub fn new(env: &'a Env, options: &LoadOptions) -> Self {
        Self {
            game: env.game(),
            history: env.history(),
            keep: options.keep_stale,
        }
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
        if self.keep || m.restorations.contains(&node.wad) {
            return Ok(false);
        }
        if self.game.size(node.hash).is_none()
            || self.game.holds(node.hash, node.size, node.content)?
        {
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
        if self.keep || m.restorations.contains(&node.wad) {
            return Ok(false);
        }
        let content = ContentId::of(bytes);
        if self.game.size(node.hash).is_none()
            || self.game.holds(node.hash, bytes.len() as u64, content)?
        {
            return Ok(false);
        }
        let key = FileKey::from_sha256(&content.0);
        Ok(self.history.held(node.hash, key)?.is_some())
    }
}
