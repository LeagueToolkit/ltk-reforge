//! The history index asked from the History page (#ADR-0091): a command per question, each the
//! library's `History` answer with its hashes spelled through the hash tables. The page adds no
//! rule (#ADR-0046). `docs/design/ROADMAP-UX.md`, "The History page".

use reforge::env::Env;
use reforge::game::{NameBook, Names, Speller};
use reforge::history::{History, HistoryError, Life, Position};
use reforge::model::bin::read_stored;
use reforge::model::dump::dump_object;
use reforge::model::{BinHash, WadHash, clean, name_hash, path_hash};
use serde::Serialize;

use crate::error::{AppError, chain};

/// The most objects naming a path a lookup answers: a page's table, and the rest said to be
/// left out.
pub const MAX_NAMED_BY: usize = 256;

/// The most paths a suggestion answers.
pub const MAX_SUGGESTED: usize = 64;

/// The fewest characters a prefix has before paths are suggested for it.
pub const MIN_PREFIX: usize = 3;

/// A build of the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BuildView {
    /// Its position: its number in the order the builds arrived.
    pub position: u16,
    /// Its version, `16.19.8207193`.
    pub version: String,
    /// The commit of the census history that holds it.
    pub commit: String,
}

/// The builds of the history, in position order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct HistoryBuilds {
    /// The position of the build appended last, which is not always the newest version.
    pub head: u16,
    /// Every build, in position order.
    pub builds: Vec<BuildView>,
}

/// The builds a key was present in, from the first to the last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct LifeView {
    /// The first build.
    pub first: BuildView,
    /// The last build.
    pub last: BuildView,
    /// How many positions the life spans, both ends counted.
    pub builds: usize,
}

/// A file a WAD of the game held at the path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct PlacedView {
    /// The WAD's path in the manifest.
    pub wad: String,
    /// The file's key, the head of its SHA-256, as 32 hex digits.
    pub content: String,
    /// The builds the WAD held the file there without a break.
    pub life: LifeView,
}

/// A game object that named the path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct NamedByView {
    /// The path of the object's bin, spelled, else its hash.
    pub bin: String,
    /// The bin's path hash, as sixteen hex digits.
    pub bin_hash: String,
    /// The object's entry name, spelled, else its hash.
    pub entry: String,
    /// The entry hash, as eight hex digits.
    pub entry_hash: String,
    /// The builds the object named the path.
    pub life: LifeView,
}

/// A bin a bin links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct LinkView {
    /// Its path, spelled, else its hash.
    pub path: String,
    /// Its path hash, as sixteen hex digits.
    pub hash: String,
}

/// An object of a bin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct EntryView {
    /// Its entry name, spelled, else its hash.
    pub entry: String,
    /// Its entry hash, as eight hex digits.
    pub hash: String,
    /// Its class, spelled, else its hash.
    pub class: String,
}

/// The bin the game held at the path at the build asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BinLookup {
    /// The WAD that held it: the first in the census history's order.
    pub wad: String,
    /// The builds that WAD held the bin there without a break.
    pub life: LifeView,
    /// The bins it links, in the order the census history lists them.
    pub links: Vec<LinkView>,
    /// Its objects, by entry name and then hash.
    pub entries: Vec<EntryView>,
}

/// A path looked up in the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct LookupView {
    /// The path, spelled through the tables, else as asked, else its hash.
    pub path: String,
    /// Its path hash, as sixteen hex digits.
    pub hash: String,
    /// The builds some WAD held a file at the path. None when the game never held one.
    pub life: Option<LifeView>,
    /// The kind of the first file seen there, as the census history spells it.
    pub kind: Option<String>,
    /// The build the rest is answered at.
    pub at: BuildView,
    /// The files the game's WADs held at the path at that build.
    pub placed: Vec<PlacedView>,
    /// The game objects that named the path, oldest first, up to [`MAX_NAMED_BY`].
    pub named_by: Vec<NamedByView>,
    /// Whether more objects named the path than `named_by` holds.
    pub named_by_capped: bool,
    /// The bin at the path at that build. None when the file there is no bin, or none is.
    pub bin: Option<BinLookup>,
}

/// A path as the page takes it: its text, or its path hash as sixteen hex digits with or
/// without `0x`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathQuery {
    /// A path, lowercased.
    Path(String),
    /// A path hash.
    Hash(WadHash),
}

impl PathQuery {
    /// The query a text asks.
    pub fn parse(text: &str) -> Self {
        let text = text.trim();
        match hex_digits(text, 16) {
            Some(hash) => PathQuery::Hash(WadHash(hash)),
            None => PathQuery::Path(text.to_ascii_lowercase()),
        }
    }

    /// The path hash asked about.
    pub fn hash(&self) -> WadHash {
        match self {
            PathQuery::Path(path) => path_hash(path),
            PathQuery::Hash(hash) => *hash,
        }
    }
}

/// An entry as the page takes it: its name, or its entry hash as eight hex digits with or
/// without `0x`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryQuery {
    /// An entry name.
    Name(String),
    /// An entry hash.
    Hash(BinHash),
}

impl EntryQuery {
    /// The query a text asks.
    pub fn parse(text: &str) -> Self {
        let text = text.trim();
        let hash = hex_digits(text, 8).and_then(|hash| u32::try_from(hash).ok());
        match hash {
            Some(hash) => EntryQuery::Hash(BinHash(hash)),
            None => EntryQuery::Name(text.to_string()),
        }
    }

    /// The entry hash asked about.
    pub fn hash(&self) -> BinHash {
        match self {
            EntryQuery::Name(name) => name_hash(name),
            EntryQuery::Hash(hash) => *hash,
        }
    }
}

/// A game object at a build, as text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct HistoryObjectView {
    /// The object as text, its hashes spelled where the tables spell them.
    pub text: String,
    /// Its class, spelled, else its hash.
    pub class: String,
    /// The WAD whose bin held it: the first in the census history's order.
    pub wad: String,
    /// The builds that WAD's bin held the object's text without a break.
    pub life: LifeView,
}

/// A number written as exactly `digits` hex digits, with or without `0x`.
fn hex_digits(text: &str, digits: usize) -> Option<u64> {
    let bare = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    let all_hex = bare.len() == digits && bare.bytes().all(|b| b.is_ascii_hexdigit());
    all_hex
        .then(|| u64::from_str_radix(bare, 16).ok())
        .flatten()
}

fn unanswered(error: HistoryError) -> AppError {
    AppError::Internal {
        detail: chain(&error),
    }
}

/// The questions the History page asks of a history, and the tables its hashes are spelled
/// with.
pub struct Lookup<'a> {
    history: &'a dyn History,
    names: &'a Names,
}

impl<'a> Lookup<'a> {
    /// The questions over a history and the tables. Refused with no history.
    pub fn new(history: &'a dyn History, names: &'a Names) -> Result<Self, AppError> {
        if history.stamp().is_none() {
            return Err(AppError::NoIndex);
        }
        Ok(Self { history, names })
    }

    /// The questions over an environment's history and tables.
    pub fn of(env: &'a Env) -> Result<Self, AppError> {
        Self::new(env.history(), env.names())
    }

    /// The builds, and the head.
    pub fn builds(&self) -> HistoryBuilds {
        let builds = self.history.builds();
        HistoryBuilds {
            head: self.history.head().map_or(0, |head| head.0),
            builds: builds.iter().map(|row| self.build(row.at)).collect(),
        }
    }

    /// A path looked up: its life, and at the build at `at`, the head when none is given,
    /// the WADs that held it, the objects that named it and the bin it was.
    pub fn lookup(&self, query: &str, at: Option<u16>) -> Result<LookupView, AppError> {
        let query = PathQuery::parse(query);
        let hash = query.hash();
        let at = self.position(at)?;
        let hit = self.history.path(hash).map_err(unanswered)?;
        let placed = self.history.placed(hash, at).map_err(unanswered)?;
        let named_by = self.history.named_by(hash).map_err(unanswered)?;
        let bin = self.history.bin(hash, at).map_err(unanswered)?;
        let path = match (self.names.path(hash), &query) {
            (Some(spelled), _) => clean(&spelled).into_owned(),
            (None, PathQuery::Path(asked)) => clean(asked).into_owned(),
            (None, PathQuery::Hash(_)) => hex16(hash),
        };
        Ok(LookupView {
            path,
            hash: hex16(hash),
            life: hit.map(|hit| self.life(hit.life)),
            kind: hit.map(|hit| hit.kind.name().to_string()),
            at: self.build(at),
            placed: placed
                .into_iter()
                .map(|held| PlacedView {
                    wad: clean(&held.wad).into_owned(),
                    content: held.key.to_string(),
                    life: self.life(held.life),
                })
                .collect(),
            named_by_capped: named_by.len() > MAX_NAMED_BY,
            named_by: named_by
                .into_iter()
                .take(MAX_NAMED_BY)
                .map(|named| NamedByView {
                    bin: self.path_name(named.bin),
                    bin_hash: hex16(named.bin),
                    entry: self.entry_name(named.entry),
                    entry_hash: hex8(named.entry),
                    life: self.life(named.life),
                })
                .collect(),
            bin: bin.map(|bin| {
                let mut entries: Vec<EntryView> = bin
                    .entries
                    .iter()
                    .map(|object| EntryView {
                        entry: self.entry_name(object.entry),
                        hash: hex8(object.entry),
                        class: self.class_name(object.class),
                    })
                    .collect();
                entries.sort_by(|a, b| a.entry.cmp(&b.entry).then_with(|| a.hash.cmp(&b.hash)));
                BinLookup {
                    wad: clean(&bin.wad).into_owned(),
                    life: self.life(bin.life),
                    links: bin
                        .links
                        .iter()
                        .map(|&link| LinkView {
                            path: self.path_name(link),
                            hash: hex16(link),
                        })
                        .collect(),
                    entries,
                }
            }),
        })
    }

    /// The paths the tables spell that start with a prefix, lowercased, in path order, up to
    /// [`MAX_SUGGESTED`]. None for a prefix of fewer than [`MIN_PREFIX`] characters.
    pub fn suggest(&self, prefix: &str) -> Vec<String> {
        let prefix = prefix.trim().to_ascii_lowercase();
        if prefix.chars().count() < MIN_PREFIX {
            return Vec::new();
        }
        let paths = self.names.first_paths_with_prefix(&prefix, MAX_SUGGESTED);
        paths.iter().map(|path| clean(path).into_owned()).collect()
    }

    /// The object `entry`, a name or eight hex digits, of the bin at `bin`, a path or sixteen
    /// hex digits, at the build at position `at`, as text with its names spelled. None when
    /// the build holds no such object, or the index holds no bytes of it.
    pub fn object(
        &self,
        bin: &str,
        entry: &str,
        at: u16,
    ) -> Result<Option<HistoryObjectView>, AppError> {
        let bin = PathQuery::parse(bin).hash();
        let entry = EntryQuery::parse(entry).hash();
        let at = self.position(Some(at))?;
        let Some(stored) = self
            .history
            .object_bytes(bin, entry, at)
            .map_err(unanswered)?
        else {
            return Ok(None);
        };
        let object = read_stored(stored.class, &stored.bytes).map_err(|error| {
            AppError::HistoryObjectUnreadable {
                bin: self.path_name(bin),
                entry: self.entry_name(entry),
                detail: error.to_string(),
            }
        })?;
        let book = NameBook::new();
        let speller = Speller::new(&book, self.names);
        Ok(Some(HistoryObjectView {
            text: dump_object(&object, &speller),
            class: self.class_name(stored.class),
            wad: clean(&stored.wad).into_owned(),
            life: self.life(stored.life),
        }))
    }

    /// The position asked for, the head when none is. Refused for a position no build has.
    fn position(&self, at: Option<u16>) -> Result<Position, AppError> {
        match at.map(Position).or_else(|| self.history.head()) {
            Some(at) if self.history.build(at).is_some() => Ok(at),
            asked => Err(AppError::Internal {
                detail: format!("the history holds no build at position {asked:?}"),
            }),
        }
    }

    /// The build at a position, `#<n>` for a position with no build.
    fn build(&self, at: Position) -> BuildView {
        match self.history.build(at) {
            Some(row) => BuildView {
                position: at.0,
                version: row.build.to_string(),
                commit: row.commit.clone(),
            },
            None => BuildView {
                position: at.0,
                version: format!("#{at}"),
                commit: String::new(),
            },
        }
    }

    fn life(&self, life: Life) -> LifeView {
        LifeView {
            first: self.build(life.first),
            last: self.build(life.last),
            builds: usize::from(life.last.0.saturating_sub(life.first.0)) + 1,
        }
    }

    fn path_name(&self, hash: WadHash) -> String {
        self.names
            .path(hash)
            .map_or_else(|| hex16(hash), |path| clean(&path).into_owned())
    }

    fn entry_name(&self, hash: BinHash) -> String {
        self.names
            .entry(hash)
            .map_or_else(|| hex8(hash), |name| clean(&name).into_owned())
    }

    fn class_name(&self, hash: BinHash) -> String {
        self.names
            .class(hash)
            .map_or_else(|| hex8(hash), |name| clean(&name).into_owned())
    }
}

fn hex16(hash: WadHash) -> String {
    format!("{:016x}", hash.0)
}

fn hex8(hash: BinHash) -> String {
    format!("{:08x}", hash.0)
}
