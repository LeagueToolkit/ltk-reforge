//! The mod from its archive: WADs, files, bins and objects, the characters and skin units they
//! make, and what the client loads of them. Facts of the archive only. Nothing here reads a
//! value inside an object to judge it. The archive comes as its raw snapshot and the store
//! its bytes are in (#ADR-0094): the snapshot carries everything `load` reads of it.
//!
//! The stage is five steps, each a method of [`Loader`], in this order:
//!
//! 1. `open`: the WADs and files of the archive, their bytes from the store;
//! 2. `fix_files`: the file fixups that make files, so that what they make is seen as the
//!    mod's own file;
//! 3. `take_game_bins`: the game's bins for a WAD with no skin bin (#MOD-ARTIFACT-0013);
//! 4. `survey`: WAD roles, characters and skin units, by path;
//! 5. `resolve`: the bins the client loads, each unit's load and entry, each file's use, then
//!    the file fixups that rewrite a file the client uses.
//!
//! Ids are handed out in creation order: WADs by lowercase name, files by path hash within a
//! WAD with the files made after them, bins in load order (each unit's load in unit order,
//! then the bins at game paths), objects in each bin's file order, a champion before its
//! sub-characters by name, units by slot within a character.
//!
//! `docs/design/ARCHITECTURE.md`, "The model" and "Fixups" (file fixups).

pub mod bank;
mod binless;
mod client;
pub mod dating;
pub mod files;
mod findings;
pub mod inventory;
pub mod legacy;
pub mod moved;
pub mod provenance;
pub mod rework;
mod stale;
mod survey;

use std::collections::BTreeMap;
use std::sync::Arc;

use self::bank::BankId;
use self::binless::Binless;
use self::client::ClientLoad;
pub use self::dating::Dating;
pub use self::files::{FileFixCx, FileFixup, FileStep, NewFile, one_record};
pub use self::findings::{
    BinUnreadable, CODES, ChampionAmbiguous, ChampionGuessed, EntryDuplicates, EqualToGame,
    FormerPathFiles, LinksMissing, LoadCapped, LocalePacked, LocaleWad, MissingLink, NoSkin,
    Restoration, SeveralChampions, StaleFiles, TakenFromGame, TakenSkin, UnusedFiles, WadDuplicate,
    WadDuplicateDiffers, WadNotCarried,
};
pub use self::inventory::Inventory;
use self::legacy::LegacyFormats;
use self::moved::MovedAssets;
pub use self::provenance::{Against, FileProvenance};
pub use self::rework::{Around, ReworkTable};
pub use self::stale::Staleness;
use self::survey::Survey;
use crate::archive::{RawSnapshot, file_kind};
use crate::diag::{Cancelled, Halted, Parts, Sink};
use crate::env::Env;
use crate::game::{GameError, NameBook, Speller};
use crate::history::HistoryError;
use crate::model::entity::{FileId, FileKind, FileNode, FileSource, FileUse, ModWad, WadRole};
use crate::model::known::{class, field};
use crate::model::{ChunkPath, ContentId, Mod, Spell, WadHash, WadName, bin, value};
use crate::store::BlobStore;

/// How many bins one root's load follows, the root included.
// #ADR-0070, measured 2026-10-06: the most bins one root of the datasets reaches is 89, and the
// next mod's most is 24.
pub const MAX_LOAD_BINS: usize = 128;

/// An input of the run that did not answer while a mod was judged against it: a fault of the
/// machine or the tool, and no diagnostic.
#[derive(Debug, thiserror::Error)]
pub enum Fault {
    /// The history's index did not answer a question.
    #[error("the history did not answer")]
    History(#[from] HistoryError),
    /// A chunk the game holds did not read.
    #[error("the game did not read")]
    Game(#[from] GameError),
}

/// Why `load` gives no mod.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// A fatal diagnostic: the archive leaves nothing to write.
    #[error(transparent)]
    Halted(#[from] Halted),
    /// The run's cancel ended the stage between two of its steps (#ADR-0097).
    #[error(transparent)]
    Cancelled(#[from] Cancelled),
    /// The history did not answer: a fault of the index or the machine, and no diagnostic.
    #[error("the history did not answer while the mod was loaded")]
    History(#[source] HistoryError),
    /// A chunk the game holds did not read: a fault of the install or the machine, and no
    /// diagnostic.
    #[error("the game did not read while the mod was loaded")]
    Game(#[source] GameError),
    /// The blob store holds nothing under an id the snapshot names: a fault of the store, and
    /// no diagnostic.
    #[error("the store held no bytes under {0:?} while the mod was loaded")]
    Store(ContentId),
}

impl From<Fault> for LoadError {
    fn from(fault: Fault) -> Self {
        match fault {
            Fault::History(error) => LoadError::History(error),
            Fault::Game(error) => LoadError::Game(error),
        }
    }
}

/// What `load` is configured with.
#[derive(Clone)]
pub struct LoadOptions {
    /// Carries stale copies (#MOD-ARTIFACT-0004) as the mod's own files. Off: they are no
    /// override (#ADR-0023).
    pub keep_stale: bool,
    /// The champions whose model was replaced, and when.
    pub reworks: ReworkTable,
    /// The file fixups to run, in this order.
    pub fixups: Vec<Arc<dyn FileFixup>>,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            keep_stale: false,
            reworks: ReworkTable::standard(),
            fixups: Self::standard_fixups(),
        }
    }
}

impl LoadOptions {
    /// The options a run starts from: stale copies left out, the reworks of
    /// #GAME-CHANGE-0001, the standard file fixups.
    pub fn new() -> Self {
        Self::default()
    }

    /// The built-in file fixups, in the order they run. `moved-assets` comes after
    /// `legacy-formats`: it skips a file a twin was made of. `bank-id` runs at a later step
    /// than both ([`FileStep::AfterResolve`]), wherever it is in the list.
    pub fn standard_fixups() -> Vec<Arc<dyn FileFixup>> {
        vec![
            Arc::new(LegacyFormats),
            Arc::new(MovedAssets),
            Arc::new(BankId),
        ]
    }

    /// Turns a file fixup off by its id, `legacy-formats`. An id of no fixup in the list
    /// changes nothing.
    #[must_use]
    pub fn without_fixup(mut self, id: &str) -> Self {
        self.fixups.retain(|fixup| fixup.id() != id);
        self
    }

    /// Runs no file fixup.
    #[must_use]
    pub fn without_fixups(mut self) -> Self {
        self.fixups.clear();
        self
    }

    /// Runs these file fixups, in this order, and no other: what a chosen set of fixups
    /// holds for the file set.
    #[must_use]
    pub fn file_fixups(mut self, fixups: Vec<Arc<dyn FileFixup>>) -> Self {
        self.fixups = fixups;
        self
    }

    /// Carries stale copies, or leaves them out.
    #[must_use]
    pub fn keep_stale(mut self, keep: bool) -> Self {
        self.keep_stale = keep;
        self
    }

    /// Judges restorations against these reworks.
    #[must_use]
    pub fn reworks(mut self, reworks: ReworkTable) -> Self {
        self.reworks = reworks;
        self
    }

    /// Runs this file fixup after the ones given before it.
    #[must_use]
    pub fn fixup(mut self, fixup: impl FileFixup + 'static) -> Self {
        self.fixups.push(Arc::new(fixup));
        self
    }
}

impl std::fmt::Debug for LoadOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fixups: Vec<&str> = self.fixups.iter().map(|fixup| fixup.id()).collect();
        f.debug_struct("LoadOptions")
            .field("keep_stale", &self.keep_stale)
            .field("reworks", &self.reworks)
            .field("fixups", &fixups)
            .finish()
    }
}

/// What the provenance of a mod's files is judged against, from an environment and the names
/// a run spells with.
pub fn against<'a>(env: &'a Env, names: &'a dyn Spell) -> Against<'a> {
    Against {
        game: env.game(),
        history: env.history(),
        names,
    }
}

/// The locale a WAD's extensions begin with, `en_us` of `en_us.wad.client`.
fn locale_of(extensions: &str) -> Option<String> {
    let tag = extensions.split('.').next()?;
    let b = tag.as_bytes();
    let shaped =
        b.len() == 5 && b[2] == b'_' && [0, 1, 3, 4].iter().all(|&i| b[i].is_ascii_alphabetic());
    shaped.then(|| tag.to_string())
}

/// The locale of a locale WAD, `<champion>.<xx_XX>.wad.client`, lowercase.
fn locale_tag(name: &WadName) -> Option<String> {
    let (_, extensions) = name.lower().split_once('.')?;
    locale_of(extensions)
}

/// What a step of `load` reads and changes.
struct Run<'a> {
    env: &'a Env,
    options: &'a LoadOptions,
    book: &'a mut NameBook,
    module: &'a mut Mod,
}

impl Run<'_> {
    fn speller(&self) -> Speller<'_> {
        Speller::new(self.book, self.env.names())
    }
}

/// The stage `load`: a `Mod` from a raw snapshot.
///
/// A loader serves one run. It holds the names the run learns: the paths the archive and its
/// bins spell, and the entry names of each character's skins.
pub struct Loader<'a> {
    env: &'a Env,
    options: LoadOptions,
    book: NameBook,
}

impl<'a> Loader<'a> {
    /// A loader against an environment, with the options a run starts from: the standard file
    /// fixups, stale copies left out.
    pub fn new(env: &'a Env) -> Self {
        Self {
            env,
            options: LoadOptions::default(),
            book: NameBook::new(),
        }
    }

    /// Loads under these options.
    #[must_use]
    pub fn options(mut self, options: LoadOptions) -> Self {
        self.options = options;
        self
    }

    /// Starts from names a caller learnt before, and adds to them.
    #[must_use]
    pub fn book(mut self, book: NameBook) -> Self {
        self.book = book;
        self
    }

    /// The names learnt so far.
    pub fn names(&self) -> &NameBook {
        &self.book
    }

    /// The names learnt, for the stages after `load`.
    pub fn into_names(self) -> NameBook {
        self.book
    }

    /// Spells hashes from the names learnt and the tables.
    pub fn speller(&self) -> Speller<'_> {
        Speller::new(&self.book, self.env.names())
    }

    fn run<'r>(&'r mut self, module: &'r mut Mod) -> Run<'r> {
        Run {
            env: self.env,
            options: &self.options,
            book: &mut self.book,
            module,
        }
    }

    /// Gives a path to each file that has none and whose hash the names learnt or the tables
    /// spell. A chunk of a packed WAD and a chunk named by its hash come without one.
    fn name_files(&self, m: &mut Mod) {
        let speller = self.speller();
        let unnamed: Vec<FileId> = m
            .files
            .iter()
            .filter(|(_, file)| file.path.is_none())
            .map(|(id, _)| id)
            .collect();
        for id in unnamed {
            let hash = m.files[id].hash;
            let spelled = speller
                .path(hash)
                .and_then(|path| ChunkPath::parse(&path).ok());
            // A checked path reads `\` as `/`, which a hash does not.
            m.files[id].path = spelled.filter(|path| path.hash() == hash);
            if m.files[id].path.is_some() {
                m.named_by_table.insert(id);
            }
        }
    }

    /// Gives each file that has no path the path a bin of the mod spells for its hash, for the
    /// file fixups to read, and says which files it named. The caller takes the paths back.
    ///
    /// A file fixup finds the file it makes a twin of by its path. A chunk of a packed WAD at
    /// a path the game dropped has a path only where a bin of the mod spells it, which no
    /// table does, and `resolve` learns that path after the fixups ran. Without it the twin is
    /// missing from a repair, and is made when the repaired mod is repaired again (#ADR-0011).
    /// The path is the fixups' alone: what names a file for the rest of the run is what the
    /// client loads, read in `resolve`.
    fn lend_spelled_paths(m: &mut Mod) -> Vec<FileId> {
        if m.files.values().all(|file| file.path.is_some()) {
            return Vec::new();
        }
        let mut spelled: BTreeMap<WadHash, ChunkPath> = BTreeMap::new();
        for file in m.files.values().filter(|file| file.kind == FileKind::Bin) {
            let Ok(parsed) = bin::parse(&file.bytes) else {
                continue;
            };
            let objects = parsed.objects.iter();
            for named in objects.flat_map(|object| value::file_refs(&object.value)) {
                let path = named.spelled().and_then(|path| ChunkPath::parse(path).ok());
                if let Some(path) = path {
                    spelled.entry(path.hash()).or_insert(path);
                }
            }
        }
        let unnamed: Vec<FileId> = m
            .files
            .iter()
            .filter(|(_, file)| file.path.is_none())
            .map(|(id, _)| id)
            .collect();
        let mut lent = Vec::new();
        for id in unnamed {
            if let Some(path) = spelled.get(&m.files[id].hash) {
                m.files[id].path = Some(path.clone());
                m.named_by_table.insert(id);
                lent.push(id);
            }
        }
        lent
    }

    /// Step 1: the mod's WADs and files as the archive ships them, from its raw snapshot and
    /// the store its bytes are in. No WAD has a role and no file a use yet. A store that
    /// holds nothing under an id the snapshot names is a fault, `LoadError::Store`.
    pub fn open(&mut self, raw: &RawSnapshot, store: &dyn BlobStore) -> Result<Mod, LoadError> {
        // The names a run reads by are spelled whatever tables are installed.
        for name in field::ALL {
            self.book.learn_field(name);
        }
        for name in class::ALL {
            self.book.learn_class(name);
        }
        let mut m = Mod::new(raw.meta.clone());
        m.extras = raw.open_extras(store).map_err(LoadError::Store)?;
        let image = raw.image.map(|image| image.open(store)).transpose();
        m.image = image.map_err(LoadError::Store)?;
        for wad in &raw.wads {
            let id = m.wads.push(ModWad {
                name: wad.name.clone(),
                role: WadRole::Other,
                layout: wad.layout,
                files: Vec::with_capacity(wad.files.len()),
                packed_locale: None,
            });
            for (&hash, file) in &wad.files {
                if let Some(path) = &file.path {
                    self.book.learn_path(path.as_str());
                }
                let bytes = store
                    .get(file.content)
                    .ok_or(LoadError::Store(file.content))?;
                // The archive gives one chunk per path of a WAD: a second one was skipped and
                // reported where it was read (`archive.chunk-duplicate`).
                let _ = m.push_file(FileNode {
                    wad: id,
                    hash,
                    path: file.path.clone(),
                    size: bytes.len() as u64,
                    content: file.content,
                    kind: file_kind(&bytes),
                    bytes,
                    used: FileUse::Unused,
                    source: FileSource::Archive,
                });
            }
        }
        self.name_files(&mut m);
        Ok(m)
    }

    /// Runs the file fixups of one step, in the order the options hold them. Each raises
    /// through a sink that names it, and the one record it implements when it implements one.
    fn run_file_fixups(
        &mut self,
        step: FileStep,
        m: &mut Mod,
        sink: &mut Sink<'_>,
    ) -> Result<(), Fault> {
        let fixups = self.options.fixups.clone();
        for fixup in fixups.iter().filter(|fixup| fixup.step() == step) {
            let mut sink = sink.by(fixup.id(), one_record(&**fixup));
            fixup.fix(&mut FileFixCx {
                module: m,
                game: self.env.game(),
                history: self.env.history(),
                names: self.env.names(),
                book: &mut self.book,
                sink: &mut sink,
            })?;
        }
        Ok(())
    }

    /// Step 2: runs the file fixups the options hold that make files
    /// ([`FileStep::BeforeSurvey`]), in their order.
    ///
    /// What a fixup makes is not charged to the archive's budget: an SCB written from an SCO
    /// is new bytes, of about the size of the file it was made from.
    pub fn fix_files(&mut self, m: &mut Mod, sink: &mut Sink<'_>) -> Result<(), Fault> {
        let lent = Self::lend_spelled_paths(m);
        let ran = self.run_file_fixups(FileStep::BeforeSurvey, m, sink);
        for id in lent {
            m.files[id].path = None;
            m.named_by_table.remove(&id);
        }
        ran?;
        self.name_files(m);
        Ok(())
    }

    /// Step 3: for each WAD with no skin bin, takes the game's bins of the skins that name the
    /// files it overrides (#MOD-ARTIFACT-0013), and marks the WADs that restore a model from
    /// before its champion's rework.
    pub fn take_game_bins(&mut self, m: &mut Mod, sink: &mut Sink<'_>) -> Result<(), Fault> {
        Binless::new(self.run(m)).take(sink)
    }

    /// Step 4: WAD roles, characters and one skin unit per skin bin. Ends the run, as
    /// `LoadError::Halted`, when no WAD is a champion's with a skin bin in it.
    pub fn survey(&mut self, m: &mut Mod, sink: &mut Sink<'_>) -> Result<(), LoadError> {
        Survey::new(self.run(m)).run(sink)
    }

    /// Step 5: reads the bins the client loads, gives each unit its load and its skin entry,
    /// and each file its use. Then runs the file fixups the options hold that rewrite a file
    /// the client uses ([`FileStep::AfterResolve`]), in their order.
    pub fn resolve(&mut self, m: &mut Mod, sink: &mut Sink<'_>) -> Result<(), Fault> {
        ClientLoad::new(self.run(m)).run(sink)?;
        self.name_files(m);
        self.run_file_fixups(FileStep::AfterResolve, m, sink)
    }

    /// The whole stage: the five steps in order, each reported and the run's cancel read
    /// before each (#ADR-0097). The results on the files (`Dating`, `Inventory`,
    /// `FileProvenance`) are the caller's to gather on the mod returned.
    pub fn load(
        &mut self,
        raw: &RawSnapshot,
        store: &dyn BlobStore,
        sink: &mut Sink<'_>,
    ) -> Result<Mod, LoadError> {
        let _span = tracing::info_span!("load", archive = %raw.meta.file_name).entered();
        let mut parts = Parts::new(&LOAD_PARTS[..5]);
        parts.begin(sink)?;
        let mut m = self.open(raw, store)?;
        parts.begin(sink)?;
        self.fix_files(&mut m, sink)?;
        parts.begin(sink)?;
        self.take_game_bins(&mut m, sink)?;
        parts.begin(sink)?;
        self.survey(&mut m, sink)?;
        parts.begin(sink)?;
        self.resolve(&mut m, sink)?;
        Ok(m)
    }
}

/// The steps of `load` (#ADR-0097): the five of the loader, then what is known of the files,
/// the inventory and the file provenance in a port alone.
pub const LOAD_PARTS: &[&str] = &[
    "open",
    "file fixups",
    "game bins",
    "survey",
    "client load",
    "dating",
    "inventory",
    "provenance",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locale_tag_is_two_letters_an_underscore_and_two_letters() {
        assert_eq!(locale_of("en_us.wad.client"), Some("en_us".into()));
        assert_eq!(locale_of("wad.client"), None);
        let wad = |name: &str| WadName::parse(name).expect("a WAD name");
        assert_eq!(
            locale_tag(&wad("Graves.en_US.wad.client")),
            Some("en_us".into())
        );
        assert_eq!(locale_tag(&wad("Graves.wad.client")), None);
    }
}
