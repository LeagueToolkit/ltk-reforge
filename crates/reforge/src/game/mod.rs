//! The target side: the install's chunks, the names that spell hashes, and the schema of the
//! target build. A run reads them and changes none of them.
//!
//! `docs/design/ARCHITECTURE.md`, "The environment".

mod book;
mod catalog;
mod install;
mod namehash;
mod names;
mod schema;
mod source;
mod speller;
mod view;

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use camino::Utf8PathBuf;

pub use self::book::NameBook;
pub use self::catalog::{CatalogSkin, GameObject, MAX_CHAIN, SkinCatalog, SkinClass, graph_clips};
pub use self::install::{
    Install, InstallError, NoBuild, SizeIndex, UnreadableWad, VersionUnknown, WadsUnreadable,
};
pub use self::namehash::{NameHashError, NameHashIndex, NameHashSlot};
pub use self::names::{
    AbsentTable, MimirAbsent, NameSource, NameTable, Names, NamesBuilder, NamesError,
    mimir_cache_dir,
};
pub use self::schema::{
    FieldType, SCHEMA_FILE, Schema, SchemaAbsent, SchemaError, SchemaInstallNewer,
    SchemaUnreadable, meta_cache_dir,
};
pub use self::source::{Copies, GameBin, GameSkin, GameSource, MAX_SAME_SIZE_READS};
pub use self::speller::Speller;
pub use self::view::GameView;
use crate::diag::{Code, Diag};
use crate::model::{Version, WadHash, WadName};

/// The bin of shader definitions, which the client loads whatever it plays.
pub const SHADERS_BIN: &str = "data/shaders/shaders.bin";

/// Every diagnostic code this module raises.
pub const CODES: &[Code] = &[
    WadsUnreadable::CODE,
    VersionUnknown::CODE,
    MimirAbsent::CODE,
    SchemaAbsent::CODE,
    SchemaUnreadable::CODE,
    SchemaInstallNewer::CODE,
];

/// One version of the game, written `16.19.8207193`. The census history marks a second manifest
/// of one version `14.8.5756478__2`, which is a build of its own with the same number.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Build {
    text: Arc<str>,
    number: u64,
}

impl Build {
    /// The build a version text names, without its `+branch` suffix. `None` when a part between
    /// two dots is not a number, or the mark after `__` is not one.
    pub fn parse(version: &str) -> Option<Self> {
        let text = version.split_once('+').map_or(version, |(build, _)| build);
        let numeric = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
        let (plain, mark) = match text.split_once("__") {
            Some((plain, mark)) => (plain, Some(mark)),
            None => (text, None),
        };
        if !plain.split('.').all(numeric) || mark.is_some_and(|mark| !numeric(mark)) {
            return None;
        }
        let number = plain.rsplit('.').next()?.parse().ok()?;
        Some(Self {
            text: text.into(),
            number,
        })
    }

    /// The build as written, `16.19.8207193`.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The parts between the dots as numbers, the last without its `__` mark.
    pub fn parts(&self) -> impl Iterator<Item = u64> {
        let plain = self
            .text
            .split_once("__")
            .map_or(&*self.text, |(plain, _)| plain);
        // Every part is digits, by `parse`. One too long for 64 bits counts as the largest.
        plain
            .split('.')
            .map(|part| part.parse().unwrap_or(u64::MAX))
    }

    /// The last part, `8207193`: the number the schema orders builds by. The history orders
    /// them by position, and a late hotfix has a higher number than the patch after it.
    pub fn number(&self) -> u64 {
        self.number
    }

    /// The first two parts, `16.19`, which order builds across hotfixes. None for a build
    /// written with fewer than three parts, or parts too large for a version.
    pub fn version(&self) -> Option<Version> {
        let mut parts = self.parts();
        let (major, minor) = (parts.next()?, parts.next()?);
        parts.next()?;
        Some(Version::new(major.try_into().ok()?, minor.try_into().ok()?))
    }
}

impl fmt::Display for Build {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl serde::Serialize for Build {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.text)
    }
}

/// Why a chunk of a game does not read.
#[derive(Debug, thiserror::Error)]
pub enum GameError {
    /// The WAD's file does not open.
    #[error(transparent)]
    Open(#[from] std::io::Error),
    /// The WAD's table of contents does not read.
    #[error("mounting {wad}")]
    Mount {
        /// The WAD's file.
        wad: Utf8PathBuf,
        /// What `ltk_wad` refused.
        #[source]
        source: ltk_wad::WadError,
    },
    /// The WAD's table of contents lists the chunk and its bytes do not read or decompress.
    #[error("reading chunk {path:016x} of {wad}")]
    Chunk {
        /// The WAD's file.
        wad: Utf8PathBuf,
        /// The chunk's path hash.
        path: WadHash,
        /// What `ltk_wad` refused.
        #[source]
        source: ltk_wad::WadError,
    },
    /// The WAD held the chunk when the game was opened and does not hold it at the read: its
    /// file changed during the run.
    #[error("{wad} changed during the run: chunk {path:016x} is gone")]
    Changed {
        /// The WAD's file.
        wad: Utf8PathBuf,
        /// The chunk's path hash.
        path: WadHash,
    },
}

/// A chunk store: the game a run is judged against.
///
/// A path resolves to the first WAD holding it. Every WAD holding it is known too: whether a
/// file is there when a champion plays depends on which WADs the client mounts, and Riot ships
/// one asset in several champions' WADs.
pub trait Game: Send + Sync {
    /// The game's build, when it says.
    fn build(&self) -> Option<&Build>;

    /// Where the game was read from, for the report.
    fn describe(&self) -> String;

    /// The uncompressed size of the chunk at a path.
    fn size(&self, path: WadHash) -> Option<u64>;

    /// The chunk's bytes, decompressed. `None` when the game has no chunk at the path.
    fn read(&self, path: WadHash) -> Result<Option<Vec<u8>>, GameError>;

    /// Every WAD holding the path, the one it is read from first. Empty when none holds it.
    fn wads(&self, path: WadHash) -> Vec<WadName>;

    /// Whether the game has a chunk at a path.
    fn contains(&self, path: WadHash) -> bool {
        self.size(path).is_some()
    }
}

/// An error and what caused it, on one line: a diagnostic's reason.
fn error_chain(error: &dyn Error) -> String {
    let mut out = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        out.push_str(": ");
        out.push_str(&cause.to_string());
        source = cause.source();
    }
    out
}
