//! Why the census history does not read, or an index is not written.

use camino::Utf8PathBuf;
use reforge::history::objects::FrameError;
use reforge::history::{HistoryError, Position};

use crate::format::FORMAT;

/// Why the census history does not read, or an index is not written.
#[derive(Debug, thiserror::Error)]
pub enum CensusError {
    /// gix refused an object, a reference or the repository.
    #[error("{what}")]
    Git {
        /// What was being read.
        what: String,
        /// What gix refused.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// The census history's server has no branch that holds the builds.
    #[error("{url} has no {branch}")]
    NoRemoteBranch {
        /// The server's address.
        url: String,
        /// The branch looked for.
        branch: String,
    },
    /// The census history's server does not leave out what an object reaches, so a fetch of
    /// one commit would take the whole history.
    #[error("{url} does not filter what it sends")]
    NoFilter {
        /// The server's address.
        url: String,
    },
    /// The server sent none of the objects a request asked for, asked again on their own.
    #[error("{url} sent none of {objects} objects asked for")]
    Unsent {
        /// The server's address.
        url: String,
        /// How many objects were asked for and not sent.
        objects: usize,
    },
    /// The fetch was stopped.
    #[error("the fetch was stopped")]
    Cancelled,
    /// The clone has no branch that holds the builds.
    #[error("{repo} has neither {local} nor {remote}")]
    NoBranch {
        /// The clone.
        repo: Utf8PathBuf,
        /// The branch of a clone that checked it out.
        local: String,
        /// The branch of a clone that never did.
        remote: String,
    },
    /// No commit of the history holds this build.
    #[error("no build {0} in the history")]
    NoBuild(String),
    /// No commit of the history has this id or starts with it.
    #[error("no commit {0} in the history")]
    NoCommit(String),
    /// More than one commit of the history starts with this.
    #[error("{prefix} names {count} commits")]
    AmbiguousCommit {
        /// The prefix given.
        prefix: String,
        /// How many commits start with it.
        count: usize,
    },
    /// A commit lacks one of the files every build has.
    #[error("commit {commit} has no {file}")]
    MissingFile {
        /// The commit.
        commit: String,
        /// The file, from the tree's root.
        file: &'static str,
    },
    /// A commit is of another census format than this crate reads.
    #[error("commit {commit} is census format {found:?} and reforge reads format {FORMAT}")]
    Format {
        /// The commit.
        commit: String,
        /// The format its `census.yaml` states.
        found: Option<u32>,
    },
    /// A commit's `build.yaml` states no version that is a build.
    #[error("the build.yaml of commit {commit} states no build")]
    NoVersion {
        /// The commit.
        commit: String,
    },
    /// The history holds no build.
    #[error("the history has no commits")]
    Empty,
    /// The history holds more builds than a position numbers.
    #[error("{0} builds, more than the index numbers")]
    TooManyBuilds(usize),
    /// A build of the index is not the history's at the same position: the history was
    /// rewritten, or the index is of another.
    #[error("build {at} of the index, commit {commit}, is not the history's; rebuild the index")]
    Diverged {
        /// The position.
        at: usize,
        /// The index's commit there.
        commit: String,
    },
    /// An appended build removes a record of a key the index has no row for.
    #[error("{table}: build {at} removes a record the index never held: {key}")]
    NeverHeld {
        /// The table.
        table: &'static str,
        /// The build's position.
        at: Position,
        /// The key.
        key: String,
    },
    /// An appended build removes more records of a key than the index counts.
    #[error(
        "{table}: build {at} removes {removed} records of {key}, which the index holds {held} times"
    )]
    Overdrawn {
        /// The table.
        table: &'static str,
        /// The build's position.
        at: Position,
        /// The key.
        key: String,
        /// How many records the build removes, net.
        removed: i64,
        /// How many the index counts.
        held: i64,
    },
    /// A bin holds an object the index has no row of. The walk writes a bin's objects from the
    /// records that write its row, so the index or the walk is broken.
    #[error("a bin holds object {key} of class {class:08x}, which the index lacks")]
    NoObject {
        /// The object's key.
        key: String,
        /// The object's class.
        class: u32,
    },
    /// A row of the object bytes an append reads names a keyframe the entry's rows lack.
    #[error("row {object} of the object bytes names keyframe {keyframe}, which its entry lacks")]
    ObjectKeyframe {
        /// The row of `object_bytes`.
        object: i64,
        /// The keyframe it names.
        keyframe: i64,
    },
    /// A frame of the object bytes an append reads does not decode.
    #[error("row {object} of the object bytes does not decode")]
    ObjectFrame {
        /// The row of `object_bytes`.
        object: i64,
        /// Why.
        #[source]
        source: FrameError,
    },
    /// A dictionary of the index does not load into zstd.
    #[error("dictionary {0} of the object bytes does not load")]
    ObjectDict(i64),
    /// The index's object bytes were written by another zstd than the one running, which may
    /// write other frames of the same bytes; an append keeps the stamp's codec.
    #[error("the object bytes were written by zstd {stamped} and zstd {running} is running")]
    ObjectZstd {
        /// The version the stamp names.
        stamped: String,
        /// The version linked.
        running: String,
    },
    /// The index beside the tables does not read or write.
    #[error(transparent)]
    Index(#[from] HistoryError),
    /// SQLite refused a statement of the writer.
    #[error("writing the index")]
    Sqlite(#[from] rusqlite::Error),
    /// A file of the index does not read, copy or move.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
