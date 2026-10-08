//! The build history: the questions the stages ask, over the index.
//!
//! The install is one build. A mod is made against another, often a year older, and carries
//! game files that build had and this one does not. Against the install's bytes those files look
//! like the author's own. A run asks the history instead: has the game ever shipped these
//! bytes, this object, this clip, and when.
//!
//! The index is derived from the census history, exact, stamped with the commit it came from,
//! and disposable: `reforge-census` writes it (#ADR-0005) from the tables `tables.rs` defines.
//! Keys are compared whole, at 128 bits for files and objects and 64 for clips and norm keys:
//! no filter stands before a table that could call an own file the game's.

mod findings;
pub mod objects;
mod sqlite;
mod stamp;
pub mod tables;

use std::cmp::Ordering;
use std::error::Error;

use camino::{Utf8Path, Utf8PathBuf};

pub use self::findings::{HistoryAbsent, IndexMissing, IndexStale, InstallNewer};
pub use self::objects::ObjectCodec;
pub use self::sqlite::{SqliteHistory, StoredObject};
pub use self::stamp::{
    BUILDS_FILE, BuildDetails, BuildRow, DB_FILE, FORMAT, STAMP_FILE, Stamp, version_order,
};
pub use self::tables::{
    BinEntry, BlobId, EventsBank, Families, Family, FileKind, Joint, Life, Mesh, Position,
    Skeleton, Submesh, Texture,
};
use crate::diag::{Code, Diag, Sink};
use crate::game::{Build, FieldType};
use crate::model::{BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash};

/// Every diagnostic code this module raises.
pub const CODES: &[Code] = &[
    HistoryAbsent::CODE,
    IndexMissing::CODE,
    IndexStale::CODE,
    InstallNewer::CODE,
];

/// Why an index does not open or does not answer.
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    /// A file of the index does not read or write.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// `stamp.json` is not the JSON of a stamp.
    #[error("the stamp {path} does not read")]
    Stamp {
        /// The stamp's file.
        path: Utf8PathBuf,
        /// What serde refused.
        #[source]
        source: serde_json::Error,
    },
    /// The index is of another format than this crate reads.
    #[error("the index at {dir} has format {found} and reforge reads format {FORMAT}; rebuild it")]
    Format {
        /// The index's directory.
        dir: Utf8PathBuf,
        /// The format its stamp states.
        found: u32,
    },
    /// A line of `builds.tsv` is no build at its position.
    #[error("line {line} of {path} is no build at its position")]
    Builds {
        /// The file.
        path: Utf8PathBuf,
        /// The line, from 1.
        line: usize,
    },
    /// The stamp counts other builds than `builds.tsv` lists.
    #[error("the index at {dir} is stamped for {stamped} builds and lists {listed}")]
    BuildCount {
        /// The index's directory.
        dir: Utf8PathBuf,
        /// The count the stamp states.
        stamped: usize,
        /// The lines of `builds.tsv`.
        listed: usize,
    },
    /// The SQLite file does not open.
    #[error("opening {path}")]
    Open {
        /// The SQLite file.
        path: Utf8PathBuf,
        /// What SQLite refused.
        #[source]
        source: rusqlite::Error,
    },
    /// A question to the SQLite file failed.
    #[error("the index does not answer")]
    Query(#[from] rusqlite::Error),
    /// A name that is no family.
    #[error("unknown family {0:?}: files, objects, clips, rigs, textures")]
    UnknownFamily(String),
    /// An events bank of the index lists an object the index does not hold.
    #[error("the events bank at {path:?} lists object {record:016x}, which the index lacks")]
    BankObject {
        /// The bank's path.
        path: WadHash,
        /// The record of the object.
        record: u64,
    },
    /// A row of the object bytes names a keyframe or a dictionary the index lacks.
    #[error("row {object} of the object bytes names {what}, which the index lacks")]
    ObjectRow {
        /// The row of `object_bytes`.
        object: i64,
        /// What it names: its keyframe or its dictionary.
        what: String,
    },
    /// A frame of the object bytes does not decode.
    #[error("row {object} of the object bytes does not decode")]
    Frame {
        /// The row of `object_bytes`.
        object: i64,
        /// Why.
        #[source]
        source: objects::FrameError,
    },
}

/// A content the game shipped: its life, and the first file seen with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileHit {
    /// The builds some WAD held the content.
    pub life: Life,
    /// The first path the content was seen under.
    pub path: WadHash,
    /// What the bytes are by their magic.
    pub kind: FileKind,
}

/// A path the game shipped a file at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathHit {
    /// The builds some WAD held a file at the path.
    pub life: Life,
    /// The kind of the first file seen there.
    pub kind: FileKind,
}

/// A path the game held a content at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldAt {
    /// The path.
    pub path: WadHash,
    /// The builds the game held the content there.
    pub life: Life,
}

/// A content a WAD of the game held at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// The WAD's path in the manifest.
    pub wad: String,
    /// The content.
    pub key: FileKey,
    /// The checksum the WAD's table held for the file, 0 for a WAD that holds none.
    pub checksum: u64,
    /// The builds the WAD held the content there without a break.
    pub life: Life,
}

/// A bin a WAD of the game held at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinHit {
    /// The WAD's path in the manifest.
    pub wad: String,
    /// The bin's content.
    pub key: FileKey,
    /// The builds the WAD held the bin there without a break.
    pub life: Life,
    /// The path of each bin it links, in the order the census history lists them.
    pub links: Vec<WadHash>,
    /// Its objects, by entry hash.
    pub entries: Vec<BinEntry>,
}

/// The objects a game object reaches by its links at one build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reached {
    /// Each object reached, by its bin's path and its entry hash, the object asked about among
    /// them, in that order.
    pub objects: Vec<(WadHash, BinHash)>,
    /// Whether [`MAX_REACHED`] stopped the search, so that the object reaches more than these.
    pub capped: bool,
}

/// How many objects [`History::reached`] answers at most.
// #ADR-0070. The 21,024 skin entries of 16.19.8230722 reach 487 objects at most, and the 3,976
// of 9.1.2577563 reach 260 (2026-10-08).
pub const MAX_REACHED: usize = 4096;

/// The blob of a bin entry's text in one WAD's bin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextBlob {
    /// The WAD's path in the manifest.
    pub wad: String,
    /// The text's blob in the census history.
    pub blob: BlobId,
    /// The builds the WAD's bin held the text without a break.
    pub life: Life,
}

/// A game object's bytes at one build, in one WAD's bin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectBytes {
    /// The WAD's path in the manifest.
    pub wad: String,
    /// The object's class, which a bin stores apart from the object.
    pub class: BinHash,
    /// The object as `ltk_meta` writes it from its entry hash on, with no size before it: the
    /// bytes its object key digests.
    pub bytes: Vec<u8>,
    /// The builds the WAD's bin held the object's text without a break.
    pub life: Life,
}

/// Orders two WADs by their paths, directory by directory, as the census history's tree lists
/// them: the first WAD in this order is the one `GitRepo::entry_text` reads.
pub fn wad_order(a: &str, b: &str) -> Ordering {
    a.split('/').cmp(b.split('/'))
}

/// An object the game shipped: its life, and the first place it was seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectHit {
    /// The builds some bin held the object.
    pub life: Life,
    /// The object's entry hash.
    pub entry: BinHash,
    /// The object's class.
    pub class: BinHash,
    /// The path of the first bin the object was seen in.
    pub bin: WadHash,
    /// The object's norm key.
    pub norm: NormKey,
}

/// A game object that named a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NamedBy {
    /// The path of the object's bin.
    pub bin: WadHash,
    /// The object's entry hash.
    pub entry: BinHash,
    /// The builds the object named the path.
    pub life: Life,
}

/// A game bank unit that loaded a bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitHit {
    /// The path of the bin of the unit's object.
    pub bin: WadHash,
    /// The entry hash of the unit's object.
    pub entry: BinHash,
    /// The unit's name.
    pub unit: String,
    /// Whether the unit is voice.
    pub voice: bool,
    /// The builds the unit loaded the bank.
    pub life: Life,
}

/// A wem an event of a bank played.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WemPlayed {
    /// The wem's id.
    pub wem: u32,
    /// The builds the event played it.
    pub life: Life,
}

/// An event of a bank that played a wem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayedBy {
    /// The path of the events bank.
    pub bank: WadHash,
    /// The event's Wwise id.
    pub event: u32,
    /// The builds the event played the wem.
    pub life: Life,
}

/// A skeleton the game held at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonHit {
    /// The builds the game held it there.
    pub life: Life,
    /// The skeleton.
    pub skeleton: Skeleton,
}

/// A mesh the game held at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshHit {
    /// The builds the game held it there.
    pub life: Life,
    /// The mesh.
    pub mesh: Mesh,
}

/// An animation clip the game held at a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipJoints {
    /// The builds the game held the clip there.
    pub life: Life,
    /// The key of the clip's bytes.
    pub key: FileKey,
    /// The hash of each joint the clip animates, in stored order.
    pub joints: Vec<u32>,
}

/// An events bank the game held at a path, by the file that held it (#ADR-0081).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventsBankHit {
    /// The key of the file.
    pub key: FileKey,
    /// The builds a WAD of the game held the file there.
    pub life: Life,
    /// Each WAD that held the file there, with the builds it did, in `wad_order` and then
    /// oldest first: a reader of one locale's WAD takes the bank that WAD held.
    pub placed: Vec<Placed>,
    /// The bank's header and objects.
    pub bank: EventsBank,
}

/// The build history, as the stages ask it. One method per question.
///
/// A question at one build (`placed`, `bin`, `text_blob`, `links`, `reached`, `field_types`)
/// reads a table of spans, whose lives have no gap. Every other life is a hull: a key that left
/// the game and came back is present, as far as the history says, at every build between.
pub trait History: Send + Sync {
    /// What the index was derived from. None without a history.
    fn stamp(&self) -> Option<&Stamp>;

    /// The builds, by position.
    fn builds(&self) -> &[BuildRow];

    /// Whether the game ever shipped these bytes, and when.
    fn file(&self, key: FileKey) -> Result<Option<FileHit>, HistoryError>;

    /// The builds some WAD of the game held a file at this path.
    fn path(&self, path: WadHash) -> Result<Option<PathHit>, HistoryError>;

    /// The builds some WAD of the game held these bytes at this path, where `file` names only
    /// the first path they were seen under.
    fn held(&self, path: WadHash, key: FileKey) -> Result<Option<Life>, HistoryError>;

    /// Every path the game held these bytes at, in path order.
    fn paths_of(&self, key: FileKey) -> Result<Vec<HeldAt>, HistoryError>;

    /// The contents the game's WADs held at this path at the build at `at`, in `wad_order`.
    fn placed(&self, path: WadHash, at: Position) -> Result<Vec<Placed>, HistoryError>;

    /// The bin the game held at this path at the build at `at`, with its links and its objects:
    /// the first of `placed` that is a bin.
    fn bin(&self, path: WadHash, at: Position) -> Result<Option<BinHit>, HistoryError>;

    /// Whether the game ever shipped this object, and when. Of an object the game held under two
    /// classes, the one whose life reaches furthest.
    fn object(&self, key: ObjectKey) -> Result<Option<ObjectHit>, HistoryError>;

    /// Every life of this object, one per class the game held it under, oldest first. Dating a
    /// mod by its objects is a batch of these: the builds from the latest first to the earliest
    /// last.
    fn object_lives(&self, key: ObjectKey) -> Result<Vec<ObjectHit>, HistoryError>;

    /// Every game object of this norm key: the same object under any entry name. The key
    /// leaves the class out, so a hit of another class is another object.
    fn objects_by_norm(&self, norm: NormKey) -> Result<Vec<ObjectHit>, HistoryError>;

    /// The blob of the text of the entry `entry` of the bin at `bin` at the build at `at`, in
    /// the first WAD in `wad_order` whose bin holds it. `reforge-census` reads the text from the
    /// census history by the blob.
    fn text_blob(
        &self,
        bin: WadHash,
        entry: BinHash,
        at: Position,
    ) -> Result<Option<TextBlob>, HistoryError>;

    /// The bytes of the object `entry` of the bin at `bin` at the build at `at`, in the WAD
    /// `text_blob` names, decoded from two frames at most. None when the build holds no such
    /// object, or its text held no object or several.
    fn object_bytes(
        &self,
        bin: WadHash,
        entry: BinHash,
        at: Position,
    ) -> Result<Option<ObjectBytes>, HistoryError>;

    /// The entry hashes the object `entry` of the bin at `bin` linked at the build at `at`, in
    /// hash order.
    fn links(
        &self,
        bin: WadHash,
        entry: BinHash,
        at: Position,
    ) -> Result<Vec<BinHash>, HistoryError>;

    /// The objects the object `entry` of the bin at `bin` reaches by its links at the build at
    /// `at`, and the objects those reach: a linked entry is followed into every bin that held
    /// it at that build. One recursive query, bounded by [`MAX_REACHED`]. No object when the
    /// build holds no such object.
    fn reached(&self, bin: WadHash, entry: BinHash, at: Position) -> Result<Reached, HistoryError>;

    /// The types the game's texts gave the field `field` of the class `class` at the build at
    /// `at`: the field's kind at that build, for `retype`. A field whose values had two kinds at
    /// one build has both, and a list of pointers one per class its items had.
    fn field_types(
        &self,
        class: BinHash,
        field: BinHash,
        at: Position,
    ) -> Result<Vec<FieldType>, HistoryError>;

    /// Whether the game ever shipped this clip, and when.
    fn clip(&self, key: ClipKey) -> Result<Option<Life>, HistoryError>;

    /// Every game object that named this path, oldest first.
    fn named_by(&self, path: WadHash) -> Result<Vec<NamedBy>, HistoryError>;

    /// Every game bank unit that loaded this bank, oldest first.
    fn units_loading(&self, bank: WadHash) -> Result<Vec<UnitHit>, HistoryError>;

    /// The wems an event of a bank played, in wem order.
    fn event_wems(&self, bank: WadHash, event: u32) -> Result<Vec<WemPlayed>, HistoryError>;

    /// The events that played this wem, in bank and event order.
    fn events_playing(&self, wem: u32) -> Result<Vec<PlayedBy>, HistoryError>;

    /// Every skeleton the game held at this path, oldest first.
    fn skeletons(&self, path: WadHash) -> Result<Vec<SkeletonHit>, HistoryError>;

    /// Every mesh the game held at this path, oldest first.
    fn meshes(&self, path: WadHash) -> Result<Vec<MeshHit>, HistoryError>;

    /// Every animation clip the game held at this path with the joints it animates, oldest
    /// first.
    fn clip_joints(&self, path: WadHash) -> Result<Vec<ClipJoints>, HistoryError>;

    /// The joints the animation clip of these bytes animates, in stored order. None when the
    /// game never shipped the bytes as a clip.
    fn joints_of(&self, key: FileKey) -> Result<Option<Vec<u32>>, HistoryError>;

    /// Every events bank the game held at this path, one per file, with the WADs that held it
    /// and its header and its objects as stored, oldest first.
    fn events_banks(&self, path: WadHash) -> Result<Vec<EventsBankHit>, HistoryError>;

    /// The head: the position of the build appended last, which every key still present lasts
    /// to. It is not always the newest build (`Stamp::newest_build`).
    fn head(&self) -> Option<Position> {
        self.builds().last().map(|row| row.at)
    }

    /// The build at a position.
    fn build(&self, at: Position) -> Option<&BuildRow> {
        self.builds().get(usize::from(at.0))
    }

    /// The version of the build at a position.
    fn version(&self, at: Position) -> Option<&Build> {
        self.build(at).map(|row| &row.build)
    }

    /// The position of the first build of a patch, `16.19`.
    fn first_of_patch(&self, patch: &str) -> Option<Position> {
        let of_patch = |row: &&BuildRow| {
            let rest = row.build.as_str().strip_prefix(patch);
            rest.is_some_and(|rest| rest.starts_with('.'))
        };
        self.builds().iter().find(of_patch).map(|row| row.at)
    }

    /// Whether a life reaches the head: the game holds the key at its last build.
    fn reaches_head(&self, life: Life) -> bool {
        self.head() == Some(life.last)
    }

    /// A life spelled as versions, `16.14.1 .. 16.19.8`. A position with no build is `#<n>`.
    fn span(&self, life: Life) -> String {
        let version = |at: Position| {
            self.version(at)
                .map_or_else(|| format!("#{at}"), ToString::to_string)
        };
        if life.first == life.last {
            version(life.first)
        } else {
            format!("{} .. {}", version(life.first), version(life.last))
        }
    }
}

/// No history: every question is answered with nothing, so a run is judged against the install
/// only.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHistory;

impl History for NoHistory {
    fn stamp(&self) -> Option<&Stamp> {
        None
    }

    fn builds(&self) -> &[BuildRow] {
        &[]
    }

    fn file(&self, _: FileKey) -> Result<Option<FileHit>, HistoryError> {
        Ok(None)
    }

    fn path(&self, _: WadHash) -> Result<Option<PathHit>, HistoryError> {
        Ok(None)
    }

    fn held(&self, _: WadHash, _: FileKey) -> Result<Option<Life>, HistoryError> {
        Ok(None)
    }

    fn paths_of(&self, _: FileKey) -> Result<Vec<HeldAt>, HistoryError> {
        Ok(Vec::new())
    }

    fn placed(&self, _: WadHash, _: Position) -> Result<Vec<Placed>, HistoryError> {
        Ok(Vec::new())
    }

    fn bin(&self, _: WadHash, _: Position) -> Result<Option<BinHit>, HistoryError> {
        Ok(None)
    }

    fn object(&self, _: ObjectKey) -> Result<Option<ObjectHit>, HistoryError> {
        Ok(None)
    }

    fn object_lives(&self, _: ObjectKey) -> Result<Vec<ObjectHit>, HistoryError> {
        Ok(Vec::new())
    }

    fn objects_by_norm(&self, _: NormKey) -> Result<Vec<ObjectHit>, HistoryError> {
        Ok(Vec::new())
    }

    fn text_blob(
        &self,
        _: WadHash,
        _: BinHash,
        _: Position,
    ) -> Result<Option<TextBlob>, HistoryError> {
        Ok(None)
    }

    fn object_bytes(
        &self,
        _: WadHash,
        _: BinHash,
        _: Position,
    ) -> Result<Option<ObjectBytes>, HistoryError> {
        Ok(None)
    }

    fn links(&self, _: WadHash, _: BinHash, _: Position) -> Result<Vec<BinHash>, HistoryError> {
        Ok(Vec::new())
    }

    fn reached(&self, _: WadHash, _: BinHash, _: Position) -> Result<Reached, HistoryError> {
        Ok(Reached::default())
    }

    fn field_types(
        &self,
        _: BinHash,
        _: BinHash,
        _: Position,
    ) -> Result<Vec<FieldType>, HistoryError> {
        Ok(Vec::new())
    }

    fn clip(&self, _: ClipKey) -> Result<Option<Life>, HistoryError> {
        Ok(None)
    }

    fn named_by(&self, _: WadHash) -> Result<Vec<NamedBy>, HistoryError> {
        Ok(Vec::new())
    }

    fn units_loading(&self, _: WadHash) -> Result<Vec<UnitHit>, HistoryError> {
        Ok(Vec::new())
    }

    fn event_wems(&self, _: WadHash, _: u32) -> Result<Vec<WemPlayed>, HistoryError> {
        Ok(Vec::new())
    }

    fn events_playing(&self, _: u32) -> Result<Vec<PlayedBy>, HistoryError> {
        Ok(Vec::new())
    }

    fn skeletons(&self, _: WadHash) -> Result<Vec<SkeletonHit>, HistoryError> {
        Ok(Vec::new())
    }

    fn meshes(&self, _: WadHash) -> Result<Vec<MeshHit>, HistoryError> {
        Ok(Vec::new())
    }

    fn clip_joints(&self, _: WadHash) -> Result<Vec<ClipJoints>, HistoryError> {
        Ok(Vec::new())
    }

    fn joints_of(&self, _: FileKey) -> Result<Option<Vec<u32>>, HistoryError> {
        Ok(None)
    }

    fn events_banks(&self, _: WadHash) -> Result<Vec<EventsBankHit>, HistoryError> {
        Ok(Vec::new())
    }
}

/// An error and what caused it, on one line.
fn chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// The history a run asks: the index in `index`, or none. Raises `history.absent` when no index
/// was given and `history.index-missing` when the one given does not open. The run goes on
/// against the install alone in both cases.
pub fn open(index: Option<&Utf8Path>, sink: &mut Sink<'_>) -> Box<dyn History> {
    let Some(dir) = index else {
        sink.raise(HistoryAbsent);
        return Box::new(NoHistory);
    };
    match SqliteHistory::open(dir) {
        Ok(history) => Box::new(history),
        Err(error) => {
            sink.raise(IndexMissing {
                index: dir.to_string(),
                reason: chain(&error),
            });
            Box::new(NoHistory)
        }
    }
}
