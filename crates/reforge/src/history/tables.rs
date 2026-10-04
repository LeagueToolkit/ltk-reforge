//! The tables of the index: for each, its columns, the typed key a row is found by, and how a
//! row's fields fill the columns. The writer (`reforge-census`) creates and fills the tables
//! from these definitions alone, whole or by an append, so a column is spelled here and in the
//! reader's queries (`sqlite.rs`) and nowhere else.
//!
//! One SQLite file answers what fixed-width tables could not without a new format each time:
//! every content a path held, a range of keys, joins between families, and queries from
//! scripts. A hash is stored as an integer (the `u64` bit pattern as `i64`), a file's and an
//! object's key as a 16-byte blob. SQLite stores a string in every row that holds it, so the
//! names a rig repeats in every version and every skin are interned in `names`.
//!
//! Every table of lives keeps `first`, `last` and `alive`: the life, and how many records hold
//! the key at the head (none once it is gone), which an append starts from.

use std::fmt;
use std::hash::Hash;
use std::str::FromStr;
use std::sync::Arc;

use reforge_wwise::Object;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::HistoryError;
pub use crate::model::EventsBank;
use crate::model::{BinHash, ClipKey, FileKey, NormKey, ObjectKey, WadHash};

/// A build's number in the history, from 0, in the order the builds arrived.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Position(pub u16);

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The first and the last build a key was present in. A key that goes and comes back keeps the
/// hull.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Life {
    /// The first build the key was present in.
    pub first: Position,
    /// The last build the key was present in.
    pub last: Position,
}

impl Life {
    /// The life from one position to another.
    pub const fn new(first: u16, last: u16) -> Self {
        Self {
            first: Position(first),
            last: Position(last),
        }
    }
}

/// A group of tables the walk fills together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    /// Contents, paths, and every path a content lived at.
    Files,
    /// Objects, and from their texts the files they name, their audio tags and bank units.
    Objects,
    /// Clips, the events that play them, and the events banks' headers and objects.
    Clips,
    /// Skeletons, meshes, and the joints animation clips animate.
    Rigs,
    /// Textures.
    Textures,
}

impl Family {
    /// Every family.
    pub const ALL: [Family; 5] = [
        Family::Files,
        Family::Objects,
        Family::Clips,
        Family::Rigs,
        Family::Textures,
    ];

    /// The family's name in a stamp and on a command line.
    pub fn as_str(self) -> &'static str {
        match self {
            Family::Files => "files",
            Family::Objects => "objects",
            Family::Clips => "clips",
            Family::Rigs => "rigs",
            Family::Textures => "textures",
        }
    }

    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Family {
    type Err = HistoryError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Family::ALL
            .into_iter()
            .find(|family| family.as_str() == text)
            .ok_or_else(|| HistoryError::UnknownFamily(text.to_string()))
    }
}

/// The families an index holds. A table of a family it does not hold is empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "Vec<Family>", into = "Vec<Family>")]
pub struct Families(u8);

impl Families {
    /// Every family.
    pub const ALL: Families = Families(0b1_1111);
    /// No family.
    pub const NONE: Families = Families(0);

    /// Whether the family is among these.
    pub fn has(self, family: Family) -> bool {
        self.0 & family.bit() != 0
    }

    /// These families and one more.
    #[must_use]
    pub fn with(self, family: Family) -> Self {
        Self(self.0 | family.bit())
    }

    /// The families, in their fixed order.
    pub fn iter(self) -> impl Iterator<Item = Family> {
        Family::ALL.into_iter().filter(move |f| self.has(*f))
    }

    /// The families a comma-separated list names, `files,clips`.
    pub fn parse(text: &str) -> Result<Self, HistoryError> {
        text.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(Family::from_str)
            .collect()
    }
}

impl FromIterator<Family> for Families {
    fn from_iter<I: IntoIterator<Item = Family>>(families: I) -> Self {
        families.into_iter().fold(Families::NONE, Families::with)
    }
}

impl From<Vec<Family>> for Families {
    fn from(families: Vec<Family>) -> Self {
        families.into_iter().collect()
    }
}

impl From<Families> for Vec<Family> {
    fn from(families: Families) -> Self {
        families.iter().collect()
    }
}

/// What a file's bytes are by their magic, as the census history spells it. The index stores
/// the kind's place in a fixed list, so the list only grows at its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct FileKind(pub u8);

impl FileKind {
    const NAMES: &'static [&'static str] = &[
        "-",
        "bin",
        "tex",
        "dds",
        "skn",
        "skl",
        "anm",
        "scb",
        "sco",
        "bnk",
        "wpk",
        "png",
        "jpg",
        "tga",
        "wgeo",
        "mapgeo",
        "lua",
        "preload",
        "stringtable",
        "luaobj",
        "lightgrid",
        "svg",
        "inibin",
        "link",
        "wad",
        "exe",
        "riff",
        "zip",
        "ogg",
    ];

    /// A kind the list does not hold.
    pub const OTHER: FileKind = FileKind(255);

    /// The kind the census history spells `name`. An entry with no kind is `-`.
    pub fn of(name: &str) -> Self {
        let name = if name.is_empty() { "-" } else { name };
        Self::NAMES
            .iter()
            .position(|known| *known == name)
            .and_then(|at| u8::try_from(at).ok())
            .map_or(Self::OTHER, FileKind)
    }

    /// The kind as the census history spells it, `?` for one the list does not hold.
    pub fn name(self) -> &'static str {
        Self::NAMES.get(usize::from(self.0)).copied().unwrap_or("?")
    }
}

/// A hash's 64 bits as SQLite stores them: the same bits, signed.
pub fn stored(bits: u64) -> i64 {
    bits as i64
}

/// The hash a stored integer holds.
pub fn loaded(stored: i64) -> u64 {
    stored as u64
}

/// How a column holds its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    /// An integer: a hash, an id, a count, a flag.
    Integer,
    /// A file's or an object's key, a 16-byte blob.
    Key,
    /// A name, as the id of its row in `names`.
    Name,
    /// A name or none, as the id of its row in `names` or null.
    OptionalName,
    /// Bytes as stored, a blob of any length.
    Bytes,
}

impl Storage {
    fn sql(self) -> &'static str {
        match self {
            Storage::Integer | Storage::Name => "INTEGER NOT NULL",
            Storage::Key | Storage::Bytes => "BLOB NOT NULL",
            Storage::OptionalName => "INTEGER",
        }
    }
}

/// One column of a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    /// The column's name.
    pub name: &'static str,
    /// How it holds its value.
    pub storage: Storage,
}

const fn integer(name: &'static str) -> Column {
    Column {
        name,
        storage: Storage::Integer,
    }
}

const fn key(name: &'static str) -> Column {
    Column {
        name,
        storage: Storage::Key,
    }
}

const fn name(name: &'static str) -> Column {
    Column {
        name,
        storage: Storage::Name,
    }
}

const fn bytes(name: &'static str) -> Column {
    Column {
        name,
        storage: Storage::Bytes,
    }
}

/// How a table's rows are found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The key columns are the primary key.
    Keyed,
    /// A row has an `id` of its own and is found by its key columns through a unique index
    /// made after the rows are in. The key's last column is the digest of the row's whole
    /// record, so a changed record at a path is a new row.
    Recorded,
}

/// The table that holds the ordered parts of another table's rows: a skeleton's joints, a
/// mesh's submeshes, a joint list's joints. Its key is the owner's id and the part's ordinal.
/// The owner's id is its row id: the `id` of a `Recorded` table, the key of a table keyed by
/// one integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartsDef {
    /// The table's name.
    pub name: &'static str,
    /// The column that holds the owner's id.
    pub owner: &'static str,
    /// The columns after the owner and the ordinal.
    pub columns: &'static [Column],
}

/// One table of lives: its name, its columns and its indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableDef {
    /// The table's name.
    pub name: &'static str,
    /// The family that fills it.
    pub family: Family,
    /// The columns a row is found by.
    pub key: &'static [Column],
    /// The columns a row takes from the record that started it.
    pub facts: &'static [Column],
    /// How its rows are found.
    pub shape: Shape,
    /// The columns besides the key that a question asks by, each indexed.
    pub lookups: &'static [&'static str],
    /// The table of its rows' parts, when they have parts.
    pub parts: Option<PartsDef>,
}

fn columns_sql(columns: &[Column], out: &mut Vec<String>) {
    out.extend(
        columns
            .iter()
            .map(|c| format!("{} {}", c.name, c.storage.sql())),
    );
}

impl TableDef {
    /// The key columns, then the facts.
    pub fn columns(&self) -> impl Iterator<Item = &Column> {
        self.key.iter().chain(self.facts)
    }

    /// The statements that create the table, and the table of its parts.
    pub fn create_sql(&self) -> String {
        let mut columns = Vec::new();
        columns_sql(self.key, &mut columns);
        columns_sql(self.facts, &mut columns);
        columns.extend(["first", "last", "alive"].map(|c| format!("{c} INTEGER NOT NULL")));
        let key: Vec<&str> = self.key.iter().map(|c| c.name).collect();
        let mut sql = match (self.shape, self.key) {
            (Shape::Recorded, _) => format!(
                "CREATE TABLE {} (id INTEGER PRIMARY KEY, {});\n",
                self.name,
                columns.join(", ")
            ),
            // One integer as the primary key is the row's id, with no index beside the table.
            (Shape::Keyed, [only]) if only.storage == Storage::Integer => {
                columns[0] = format!("{} INTEGER PRIMARY KEY", only.name);
                format!("CREATE TABLE {} ({});\n", self.name, columns.join(", "))
            }
            (Shape::Keyed, _) => format!(
                "CREATE TABLE {} ({}, PRIMARY KEY ({})) WITHOUT ROWID;\n",
                self.name,
                columns.join(", "),
                key.join(", ")
            ),
        };
        if let Some(parts) = &self.parts {
            let mut columns = vec![
                format!("{} INTEGER NOT NULL", parts.owner),
                "ordinal INTEGER NOT NULL".to_string(),
            ];
            columns_sql(parts.columns, &mut columns);
            sql.push_str(&format!(
                "CREATE TABLE {} ({}, PRIMARY KEY ({}, ordinal)) WITHOUT ROWID;\n",
                parts.name,
                columns.join(", "),
                parts.owner
            ));
        }
        sql
    }

    /// The statements that index the table.
    pub fn index_sql(&self) -> String {
        let mut sql = String::new();
        if self.shape == Shape::Recorded {
            let key: Vec<&str> = self.key.iter().map(|c| c.name).collect();
            sql.push_str(&format!(
                "CREATE UNIQUE INDEX {0}_key ON {0} ({1});\n",
                self.name,
                key.join(", ")
            ));
        }
        for column in self.lookups {
            sql.push_str(&format!(
                "CREATE INDEX {0}_{1} ON {0} ({1});\n",
                self.name, column
            ));
        }
        sql
    }
}

/// The table every name is held in once: a joint, a material, a texture format, a vertex type,
/// an audio tag, a bank unit.
pub const NAMES_SQL: &str =
    "CREATE TABLE names (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);\n";

/// The statements that create every table of an empty index.
pub fn schema_sql() -> String {
    let mut sql: String = TABLES.iter().map(|table| table.create_sql()).collect();
    sql.push_str(NAMES_SQL);
    sql
}

/// The statements that index every table. A whole write runs them after the rows are in, which
/// is faster than keeping the indexes up to date per insert.
pub fn index_sql() -> String {
    TABLES.iter().map(|table| table.index_sql()).collect()
}

/// One value of a row, as its column holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell<'a> {
    /// An integer.
    Integer(i64),
    /// A 16-byte key.
    Key([u8; 16]),
    /// A name, which the writer interns.
    Name(&'a str),
    /// No name.
    NoName,
    /// Bytes as stored.
    Bytes(&'a [u8]),
}

impl From<u64> for Cell<'_> {
    fn from(bits: u64) -> Self {
        Cell::Integer(stored(bits))
    }
}

impl From<u32> for Cell<'_> {
    fn from(n: u32) -> Self {
        Cell::Integer(i64::from(n))
    }
}

impl From<bool> for Cell<'_> {
    fn from(flag: bool) -> Self {
        Cell::Integer(i64::from(flag))
    }
}

impl From<WadHash> for Cell<'_> {
    fn from(hash: WadHash) -> Self {
        hash.0.into()
    }
}

impl From<BinHash> for Cell<'_> {
    fn from(hash: BinHash) -> Self {
        hash.0.into()
    }
}

impl From<FileKind> for Cell<'_> {
    fn from(kind: FileKind) -> Self {
        Cell::Integer(i64::from(kind.0))
    }
}

impl From<FileKey> for Cell<'_> {
    fn from(key: FileKey) -> Self {
        Cell::Key(key.0)
    }
}

impl From<ObjectKey> for Cell<'_> {
    fn from(key: ObjectKey) -> Self {
        Cell::Key(key.0)
    }
}

impl From<NormKey> for Cell<'_> {
    fn from(key: NormKey) -> Self {
        key.0.into()
    }
}

impl From<ClipKey> for Cell<'_> {
    fn from(key: ClipKey) -> Self {
        key.0.into()
    }
}

/// A table of lives, as a type: the key its rows are found by, what a row keeps of the record
/// that started it, and the cells both make.
pub trait Table: 'static {
    /// What a row is found by.
    type Key: Clone + Eq + Ord + Hash + fmt::Debug + Send + Sync;
    /// What a row keeps of the record that started it.
    type Facts: Clone + Send + Sync;
    /// The table's columns.
    const DEF: TableDef;

    /// A row's cells, in the order of `DEF`'s key columns and then its facts.
    fn cells<'a>(key: &'a Self::Key, facts: &'a Self::Facts) -> impl Iterator<Item = Cell<'a>>;

    /// The row's parts in order, each as the cells of `DEF.parts`' columns.
    fn parts(key: &Self::Key) -> Vec<Vec<Cell<'_>>> {
        let _ = key;
        Vec::new()
    }
}

/// One row of a table: a key, its facts, its life and how many records hold it at the head.
pub struct Row<T: Table> {
    /// What the row is found by.
    pub key: T::Key,
    /// What the row keeps of the record that started it.
    pub facts: T::Facts,
    /// The builds the key was present in.
    pub life: Life,
    /// How many records hold the key at the head, none once it is gone.
    pub alive: u32,
}

/// What a row of `files` keeps: the first file seen with the content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileFacts {
    /// What the bytes are by their magic.
    pub kind: FileKind,
    /// The first path the content was seen under.
    pub path: WadHash,
}

/// `files`: every content the game shipped.
pub struct Files;

impl Table for Files {
    type Key = FileKey;
    type Facts = FileFacts;
    const DEF: TableDef = TableDef {
        name: "files",
        family: Family::Files,
        key: &[key("key")],
        facts: &[integer("kind"), integer("path")],
        shape: Shape::Keyed,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a FileKey, facts: &'a FileFacts) -> impl Iterator<Item = Cell<'a>> {
        [(*key).into(), facts.kind.into(), facts.path.into()].into_iter()
    }
}

/// `paths`: every path some WAD of the game held a file at, with the kind of the first file
/// seen there.
pub struct Paths;

impl Table for Paths {
    type Key = WadHash;
    type Facts = FileKind;
    const DEF: TableDef = TableDef {
        name: "paths",
        family: Family::Files,
        key: &[integer("path")],
        facts: &[integer("kind")],
        shape: Shape::Keyed,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a WadHash, facts: &'a FileKind) -> impl Iterator<Item = Cell<'a>> {
        [(*key).into(), (*facts).into()].into_iter()
    }
}

/// A content at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Held {
    /// The path.
    pub path: WadHash,
    /// The content.
    pub key: FileKey,
}

/// `contents`: every path a content lived at, where `files` keeps only the first. Indexed by
/// content too, so every path of a content is one query.
pub struct Contents;

impl Table for Contents {
    type Key = Held;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "contents",
        family: Family::Files,
        key: &[integer("path"), key("key")],
        facts: &[],
        shape: Shape::Keyed,
        lookups: &["key"],
        parts: None,
    };

    fn cells<'a>(key: &'a Held, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [key.path.into(), key.key.into()].into_iter()
    }
}

/// What a row of `objects` keeps: the first place the object was seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectFacts {
    /// The object's entry hash.
    pub entry: BinHash,
    /// The object's class.
    pub class: BinHash,
    /// The path of the first bin the object was seen in.
    pub bin: WadHash,
    /// The object's norm key.
    pub norm: NormKey,
}

/// `objects`: every object the game shipped, by its key. Indexed by norm key too.
pub struct Objects;

impl Table for Objects {
    type Key = ObjectKey;
    type Facts = ObjectFacts;
    const DEF: TableDef = TableDef {
        name: "objects",
        family: Family::Objects,
        key: &[key("key")],
        facts: &[
            integer("entry"),
            integer("class"),
            integer("bin"),
            integer("norm"),
        ],
        shape: Shape::Keyed,
        lookups: &["norm"],
        parts: None,
    };

    fn cells<'a>(key: &'a ObjectKey, facts: &'a ObjectFacts) -> impl Iterator<Item = Cell<'a>> {
        [
            (*key).into(),
            facts.entry.into(),
            facts.class.into(),
            facts.bin.into(),
            facts.norm.into(),
        ]
        .into_iter()
    }
}

/// A path an object names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Naming {
    /// The path named.
    pub path: WadHash,
    /// The path of the object's bin.
    pub bin: WadHash,
    /// The object's entry hash.
    pub entry: BinHash,
}

/// `refs`: which game objects named a path, from their texts.
pub struct Refs;

impl Table for Refs {
    type Key = Naming;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "refs",
        family: Family::Objects,
        key: &[integer("path"), integer("bin"), integer("entry")],
        facts: &[],
        shape: Shape::Keyed,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a Naming, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [key.path.into(), key.bin.into(), key.entry.into()].into_iter()
    }
}

/// An audio tag of an object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag {
    /// The path of the object's bin.
    pub bin: WadHash,
    /// The object's entry hash.
    pub entry: BinHash,
    /// The tag.
    pub tag: String,
}

/// `tags`: the audio tags of a game object's `tagEventList`.
pub struct Tags;

impl Table for Tags {
    type Key = Tag;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "tags",
        family: Family::Objects,
        key: &[integer("bin"), integer("entry"), name("tag")],
        facts: &[],
        shape: Shape::Keyed,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a Tag, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [key.bin.into(), key.entry.into(), Cell::Name(&key.tag)].into_iter()
    }
}

/// A bank unit of an object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Unit {
    /// The path of the object's bin.
    pub bin: WadHash,
    /// The object's entry hash.
    pub entry: BinHash,
    /// The unit's name.
    pub unit: String,
    /// Whether the unit is voice.
    pub voice: bool,
}

/// `units`: the bank units of a game object.
pub struct Units;

impl Table for Units {
    type Key = Unit;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "units",
        family: Family::Objects,
        key: &[
            integer("bin"),
            integer("entry"),
            name("unit"),
            integer("voice"),
        ],
        facts: &[],
        shape: Shape::Keyed,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a Unit, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [
            key.bin.into(),
            key.entry.into(),
            Cell::Name(&key.unit),
            key.voice.into(),
        ]
        .into_iter()
    }
}

/// A bank a bank unit loads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitBank {
    /// The path of the object's bin.
    pub bin: WadHash,
    /// The object's entry hash.
    pub entry: BinHash,
    /// The unit's name.
    pub unit: String,
    /// The bank's path.
    pub bank: WadHash,
}

/// `unit_banks`: the banks a game unit loads. Indexed by bank too.
pub struct UnitBanks;

impl Table for UnitBanks {
    type Key = UnitBank;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "unit_banks",
        family: Family::Objects,
        key: &[
            integer("bin"),
            integer("entry"),
            name("unit"),
            integer("bank"),
        ],
        facts: &[],
        shape: Shape::Keyed,
        lookups: &["bank"],
        parts: None,
    };

    fn cells<'a>(key: &'a UnitBank, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [
            key.bin.into(),
            key.entry.into(),
            Cell::Name(&key.unit),
            key.bank.into(),
        ]
        .into_iter()
    }
}

/// An event a bank unit lists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitEvent {
    /// The path of the object's bin.
    pub bin: WadHash,
    /// The object's entry hash.
    pub entry: BinHash,
    /// The unit's name.
    pub unit: String,
    /// The event's Wwise id.
    pub event: u32,
}

/// `unit_events`: the events a game unit lists.
pub struct UnitEvents;

impl Table for UnitEvents {
    type Key = UnitEvent;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "unit_events",
        family: Family::Objects,
        key: &[
            integer("bin"),
            integer("entry"),
            name("unit"),
            integer("event"),
        ],
        facts: &[],
        shape: Shape::Keyed,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a UnitEvent, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [
            key.bin.into(),
            key.entry.into(),
            Cell::Name(&key.unit),
            key.event.into(),
        ]
        .into_iter()
    }
}

/// `clips`: every clip the game shipped in a bank.
pub struct Clips;

impl Table for Clips {
    type Key = ClipKey;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "clips",
        family: Family::Clips,
        key: &[integer("key")],
        facts: &[],
        shape: Shape::Keyed,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a ClipKey, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [(*key).into()].into_iter()
    }
}

/// An event an events bank defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BankEvent {
    /// The bank's path.
    pub bank: WadHash,
    /// The event's Wwise id.
    pub event: u32,
}

/// `events`: the events a game events bank defines.
pub struct Events;

impl Table for Events {
    type Key = BankEvent;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "events",
        family: Family::Clips,
        key: &[integer("bank"), integer("event")],
        facts: &[],
        shape: Shape::Keyed,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a BankEvent, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [key.bank.into(), key.event.into()].into_iter()
    }
}

/// A wem an event's play actions reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventWem {
    /// The bank's path.
    pub bank: WadHash,
    /// The event's Wwise id.
    pub event: u32,
    /// The wem's id.
    pub wem: u32,
}

/// `event_wems`: the wems a game event plays. Indexed by wem too.
pub struct EventWems;

impl Table for EventWems {
    type Key = EventWem;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "event_wems",
        family: Family::Clips,
        key: &[integer("bank"), integer("event"), integer("wem")],
        facts: &[],
        shape: Shape::Keyed,
        lookups: &["wem"],
        parts: None,
    };

    fn cells<'a>(key: &'a EventWem, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [key.bank.into(), key.event.into(), key.wem.into()].into_iter()
    }
}

/// A record's identity among the records at its path: the leading eight bytes of a SHA-256
/// over its fields, each string length-prefixed. It does not change with the compiler as
/// `Hash` may.
struct Digester(Sha256);

impl Digester {
    fn new(tag: &str) -> Self {
        let mut digester = Self(Sha256::new());
        digester.text(tag);
        digester
    }

    fn text(&mut self, text: &str) {
        self.int(text.len() as u64);
        self.0.update(text.as_bytes());
    }

    fn int(&mut self, n: u64) {
        self.0.update(n.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.int(bytes.len() as u64);
        self.0.update(bytes);
    }

    fn finish(self) -> u64 {
        let digest: [u8; 32] = self.0.finalize().into();
        let mut head = [0; 8];
        head.copy_from_slice(&digest[..8]);
        u64::from_be_bytes(head)
    }
}

/// A record held whole in a `Recorded` table, found by its path and its digest.
pub trait Recorded {
    /// The record's identity among the records at its path.
    fn digest(&self) -> u64;
}

/// A record at a path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct At<R> {
    /// The path.
    pub path: WadHash,
    /// What the game held there.
    pub record: R,
}

/// One joint of a skeleton.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Joint {
    /// The joint's name.
    pub name: String,
    /// Its parent's name, `#<ordinal>` where another joint has the parent's name, and none
    /// for a root.
    pub parent: Option<String>,
}

/// A skeleton: its joints in stored order.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Skeleton {
    /// How many joints influence vertices.
    pub influences: u32,
    /// The joints, in stored order.
    pub joints: Vec<Joint>,
}

impl Recorded for Skeleton {
    fn digest(&self) -> u64 {
        let mut d = Digester::new("skeleton");
        d.int(u64::from(self.influences));
        for joint in &self.joints {
            d.text(&joint.name);
            d.text(joint.parent.as_deref().unwrap_or("-"));
        }
        d.finish()
    }
}

/// `skeletons` and `joints`: each distinct skeleton the game held at a path.
pub struct Skeletons;

impl Table for Skeletons {
    type Key = At<Skeleton>;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "skeletons",
        family: Family::Rigs,
        key: &[integer("path"), integer("record")],
        facts: &[integer("influences")],
        shape: Shape::Recorded,
        lookups: &[],
        parts: Some(PartsDef {
            name: "joints",
            owner: "skeleton",
            columns: &[
                name("name"),
                Column {
                    name: "parent",
                    storage: Storage::OptionalName,
                },
            ],
        }),
    };

    fn cells<'a>(key: &'a At<Skeleton>, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [
            key.path.into(),
            key.record.digest().into(),
            key.record.influences.into(),
        ]
        .into_iter()
    }

    fn parts(key: &At<Skeleton>) -> Vec<Vec<Cell<'_>>> {
        let joints = key.record.joints.iter();
        joints
            .map(|joint| {
                let parent = joint.parent.as_deref().map_or(Cell::NoName, Cell::Name);
                vec![Cell::Name(&joint.name), parent]
            })
            .collect()
    }
}

/// The joints an animation clip animates: each joint's hash, in stored order.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JointList(pub Arc<[u32]>);

impl Recorded for JointList {
    fn digest(&self) -> u64 {
        let mut d = Digester::new("joint-list");
        for joint in self.0.iter() {
            d.int(u64::from(*joint));
        }
        d.finish()
    }
}

/// `joint_lists` and `list_joints`: each distinct list of joints a clip animates, held once.
/// The clips of a skin animate the same joints, so 64,903 clips hold 15,563 lists
/// (16.19.8207193).
pub struct JointLists;

impl Table for JointLists {
    type Key = JointList;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "joint_lists",
        family: Family::Rigs,
        key: &[integer("record")],
        facts: &[integer("count")],
        shape: Shape::Keyed,
        lookups: &[],
        parts: Some(PartsDef {
            name: "list_joints",
            owner: "list",
            columns: &[integer("joint")],
        }),
    };

    fn cells<'a>(key: &'a JointList, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [key.digest().into(), (key.0.len() as u64).into()].into_iter()
    }

    fn parts(key: &JointList) -> Vec<Vec<Cell<'_>>> {
        key.0.iter().map(|joint| vec![(*joint).into()]).collect()
    }
}

/// An animation clip's bytes at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Animation {
    /// The path.
    pub path: WadHash,
    /// The key of the clip's bytes.
    pub key: FileKey,
}

/// `animations`: every animation clip the game held at a path, with the record of its joint
/// list in `joint_lists`. Indexed by file key too.
pub struct Animations;

impl Table for Animations {
    type Key = Animation;
    type Facts = u64;
    const DEF: TableDef = TableDef {
        name: "animations",
        family: Family::Rigs,
        key: &[integer("path"), key("key")],
        facts: &[integer("joints")],
        shape: Shape::Keyed,
        lookups: &["key"],
        parts: None,
    };

    fn cells<'a>(key: &'a Animation, joints: &'a u64) -> impl Iterator<Item = Cell<'a>> {
        [key.path.into(), key.key.into(), (*joints).into()].into_iter()
    }
}

impl Recorded for Object {
    fn digest(&self) -> u64 {
        let mut d = Digester::new("bank-object");
        d.int(u64::from(self.kind));
        d.int(u64::from(self.id));
        d.bytes(&self.body);
        d.finish()
    }
}

/// `bank_objects`: each distinct object of an events bank's hierarchy, held once, by its
/// record. A bank changes a few objects from one build to the next, and the voice banks of two
/// languages share most: 4,862,673 objects in 18,689 banks are 2,360,592 distinct
/// (16.19.8207193).
pub struct BankObjects;

impl Table for BankObjects {
    type Key = u64;
    type Facts = Arc<Object>;
    const DEF: TableDef = TableDef {
        name: "bank_objects",
        family: Family::Clips,
        key: &[integer("record")],
        facts: &[integer("kind"), integer("id"), bytes("body")],
        shape: Shape::Keyed,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a u64, object: &'a Arc<Object>) -> impl Iterator<Item = Cell<'a>> {
        [
            (*key).into(),
            u32::from(object.kind).into(),
            object.id.into(),
            Cell::Bytes(&object.body),
        ]
        .into_iter()
    }
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    let four = bytes.get(at..).and_then(|rest| rest.first_chunk());
    four.copied().map_or(0, u32::from_le_bytes)
}

/// An events bank at a path, by its record: the digest of its header and of its objects'
/// records in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BankAt {
    /// The path.
    pub path: WadHash,
    /// The bank's record.
    pub record: u64,
}

/// What a row of `event_banks` keeps: the header, and the records of the bank's objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankFacts {
    /// The body of the bank's `BKHD` section.
    pub header: Arc<[u8]>,
    /// The record of each object in `bank_objects`, in stored order, eight bytes each, little
    /// endian.
    pub objects: Arc<[u8]>,
}

/// The bytes one object's record takes in `event_banks.objects`.
pub const RECORD_BYTES: usize = 8;

impl BankFacts {
    /// A bank's record and the facts of its row, from its header and the records of its
    /// objects in order.
    pub fn of(header: &[u8], records: &[u64]) -> (u64, Self) {
        let mut d = Digester::new("events-bank");
        d.bytes(header);
        let mut objects = Vec::with_capacity(records.len() * RECORD_BYTES);
        for record in records {
            d.int(*record);
            objects.extend_from_slice(&record.to_le_bytes());
        }
        let facts = Self {
            header: header.into(),
            objects: objects.into(),
        };
        (d.finish(), facts)
    }

    /// The records of the bank's objects, in stored order.
    pub fn records(objects: &[u8]) -> impl Iterator<Item = u64> {
        let (records, _) = objects.as_chunks::<RECORD_BYTES>();
        records.iter().copied().map(u64::from_le_bytes)
    }
}

/// `event_banks`: each distinct events bank the game held at a path: its version, its bank id,
/// its header, and its objects as their records in `bank_objects`.
///
/// The objects are one value and not rows of a parts table: the 162,930 banks of the history
/// list 49,000,478 objects (2026-10-04), and a parts row holds its owner and its ordinal beside
/// the eight bytes of the record.
pub struct EventBanks;

impl Table for EventBanks {
    type Key = BankAt;
    type Facts = BankFacts;
    const DEF: TableDef = TableDef {
        name: "event_banks",
        family: Family::Clips,
        key: &[integer("path"), integer("record")],
        facts: &[
            integer("version"),
            integer("bank_id"),
            bytes("header"),
            bytes("objects"),
        ],
        shape: Shape::Recorded,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a BankAt, facts: &'a BankFacts) -> impl Iterator<Item = Cell<'a>> {
        [
            key.path.into(),
            key.record.into(),
            u32_at(&facts.header, 0).into(),
            u32_at(&facts.header, 4).into(),
            Cell::Bytes(&facts.header),
            Cell::Bytes(&facts.objects),
        ]
        .into_iter()
    }
}

/// One submesh of a mesh.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Submesh {
    /// Its vertex count.
    pub vertices: u32,
    /// Its index count.
    pub indices: u32,
    /// The XXH3 of its positions as stored.
    pub positions: u64,
    /// The XXH3 of its positions rounded to 1/16 unit and sorted.
    pub rounded: u64,
    /// Its material's name.
    pub material: String,
}

/// A mesh: its vertex layout, its counts and its submeshes in stored order.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mesh {
    /// The vertex layout's name.
    pub vertex_type: String,
    /// The vertex count.
    pub vertices: u32,
    /// The index count.
    pub indices: u32,
    /// The submeshes, in stored order.
    pub submeshes: Vec<Submesh>,
}

impl Recorded for Mesh {
    fn digest(&self) -> u64 {
        let mut d = Digester::new("mesh");
        d.text(&self.vertex_type);
        d.int(u64::from(self.vertices));
        d.int(u64::from(self.indices));
        for submesh in &self.submeshes {
            d.text(&submesh.material);
            d.int(u64::from(submesh.vertices));
            d.int(u64::from(submesh.indices));
            d.int(submesh.positions);
            d.int(submesh.rounded);
        }
        d.finish()
    }
}

/// `meshes` and `submeshes`: each distinct mesh the game held at a path.
pub struct Meshes;

impl Table for Meshes {
    type Key = At<Mesh>;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "meshes",
        family: Family::Rigs,
        key: &[integer("path"), integer("record")],
        facts: &[name("vertex_type"), integer("vertices"), integer("indices")],
        shape: Shape::Recorded,
        lookups: &[],
        parts: Some(PartsDef {
            name: "submeshes",
            owner: "mesh",
            columns: &[
                integer("vertices"),
                integer("indices"),
                integer("positions"),
                integer("rounded"),
                name("material"),
            ],
        }),
    };

    fn cells<'a>(key: &'a At<Mesh>, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        let mesh = &key.record;
        [
            key.path.into(),
            mesh.digest().into(),
            Cell::Name(&mesh.vertex_type),
            mesh.vertices.into(),
            mesh.indices.into(),
        ]
        .into_iter()
    }

    fn parts(key: &At<Mesh>) -> Vec<Vec<Cell<'_>>> {
        let submeshes = key.record.submeshes.iter();
        submeshes
            .map(|s| {
                vec![
                    s.vertices.into(),
                    s.indices.into(),
                    s.positions.into(),
                    s.rounded.into(),
                    Cell::Name(&s.material),
                ]
            })
            .collect()
    }
}

/// A texture's header and the hash of its top mip.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Texture {
    /// The pixel format's name.
    pub format: String,
    /// The width of the top mip.
    pub width: u32,
    /// The height of the top mip.
    pub height: u32,
    /// How many mips it holds.
    pub mips: u32,
    /// The XXH3 of the top mip's stored bytes.
    pub top: u64,
}

impl Recorded for Texture {
    fn digest(&self) -> u64 {
        let mut d = Digester::new("texture");
        d.text(&self.format);
        for n in [self.width, self.height, self.mips] {
            d.int(u64::from(n));
        }
        d.int(self.top);
        d.finish()
    }
}

/// `textures`: each distinct texture the game held at a path.
pub struct Textures;

impl Table for Textures {
    type Key = At<Texture>;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "textures",
        family: Family::Textures,
        key: &[integer("path"), integer("record")],
        facts: &[
            name("format"),
            integer("width"),
            integer("height"),
            integer("mips"),
            integer("top"),
        ],
        shape: Shape::Recorded,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a At<Texture>, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        let texture = &key.record;
        [
            key.path.into(),
            texture.digest().into(),
            Cell::Name(&texture.format),
            texture.width.into(),
            texture.height.into(),
            texture.mips.into(),
            texture.top.into(),
        ]
        .into_iter()
    }
}

/// Every table of lives, in the order a whole write fills them. Names take their ids in this
/// order, so the same history gives the same file.
pub const TABLES: &[TableDef] = &[
    Files::DEF,
    Paths::DEF,
    Contents::DEF,
    Objects::DEF,
    Refs::DEF,
    Tags::DEF,
    Units::DEF,
    UnitBanks::DEF,
    UnitEvents::DEF,
    Clips::DEF,
    Events::DEF,
    EventWems::DEF,
    BankObjects::DEF,
    EventBanks::DEF,
    Skeletons::DEF,
    Meshes::DEF,
    JointLists::DEF,
    Animations::DEF,
    Textures::DEF,
];

/// The name of every table of the index: the tables of lives, their parts, and `names`.
pub fn table_names() -> Vec<&'static str> {
    let mut names = Vec::new();
    for table in TABLES {
        names.push(table.name);
        names.extend(table.parts.map(|parts| parts.name));
    }
    names.push("names");
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_is_created_by_the_shape_of_its_key() {
        assert_eq!(
            Paths::DEF.create_sql(),
            "CREATE TABLE paths (path INTEGER PRIMARY KEY, kind INTEGER NOT NULL, first INTEGER \
             NOT NULL, last INTEGER NOT NULL, alive INTEGER NOT NULL);\n"
        );
        assert_eq!(
            Contents::DEF.create_sql(),
            "CREATE TABLE contents (path INTEGER NOT NULL, key BLOB NOT NULL, first INTEGER NOT \
             NULL, last INTEGER NOT NULL, alive INTEGER NOT NULL, PRIMARY KEY (path, key)) \
             WITHOUT ROWID;\n"
        );
        assert_eq!(
            Contents::DEF.index_sql(),
            "CREATE INDEX contents_key ON contents (key);\n"
        );
        assert_eq!(
            Skeletons::DEF.create_sql(),
            "CREATE TABLE skeletons (id INTEGER PRIMARY KEY, path INTEGER NOT NULL, record \
             INTEGER NOT NULL, influences INTEGER NOT NULL, first INTEGER NOT NULL, last INTEGER \
             NOT NULL, alive INTEGER NOT NULL);\nCREATE TABLE joints (skeleton INTEGER NOT NULL, \
             ordinal INTEGER NOT NULL, name INTEGER NOT NULL, parent INTEGER, PRIMARY KEY \
             (skeleton, ordinal)) WITHOUT ROWID;\n"
        );
        assert_eq!(
            Skeletons::DEF.index_sql(),
            "CREATE UNIQUE INDEX skeletons_key ON skeletons (path, record);\n"
        );
        assert_eq!(
            JointLists::DEF.create_sql(),
            "CREATE TABLE joint_lists (record INTEGER PRIMARY KEY, count INTEGER NOT NULL, first \
             INTEGER NOT NULL, last INTEGER NOT NULL, alive INTEGER NOT NULL);\nCREATE TABLE \
             list_joints (list INTEGER NOT NULL, ordinal INTEGER NOT NULL, joint INTEGER NOT \
             NULL, PRIMARY KEY (list, ordinal)) WITHOUT ROWID;\n"
        );
        assert_eq!(
            BankObjects::DEF.create_sql(),
            "CREATE TABLE bank_objects (record INTEGER PRIMARY KEY, kind INTEGER NOT NULL, id \
             INTEGER NOT NULL, body BLOB NOT NULL, first INTEGER NOT NULL, last INTEGER NOT \
             NULL, alive INTEGER NOT NULL);\n"
        );
    }

    #[test]
    fn a_banks_record_is_of_its_header_and_its_objects_in_order() {
        let object = |id| Object {
            kind: 2,
            id,
            body: vec![1, 2],
        };
        let records = [object(1).digest(), object(2).digest()];
        assert_ne!(records[0], records[1]);
        let (record, facts) = BankFacts::of(&[145, 0, 0, 0, 7, 0, 0, 0], &records);
        let back: Vec<u64> = BankFacts::records(&facts.objects).collect();
        assert_eq!(back, records);
        let (reversed, _) = BankFacts::of(&[145, 0, 0, 0, 7, 0, 0, 0], &[records[1], records[0]]);
        let (other_header, _) = BankFacts::of(&[145, 0, 0, 0, 8, 0, 0, 0], &records);
        assert_ne!(record, reversed);
        assert_ne!(record, other_header);
        let bank = EventsBank::new(facts.header.to_vec(), vec![object(1)]).unwrap();
        assert_eq!((bank.version(), bank.bank_id()), (145, 7));
        assert_eq!(EventsBank::new(vec![145, 0, 0, 0], Vec::new()), None);
    }

    #[test]
    fn a_kind_is_its_place_in_the_list() {
        assert_eq!(FileKind::of(""), FileKind(0));
        assert_eq!(FileKind::of("tex"), FileKind(2));
        assert_eq!(FileKind::of("tex").name(), "tex");
        assert_eq!(FileKind::of("unheard-of"), FileKind::OTHER);
        assert_eq!(FileKind::OTHER.name(), "?");
    }

    #[test]
    fn families_read_from_a_list() {
        let some = Families::parse("clips, files").unwrap();
        assert!(some.has(Family::Files) && some.has(Family::Clips) && !some.has(Family::Rigs));
        assert_eq!(
            some.iter().collect::<Vec<_>>(),
            [Family::Files, Family::Clips]
        );
        assert_eq!(Family::ALL.into_iter().collect::<Families>(), Families::ALL);
        assert!(matches!(
            Families::parse("files,sounds"),
            Err(HistoryError::UnknownFamily(name)) if name == "sounds"
        ));
    }

    #[test]
    fn a_record_at_a_path_is_told_apart_by_its_digest() {
        let root = Joint {
            name: "Root".into(),
            parent: None,
        };
        let hip = Joint {
            name: "Hip".into(),
            parent: Some("Root".into()),
        };
        let one = Skeleton {
            influences: 4,
            joints: vec![root.clone()],
        };
        let two = Skeleton {
            influences: 4,
            joints: vec![root, hip],
        };
        assert_ne!(one.digest(), two.digest());
        assert_eq!(one.digest(), one.clone().digest());
    }
}
