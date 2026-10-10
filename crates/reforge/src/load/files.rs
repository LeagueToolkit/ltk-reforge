//! File fixups: repairs of the file set. One that makes files runs before characters and units
//! are found, so that what it makes is seen as the mod's own file at its path. One that
//! rewrites a file the client uses runs once each file has its use.

use crate::diag::{Record, Sink};
use crate::game::{GameSource, NameBook, Names, Speller};
use crate::history::{History, HistoryError};
use crate::model::entity::{FileId, FileKind, FileNode, FileSource, FileUse, MadeBy, Occupied};
use crate::model::{ByteView, ChunkPath, ContentId, FixupInfo, Mod, Spell};

/// When in `load` a file fixup runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileStep {
    /// Before characters and units are found: a file it makes is surveyed as the mod's own.
    BeforeSurvey,
    /// After the bins the client loads are read: each file has its use, and its path where a
    /// bin of the mod spells it.
    AfterResolve,
}

/// A repair of the mod's file set. It adds a file beside the one it starts from, which stays
/// for what the mod's own bins name, or rewrites the bytes of a file where it is.
pub trait FileFixup: Send + Sync {
    /// What a caller lists and toggles it by: its id, the records it implements, its phase.
    /// A fixup of several records names, in each finding, the ones each file rests on.
    fn info(&self) -> &FixupInfo;

    /// When in `load` it runs.
    fn step(&self) -> FileStep {
        FileStep::BeforeSurvey
    }

    /// The name a caller turns it on and off by, `legacy-formats`.
    fn id(&self) -> &str {
        self.info().id.as_str()
    }

    /// The records it implements.
    fn records(&self) -> &[Record] {
        &self.info().records
    }

    /// Makes the files the rule gives, each with the file it was made from as its source. A
    /// history that does not answer is a fault, and nothing is made.
    fn fix(&self, cx: &mut FileFixCx<'_, '_>) -> Result<(), HistoryError>;

    /// The paths at which a file of the mod stands for the file at `path`: where the rule
    /// makes a file from it, or would when the mod did not ship one there itself. A file at a
    /// path the game does not ship is counted as covered when the mod holds a file at one of
    /// them.
    fn destinations(&self, path: &ChunkPath, game: &dyn GameSource) -> Vec<ChunkPath> {
        let _ = (path, game);
        Vec::new()
    }
}

/// The one record a fixup implements, for the sink it raises through. A fixup of several
/// records leaves the sink's record empty.
pub fn one_record(fixup: &dyn FileFixup) -> Option<Record> {
    fixup.info().one_record()
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
    pub bytes: ByteView,
    /// The kind the bytes are.
    pub kind: FileKind,
}

/// What a file fixup works on and is judged against.
pub struct FileFixCx<'a, 's> {
    /// The mod, with its WADs and files and no character yet.
    pub module: &'a mut Mod,
    /// The target game.
    pub game: &'a dyn GameSource,
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
        let same_view =
            file.bytes.as_ptr() == source.bytes.as_ptr() && file.bytes.len() == source.bytes.len();
        let content = if same_view {
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

    /// Replaces the bytes of a file with the ones a fixup wrote, where the file is. The file
    /// keeps its id, its path, its kind and its use, and its size and content follow the new
    /// bytes. A file the archive ships keeps the digest of the bytes it shipped in its source.
    pub fn rewrite(&mut self, file: FileId, bytes: ByteView, by: MadeBy) {
        let node = &mut self.module.files[file];
        if node.source == FileSource::Archive {
            let shipped = node.content;
            node.source = FileSource::Rewritten { by, shipped };
        }
        node.size = bytes.len() as u64;
        node.content = ContentId::of(&bytes);
        node.bytes = bytes;
    }
}
