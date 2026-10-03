//! The name tables. Names are for reading, for spelling and for finding characters by pattern:
//! nothing a run decides rests on a hash having a name. Two sources, the first spelling winning:
//!
//! 1. the hash cache LTK Manager keeps on the machine (Mimir);
//! 2. CommunityDragon text tables, `<hex> <name>` per line, from the directories given.
//!
//! Field names are the exception to "cosmetic": a PTCH record addresses a property by a path of
//! plaintext field names, so a package can only patch a field some table spells. Every name
//! handed out hashes back to the hash it was asked for.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::io;
use std::sync::OnceLock;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use ltk_hashdb::HashDb;
use ltk_mimir_cache::{HashStore, Table};
use serde::Serialize;

use super::namehash::{NameHashError, NameHashIndex, NameHashSlot};
use crate::diag::{Channel, Code, Diag, Input, Message, Severity, Sink, Subject};
use crate::model::{BinHash, WadHash, name_hash, path_hash};

/// A kind of hash, and the table that spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NameTable {
    /// Chunk paths, by path hash.
    Paths,
    /// Entry names of bin objects.
    Entries,
    /// Field names.
    Fields,
    /// Class names.
    Classes,
    /// The plaintext behind a hash value or a hashed map key.
    Hashes,
}

impl NameTable {
    /// Every table.
    pub const ALL: [NameTable; 5] = [
        NameTable::Paths,
        NameTable::Entries,
        NameTable::Fields,
        NameTable::Classes,
        NameTable::Hashes,
    ];

    /// The table's name in a report.
    pub fn as_str(self) -> &'static str {
        match self {
            NameTable::Paths => "paths",
            NameTable::Entries => "entries",
            NameTable::Fields => "fields",
            NameTable::Classes => "classes",
            NameTable::Hashes => "hashes",
        }
    }

    /// The hash the game gives a name of this table. A bin hash is widened to 64 bits.
    pub fn hash_of(self, name: &str) -> u64 {
        match self {
            NameTable::Paths => path_hash(name).0,
            _ => u64::from(name_hash(name).0),
        }
    }

    fn slot(self) -> usize {
        self as usize
    }

    fn mimir(self) -> Table {
        match self {
            NameTable::Paths => Table::Game,
            NameTable::Entries => Table::BinEntries,
            NameTable::Fields => Table::BinFields,
            NameTable::Classes => Table::BinTypes,
            NameTable::Hashes => Table::BinHashes,
        }
    }

    /// The table a CommunityDragon file holds, by its lowercase file name. The paths come in
    /// parts, `hashes.game.txt.0` and on.
    fn of_text_file(file: &str) -> Option<Self> {
        match file {
            "hashes.binentries.txt" => Some(NameTable::Entries),
            "hashes.binfields.txt" => Some(NameTable::Fields),
            "hashes.bintypes.txt" => Some(NameTable::Classes),
            "hashes.binhashes.txt" => Some(NameTable::Hashes),
            _ => file
                .starts_with("hashes.game.txt")
                .then_some(NameTable::Paths),
        }
    }
}

impl fmt::Display for NameTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why the name tables do not build.
#[derive(Debug, thiserror::Error)]
pub enum NamesError {
    /// A directory of text tables, or a table in it, does not read.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// The name-hash index can be neither read nor written.
    #[error("the name-hash index")]
    NameHash(#[from] NameHashError),
}

/// A source of names that loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameSource {
    /// The hash cache.
    Mimir {
        /// The cache's directory.
        dir: Utf8PathBuf,
        /// The tables of it that opened.
        tables: Vec<NameTable>,
    },
    /// A directory of text tables.
    TextTables {
        /// The directory.
        dir: Utf8PathBuf,
        /// How many table files were read from it.
        files: usize,
    },
}

/// A table of the hash cache that did not open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AbsentTable {
    /// The table.
    pub table: NameTable,
    /// What failed.
    pub reason: String,
}

/// Tables of the hash cache that did not open at the directory given. Their hashes are spelled
/// from the text tables and the run's own names only.
#[derive(Debug, Clone, Serialize)]
pub struct MimirAbsent {
    /// The tables, in the order of [`NameTable::ALL`].
    pub tables: Vec<AbsentTable>,
}

impl MimirAbsent {
    /// The finding for a machine with no directory to look in, where [`mimir_cache_dir`] gives
    /// none: a caller that wanted the hash cache raises it in place of [`Names::report`].
    pub fn no_directory() -> Self {
        let tables = NameTable::ALL.map(|table| AbsentTable {
            table,
            reason: "the platform names no data directory".to_string(),
        });
        Self {
            tables: tables.to_vec(),
        }
    }
}

impl Diag for MimirAbsent {
    const CODE: Code = Code::new("names.mimir-absent", Channel::Run, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Input(Input::Names)
    }

    fn message(&self, f: &mut Message<'_>) {
        if self.tables.len() == NameTable::ALL.len() {
            f.text("the hash cache was not found; names come from the text tables only");
            return;
        }
        f.count(self.tables.len(), "table")
            .text(" of the hash cache not found: ");
        f.list(&self.tables, |f, absent| {
            f.text(absent.table.as_str());
        });
    }
}

/// The directory LTK Manager keeps its hash cache in on this machine: `MIMIR_DIR` when set,
/// else under the platform's data directory. The directory may not exist. `None` when the
/// platform names no data directory.
pub fn mimir_cache_dir() -> Option<Utf8PathBuf> {
    let store = HashStore::discover().ok()?;
    Utf8PathBuf::from_path_buf(store.dir().to_path_buf()).ok()
}

/// Whether a text table's path is kept.
type KeepPath = Box<dyn Fn(&str) -> bool>;

/// Builds [`Names`] from the sources a caller chooses. Nothing is looked for on the machine
/// unless it is given here.
pub struct NamesBuilder {
    mimir: Option<Utf8PathBuf>,
    text_tables: Vec<Utf8PathBuf>,
    keep_path: KeepPath,
    name_hash_index: Option<Utf8PathBuf>,
}

impl NamesBuilder {
    /// Reads the hash cache in this directory. [`mimir_cache_dir`] gives the one on this
    /// machine.
    pub fn mimir(mut self, dir: impl Into<Utf8PathBuf>) -> Self {
        self.mimir = Some(dir.into());
        self
    }

    /// Reads the CommunityDragon text tables in this directory, after those of the directories
    /// given before it.
    pub fn text_tables(mut self, dir: impl Into<Utf8PathBuf>) -> Self {
        self.text_tables.push(dir.into());
        self
    }

    /// Chooses the paths kept from the text tables. The default keeps the paths that hold
    /// `characters/`.
    pub fn keep_text_paths(mut self, keep: impl Fn(&str) -> bool + 'static) -> Self {
        self.keep_path = Box::new(keep);
        self
    }

    /// Keeps the name-hash index of the hash cache's paths in this directory. It is built there
    /// when the directory has none, or one of another version of the cache.
    pub fn name_hash_index(mut self, dir: impl Into<Utf8PathBuf>) -> Self {
        self.name_hash_index = Some(dir.into());
        self
    }

    /// Opens the sources. A table of the hash cache that does not open is no error: it is
    /// listed for [`Names::report`].
    pub fn build(self) -> Result<Names, NamesError> {
        let _span = tracing::info_span!("names").entered();
        let mut names = Names::default();
        if let Some(dir) = &self.mimir {
            names.open_mimir(dir);
        }
        for dir in &self.text_tables {
            names.read_text_tables(dir, &self.keep_path)?;
        }
        if let (Some(dir), Some(db), Some(stamp)) = (
            &self.name_hash_index,
            names.mimir(NameTable::Paths),
            &names.mimir_stamp,
        ) {
            names.name_hash = Some(NameHashIndex::open_or_build(dir, stamp, db)?);
        }
        Ok(names)
    }
}

/// The name tables. They do not change once built: what a run learns is in its
/// [`NameBook`](super::NameBook), and a [`Speller`](super::Speller) reads both.
#[derive(Default)]
pub struct Names {
    /// By [`NameTable::slot`].
    mimir: [Option<HashDb>; 5],
    /// By [`NameTable::slot`], each keyed by the hash widened to 64 bits.
    text: [HashMap<u64, String>; 5],
    /// The file name of the cache's path table: its version, and the name-hash index's stamp.
    mimir_stamp: Option<String>,
    /// The cache's paths by name hash, on disk.
    name_hash: Option<NameHashIndex>,
    /// The text tables' paths by name hash, and the cache's too when no index is on disk. Made
    /// the first time a path is asked for by name hash.
    by_name_hash: OnceLock<HashMap<BinHash, NameHashSlot>>,
    sources: Vec<NameSource>,
    absent: Vec<AbsentTable>,
}

impl Names {
    /// A builder with no source chosen.
    pub fn builder() -> NamesBuilder {
        NamesBuilder {
            mimir: None,
            text_tables: Vec::new(),
            // The full table is millions of lines, and a run asks for the paths of characters.
            keep_path: Box::new(|path| path.contains("characters/")),
            name_hash_index: None,
        }
    }

    fn open_mimir(&mut self, dir: &Utf8Path) {
        let store = HashStore::at(dir);
        let mut opened = Vec::new();
        for table in NameTable::ALL {
            match store.open_shared(table.mimir()) {
                Ok(db) => {
                    self.mimir[table.slot()] = Some(db);
                    opened.push(table);
                }
                Err(error) => self.absent.push(AbsentTable {
                    table,
                    reason: super::error_chain(&error),
                }),
            }
        }
        if self.mimir(NameTable::Paths).is_some() {
            self.mimir_stamp = store
                .path_for(Table::Game)
                .ok()
                .and_then(|file| Some(file.file_name()?.to_str()?.to_string()));
        }
        tracing::debug!(%dir, opened = opened.len(), "hash cache");
        if !opened.is_empty() {
            self.sources.push(NameSource::Mimir {
                dir: dir.to_owned(),
                tables: opened,
            });
        }
    }

    fn read_text_tables(&mut self, dir: &Utf8Path, keep_path: &KeepPath) -> Result<(), NamesError> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir)? {
            files.push(entry?.path());
        }
        files.sort();
        let mut read = 0;
        for file in files {
            let table = file
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| NameTable::of_text_file(&name.to_ascii_lowercase()));
            let Some(table) = table else { continue };
            let bytes = fs::read(&file)?;
            let names = &mut self.text[table.slot()];
            for line in String::from_utf8_lossy(&bytes).lines() {
                let Some((hex, name)) = line.split_once(' ') else {
                    continue;
                };
                let Ok(hash) = u64::from_str_radix(hex, 16) else {
                    continue;
                };
                let name = name.trim_end();
                if table == NameTable::Paths && !keep_path(name) {
                    continue;
                }
                // A line that spells a name under another hash is dropped here, so a later
                // line with the right name is still taken.
                if table.hash_of(name) == hash {
                    names.entry(hash).or_insert_with(|| name.to_string());
                }
            }
            read += 1;
        }
        tracing::debug!(%dir, files = read, "text tables");
        if read > 0 {
            self.sources.push(NameSource::TextTables {
                dir: dir.to_owned(),
                files: read,
            });
        }
        Ok(())
    }

    fn mimir(&self, table: NameTable) -> Option<&HashDb> {
        self.mimir[table.slot()].as_ref()
    }

    /// The name a table gives a hash: the hash cache's, else the text tables'.
    pub(super) fn spell(&self, table: NameTable, hash: u64) -> Option<String> {
        let cached = self
            .mimir(table)
            .and_then(|db| db.get(hash))
            .filter(|name| table.hash_of(name.as_str()) == hash)
            .map(|name| name.into_owned());
        cached.or_else(|| self.text[table.slot()].get(&hash).cloned())
    }

    /// The path a chunk hash is the hash of.
    pub fn path(&self, hash: WadHash) -> Option<String> {
        self.spell(NameTable::Paths, hash.0)
    }

    /// The entry name an entry hash is the hash of.
    pub fn entry(&self, hash: BinHash) -> Option<String> {
        self.spell(NameTable::Entries, u64::from(hash.0))
    }

    /// The field name a field hash is the hash of.
    pub fn field(&self, hash: BinHash) -> Option<String> {
        self.spell(NameTable::Fields, u64::from(hash.0))
    }

    /// The class name a class hash is the hash of.
    pub fn class(&self, hash: BinHash) -> Option<String> {
        self.spell(NameTable::Classes, u64::from(hash.0))
    }

    /// The plaintext behind a hash value or a hashed map key.
    pub fn hash(&self, hash: BinHash) -> Option<String> {
        self.spell(NameTable::Hashes, u64::from(hash.0))
    }

    /// Every path the tables spell that starts with `prefix`, which is lowercase.
    pub fn paths_with_prefix(&self, prefix: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        if let Some(db) = self.mimir(NameTable::Paths) {
            for (hash, path) in db.prefix(prefix) {
                if NameTable::Paths.hash_of(path.as_str()) == hash {
                    out.insert(path.into_owned());
                }
            }
        }
        let text = self.text[NameTable::Paths.slot()].values();
        out.extend(text.filter(|path| path.starts_with(prefix)).cloned());
        out
    }

    /// The characters whose name starts with a champion's: the directories under
    /// `data/characters/` that the tables spell, the champion's own included.
    pub fn characters_like(&self, champion: &str) -> BTreeSet<String> {
        const CHARACTERS: &str = "data/characters/";
        let prefix = format!("{CHARACTERS}{}", champion.to_ascii_lowercase());
        self.paths_with_prefix(&prefix)
            .iter()
            .filter_map(|path| path.strip_prefix(CHARACTERS)?.split('/').next())
            .filter(|character| !character.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// The one path, of every path the tables spell, whose name hash is `hash`: the FNV-1a hash
    /// an entry name has. `None` when no path has it or several have.
    pub fn path_by_name_hash(&self, hash: BinHash) -> Result<Option<String>, NameHashError> {
        let on_disk = match &self.name_hash {
            Some(index) => index.get(hash)?,
            None => None,
        };
        let in_memory = self.by_name_hash().get(&hash).copied();
        let slot = match (on_disk, in_memory) {
            (Some(a), Some(b)) => Some(a.merge(b)),
            (a, b) => a.or(b),
        };
        let Some(NameHashSlot::One(path)) = slot else {
            return Ok(None);
        };
        Ok(self.path(path).filter(|name| name_hash(name) == hash))
    }

    fn by_name_hash(&self) -> &HashMap<BinHash, NameHashSlot> {
        self.by_name_hash.get_or_init(|| {
            let mut index: HashMap<BinHash, NameHashSlot> = HashMap::new();
            let mut add = |path: &str, hash: u64| {
                let slot = NameHashSlot::One(WadHash(hash));
                index
                    .entry(name_hash(path))
                    .and_modify(|held| *held = held.merge(slot))
                    .or_insert(slot);
            };
            if let (None, Some(db)) = (&self.name_hash, self.mimir(NameTable::Paths)) {
                for (hash, path) in db.iter() {
                    add(path.as_str(), hash);
                }
            }
            for (hash, path) in &self.text[NameTable::Paths.slot()] {
                add(path, *hash);
            }
            index
        })
    }

    /// Whether the hash cache's path table opened.
    pub fn has_mimir(&self) -> bool {
        self.mimir(NameTable::Paths).is_some()
    }

    /// The sources that loaded, in the order they are asked.
    pub fn sources(&self) -> &[NameSource] {
        &self.sources
    }

    /// Raises what building the tables found: the tables of the hash cache that did not open at
    /// the directory given.
    pub fn report(&self, sink: &mut Sink<'_>) {
        if !self.absent.is_empty() {
            sink.raise(MimirAbsent {
                tables: self.absent.clone(),
            });
        }
    }
}

impl fmt::Debug for Names {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Names")
            .field("sources", &self.sources)
            .field("absent", &self.absent)
            .finish()
    }
}
