//! When the mod was made: the oldest build it can have been made at. A mod cannot hold game
//! bytes before the game shipped them, so the newest first build among its game files and
//! objects bounds its build from below. The last build of a life bounds nothing: mods keep
//! copies of old game files for years (#MOD-ARTIFACT-0004).
//!
//! The evidence is gathered first, each piece of the mod the history holds with the build the
//! game first shipped it at. The verdict is read from it.

use serde::{Deserialize, Serialize};

use crate::diag::Code;
use crate::game::Build;
use crate::history::{History, HistoryError, Position};
use crate::model::entity::{FileId, FileSource, Mod, ObjectId};
use crate::model::{FileKey, ObjectKey};

/// Every diagnostic code dating raises: none. The bound is a fact the fixups read.
pub const CODES: &[Code] = &[];

/// The size under which a file does not date the mod. A tiny texture can equal a game one the
/// author never copied.
pub const MIN_FILE: u64 = 4096;

/// The number of properties under which an object does not date the mod. A near-empty object
/// can equal a game one the author never copied.
pub const MIN_PROPERTIES: usize = 4;

/// A piece of the mod the game shipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "piece", content = "id", rename_all = "kebab-case")]
pub enum Piece {
    /// A file, by its bytes.
    File(FileId),
    /// An object of a loaded bin, by its bytes from its entry hash to its end.
    Object(ObjectId),
}

/// A piece of the mod with the build the game first shipped it at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Dated {
    /// The piece.
    pub piece: Piece,
    /// The first build the game held it.
    pub first: Position,
}

/// What dates the mod: every piece the history holds, the files in file order, then the objects
/// in object order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DatingEvidence {
    /// The pieces counted.
    pub pieces: Vec<Dated>,
}

impl DatingEvidence {
    /// The pieces of a mod the history dates. A history that holds nothing dates none.
    pub fn gather(m: &Mod, history: &dyn History) -> Result<Self, HistoryError> {
        let mut pieces = Vec::new();
        // A bin taken from the game for a binless mod is the install's, not the author's.
        let authors = |file: FileId| m.files[file].source != FileSource::Game;
        for (id, file) in m.files.iter() {
            if file.size < MIN_FILE || !authors(id) {
                continue;
            }
            // #ADR-0037: a file and an object are asked by the head of their digest.
            if let Some(hit) = history.file(FileKey::from_sha256(&file.content.0))? {
                pieces.push(Dated {
                    piece: Piece::File(id),
                    first: hit.life.first,
                });
            }
        }
        for (id, object) in m.objects.iter() {
            if object.value.properties.len() < MIN_PROPERTIES || !authors(m.bins[object.bin].file) {
                continue;
            }
            if let Some(hit) = history.object(ObjectKey::from_sha256(&object.raw.0))? {
                pieces.push(Dated {
                    piece: Piece::Object(id),
                    first: hit.life.first,
                });
            }
        }
        Ok(Self { pieces })
    }

    /// The newest first build among the pieces.
    pub fn newest(&self) -> Option<Position> {
        self.pieces.iter().map(|dated| dated.first).max()
    }
}

/// The build a mod was made at or after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bound {
    /// The build's position in the history.
    pub at: Position,
    /// The build.
    pub build: Build,
}

/// The oldest build the mod can have been made at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Dating {
    /// The bound. None when the history dates no piece of the mod, or lists no build at the
    /// newest first position.
    pub not_before: Option<Bound>,
    /// The first piece that first shipped at the bound.
    pub evidence: Option<Piece>,
    /// How many pieces the history dates.
    pub dated: usize,
    /// How many of them first shipped at the bound.
    pub at_bound: usize,
}

impl Dating {
    /// The dating of a mod: its evidence gathered and judged.
    pub fn find(m: &Mod, history: &dyn History) -> Result<Self, HistoryError> {
        Ok(Self::judge(&DatingEvidence::gather(m, history)?, history))
    }

    /// The verdict on gathered evidence: the newest first build among the pieces.
    pub fn judge(evidence: &DatingEvidence, history: &dyn History) -> Self {
        let dated = evidence.pieces.len();
        let Some(newest) = evidence.newest() else {
            return Self {
                dated,
                ..Self::default()
            };
        };
        let at_bound = || evidence.pieces.iter().filter(|p| p.first == newest);
        Self {
            not_before: history.build(newest).map(|row| Bound {
                at: newest,
                build: row.build.clone(),
            }),
            evidence: at_bound().next().map(|dated| dated.piece),
            dated,
            at_bound: at_bound().count(),
        }
    }

    /// The number of the build the mod was made at or after: what the schema orders builds by.
    pub fn build(&self) -> Option<u64> {
        self.not_before.as_ref().map(|bound| bound.build.number())
    }

    /// The position in the history of the build the mod was made at or after.
    pub fn position(&self) -> Option<Position> {
        self.not_before.as_ref().map(|bound| bound.at)
    }
}
