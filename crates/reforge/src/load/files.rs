//! File fixups: repairs of the file set, made before characters and units are found, so that
//! what they make is seen as the mod's own file at its path.

use std::sync::Arc;

use crate::diag::{Record, Sink};
use crate::game::{Game, NameBook, Names, Speller};
use crate::history::{History, HistoryError};
use crate::model::entity::{FileId, FileKind, FileNode, FileSource, FileUse, MadeBy, Occupied};
use crate::model::{ChunkPath, ContentId, Mod, Spell};

/// A repair of the mod's file set. It adds files and changes none: the file it starts from
/// stays, for what the mod's own bins name.
pub trait FileFixup: Send + Sync {
    /// The name a caller turns it on and off by, `legacy-formats`.
    fn id(&self) -> &'static str;

    /// The records it implements. A fixup of several names, in each finding, the ones each
    /// file rests on.
    fn records(&self) -> &'static [Record];

    /// Makes the files the rule gives, each with the file it was made from as its source. A
    /// history that does not answer is a fault, and nothing is made.
    fn fix(&self, cx: &mut FileFixCx<'_, '_>) -> Result<(), HistoryError>;

    /// The paths at which a file of the mod stands for the file at `path`: where the rule
    /// makes a file from it, or would when the mod did not ship one there itself. A file at a
    /// path the game does not ship is counted as covered when the mod holds a file at one of
    /// them.
    fn destinations(&self, path: &ChunkPath, game: &dyn Game) -> Vec<ChunkPath> {
        let _ = (path, game);
        Vec::new()
    }
}

/// The one record a fixup implements, for the sink it raises through. A fixup of several
/// records leaves the sink's record empty.
pub fn one_record(fixup: &dyn FileFixup) -> Option<Record> {
    match fixup.records() {
        [record] => Some(*record),
        _ => None,
    }
}

/// A file a fixup makes from another file of the mod.
#[derive(Debug, Clone)]
pub struct NewFile {
    /// The file it is made from. The new file goes in the same WAD.
    pub from: FileId,
    /// The rule that makes it.
    pub by: MadeBy,
    /// Its path.
    pub to: ChunkPath,
    /// Its bytes: the source's own, or new ones.
    pub bytes: Arc<[u8]>,
    /// The kind the bytes are.
    pub kind: FileKind,
}

/// What a file fixup works on and is judged against.
pub struct FileFixCx<'a, 's> {
    /// The mod, with its WADs and files and no character yet.
    pub module: &'a mut Mod,
    /// The target game.
    pub game: &'a dyn Game,
    /// The build history.
    pub history: &'a dyn History,
    /// The name tables.
    pub names: &'a Names,
    /// The run's own names, which learn the path of each file made.
    pub book: &'a mut NameBook,
    /// Where the fixup raises what it made.
    pub sink: &'a mut Sink<'s>,
}

impl FileFixCx<'_, '_> {
    /// Spells hashes from the run's names and the tables.
    pub fn speller(&self) -> Speller<'_> {
        Speller::new(self.book, self.names)
    }

    /// The path of a file: the one the archive spells, else the one the names spell for its
    /// hash.
    pub fn path_of(&self, file: FileId) -> Option<ChunkPath> {
        let node = &self.module.files[file];
        node.path
            .clone()
            .or_else(|| ChunkPath::parse(&self.speller().path(node.hash)?).ok())
    }

    /// Adds a file a fixup made, in the WAD of the file it was made from, and has the run's
    /// names learn its path. Where the WAD holds a file at the path nothing is added: the file
    /// there is the asset's.
    pub fn add(&mut self, file: NewFile) -> Result<FileId, Occupied> {
        let source = &self.module.files[file.from];
        let wad = source.wad;
        let content = if Arc::ptr_eq(&file.bytes, &source.bytes) {
            source.content
        } else {
            ContentId::of(&file.bytes)
        };
        let hash = self.book.learn_path(file.to.as_str());
        self.module.push_file(FileNode {
            wad,
            hash,
            path: Some(file.to),
            size: file.bytes.len() as u64,
            content,
            kind: file.kind,
            bytes: file.bytes,
            used: FileUse::Unused,
            source: FileSource::Made {
                from: file.from,
                by: file.by,
            },
        })
    }
}
