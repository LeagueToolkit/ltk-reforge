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
//! the key at the head (none once it is gone), which an append starts from. A table of spans
//! keeps one row per unbroken life, so a question at one build is exact; the others keep the
//! hull of a key's lives in one row.

use std::fmt;
use std::hash::Hash;
use std::str::FromStr;
use std::sync::Arc;

use reforge_wwise::Object;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use ltk_meta::PropertyKind as Kind;

use super::HistoryError;
use crate::game::FieldType;
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

impl Life {
    /// Whether the life holds the build at a position.
    pub fn holds(&self, at: Position) -> bool {
        self.first <= at && at <= self.last
    }
}

/// A group of tables the walk fills together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    /// Contents, the WADs, and where each WAD held each content.
    Files,
    /// Objects, the bins that hold them, their texts' blobs, the bytes of the texts' objects,
    /// and from their texts the files and the objects they name, the types of their fields,
    /// their audio tags and bank units.
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
    /// A WAD's path in the manifest, as the id of its row in `wads`.
    Wad,
    /// An object, as the id of its row in `objects`.
    Object,
}

impl Storage {
    fn sql(self) -> &'static str {
        match self {
            Storage::Integer | Storage::Name | Storage::Wad | Storage::Object => "INTEGER NOT NULL",
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

const fn wad(name: &'static str) -> Column {
    Column {
        name,
        storage: Storage::Wad,
    }
}

const fn object(name: &'static str) -> Column {
    Column {
        name,
        storage: Storage::Object,
    }
}

/// How a table's rows are found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The key columns are the primary key.
    Keyed,
    /// A row has an `id` of its own, which its parts and other tables' rows store, and is found
    /// by its key columns through a unique index. The key of a rig or a texture ends with the
    /// digest of its whole record, and an events bank's with its file's key, so a changed record
    /// at a path is a new row.
    Recorded,
    /// A row is one unbroken life of its key: the key columns and `first` are the primary key,
    /// and a key that comes back after a build without it gets a row of its own.
    Spans,
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
    /// The column of `columns` that orders the parts and tells them apart, or none for an
    /// `ordinal` column of their own after the owner.
    pub by: Option<&'static str>,
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
            (Shape::Spans, _) => format!(
                "CREATE TABLE {} ({}, PRIMARY KEY ({}, first)) WITHOUT ROWID;\n",
                self.name,
                columns.join(", "),
                key.join(", ")
            ),
        };
        if let Some(parts) = &self.parts {
            let mut columns = vec![format!("{} INTEGER NOT NULL", parts.owner)];
            if parts.by.is_none() {
                columns.push("ordinal INTEGER NOT NULL".to_string());
            }
            columns_sql(parts.columns, &mut columns);
            sql.push_str(&format!(
                "CREATE TABLE {} ({}, PRIMARY KEY ({}, {})) WITHOUT ROWID;\n",
                parts.name,
                columns.join(", "),
                parts.owner,
                parts.by.unwrap_or("ordinal")
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

/// The table every WAD is held in once, by its path in the manifest.
pub const WADS_SQL: &str =
    "CREATE TABLE wads (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE);\n";

/// `object_bytes`: each distinct object of an entry text, once, as a zstd frame of its bytes
/// from the entry hash on (`objects.rs`). `keyframe` is the row whose bytes the frame was
/// compressed against, the row itself for a keyframe. `dict` is the row of `object_dicts` a
/// keyframe was compressed with, none for another row or a keyframe compressed with none.
pub const OBJECT_BYTES_SQL: &str = "CREATE TABLE object_bytes (id INTEGER PRIMARY KEY, \
     keyframe INTEGER NOT NULL, dict INTEGER, frame BLOB NOT NULL);\n";

/// `text_bytes`: the object of each entry text, by the text's blob, with the object's class. The
/// class is the text's: two texts whose objects write the same bytes under two classes share a
/// row of `object_bytes`.
pub const TEXT_BYTES_SQL: &str = "CREATE TABLE text_bytes (blob BLOB PRIMARY KEY, \
     class INTEGER NOT NULL, object INTEGER NOT NULL) WITHOUT ROWID;\n";

/// `object_dicts`: the dictionaries keyframes were compressed with, one per class that earned
/// one and a general one, of no class, for every other class. A whole build trains them, and an
/// append uses them as they are.
pub const OBJECT_DICTS_SQL: &str = "CREATE TABLE object_dicts (id INTEGER PRIMARY KEY, \
     class INTEGER, dict BLOB NOT NULL);\n";

/// The tables of the object bytes, which hold no lives: `entry_texts` holds a text's.
pub const OBJECT_TABLES: [&str; 3] = ["object_bytes", "text_bytes", "object_dicts"];

/// The statements that create every table of an empty index.
pub fn schema_sql() -> String {
    let mut sql: String = TABLES.iter().map(|table| table.create_sql()).collect();
    sql.push_str(NAMES_SQL);
    sql.push_str(WADS_SQL);
    sql.push_str(OBJECT_BYTES_SQL);
    sql.push_str(TEXT_BYTES_SQL);
    sql.push_str(OBJECT_DICTS_SQL);
    sql
}

/// The statements that index every table. A whole write runs a table's after its rows are in,
/// which is faster than keeping the indexes up to date per insert.
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
    /// A WAD's path, which the writer interns.
    Wad(&'a str),
    /// An object by its key and its class, which the writer finds in `objects`.
    Object(ObjectKey, BinHash),
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
    fn parts<'a>(key: &'a Self::Key, facts: &'a Self::Facts) -> Vec<Vec<Cell<'a>>> {
        let _ = (key, facts);
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

/// A content at a path in one WAD.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Placement {
    /// The path.
    pub path: WadHash,
    /// The WAD's path in the manifest.
    pub wad: Arc<str>,
    /// The content.
    pub key: FileKey,
    /// The checksum the WAD's table holds for the file, 0 for a WAD that holds none.
    pub checksum: u64,
}

/// `placements`: every content a WAD held at a path, one row per unbroken life, so the file at a
/// path at one build is one query. A path whose bytes go and come back has a row per life.
/// The path leads the key, and the content is indexed too, so every path of a content is one
/// query.
pub struct Placements;

impl Table for Placements {
    type Key = Placement;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "placements",
        family: Family::Files,
        key: &[integer("path"), wad("wad"), key("key"), integer("checksum")],
        facts: &[],
        shape: Shape::Spans,
        lookups: &["key"],
        parts: None,
    };

    fn cells<'a>(key: &'a Placement, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [
            key.path.into(),
            Cell::Wad(&key.wad),
            key.key.into(),
            key.checksum.into(),
        ]
        .into_iter()
    }
}

/// A WAD as its manifest described it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WadLife {
    /// The WAD's path in the manifest.
    pub wad: Arc<str>,
    /// The WAD format's `major.minor`.
    pub version: String,
    /// The manifest's id for this version of the file. It changes whenever the WAD's bytes do.
    pub file_id: u64,
    /// The manifest's tags for the file, sorted, joined by commas; empty for none.
    pub tags: String,
}

/// `wad_lives`: every WAD of the game as its manifest described it, one row per unbroken life.
pub struct WadLives;

impl Table for WadLives {
    type Key = WadLife;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "wad_lives",
        family: Family::Files,
        key: &[
            wad("wad"),
            name("version"),
            integer("file_id"),
            name("tags"),
        ],
        facts: &[],
        shape: Shape::Spans,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a WadLife, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [
            Cell::Wad(&key.wad),
            Cell::Name(&key.version),
            key.file_id.into(),
            Cell::Name(&key.tags),
        ]
        .into_iter()
    }
}

/// An object's bytes under one class. The key covers the bytes from the entry hash on, and a bin
/// stores the class apart, so the same bytes under two classes are two objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassedObject {
    /// The object's key.
    pub key: ObjectKey,
    /// The object's class.
    pub class: BinHash,
}

/// What a row of `objects` keeps: the first place the object was seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectFacts {
    /// The object's entry hash.
    pub entry: BinHash,
    /// The path of the first bin the object was seen in.
    pub bin: WadHash,
    /// The object's norm key.
    pub norm: NormKey,
}

/// `objects`: every object the game shipped, by its key and its class. Indexed by norm key too.
/// A row's id is what `bin_entries` stores of the object: an integer of a few bytes, where the
/// key and the class are twenty.
pub struct Objects;

impl Table for Objects {
    type Key = ClassedObject;
    type Facts = ObjectFacts;
    const DEF: TableDef = TableDef {
        name: "objects",
        family: Family::Objects,
        key: &[key("key"), integer("class")],
        facts: &[integer("entry"), integer("bin"), integer("norm")],
        shape: Shape::Recorded,
        lookups: &["norm"],
        parts: None,
    };

    fn cells<'a>(key: &'a ClassedObject, facts: &'a ObjectFacts) -> impl Iterator<Item = Cell<'a>> {
        [
            key.key.into(),
            key.class.into(),
            facts.entry.into(),
            facts.bin.into(),
            facts.norm.into(),
        ]
        .into_iter()
    }
}

/// One object of a bin, as the bin holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BinEntry {
    /// The object's entry hash.
    pub entry: BinHash,
    /// The object's class.
    pub class: BinHash,
    /// The object's key.
    pub object: ObjectKey,
    /// The object's norm key.
    pub norm: NormKey,
}

/// What a row of `bins` keeps: the bins a bin links, and its objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinFacts {
    /// The path hash of each bin it links, in the order the census history lists them, eight
    /// bytes each, little endian.
    pub links: Arc<[u8]>,
    /// Its objects, by entry hash. An entry hash the bin repeats keeps its first object.
    pub entries: Arc<[BinEntry]>,
}

/// The bytes one link takes in `bins.links`.
pub const LINK_BYTES: usize = 8;

impl BinFacts {
    /// A bin's facts from the paths it links and its objects.
    pub fn of(links: &[WadHash], entries: &[BinEntry]) -> Self {
        let packed: Vec<u8> = links.iter().flat_map(|link| link.0.to_le_bytes()).collect();
        let mut entries = entries.to_vec();
        entries.sort_by_key(|e| e.entry);
        entries.dedup_by_key(|e| e.entry);
        Self {
            links: packed.into(),
            entries: entries.into(),
        }
    }

    /// The paths of the bins `bins.links` holds, in order.
    pub fn links(packed: &[u8]) -> impl Iterator<Item = WadHash> {
        let (links, _) = packed.as_chunks::<LINK_BYTES>();
        links.iter().map(|link| WadHash(u64::from_le_bytes(*link)))
    }
}

/// `bins` and `bin_entries`: every bin the game shipped, by its content, with the bins it links
/// and its objects. A bin at a path at a build is its row in `placements`, then this.
///
/// The links are one value: a bin links a few bins, and its links are read whole. An object is
/// the id of its row in `objects`, which holds its key, its class and its norm key: a changed bin
/// repeats every object it holds, 21 a bin on average (2026-10-07, the first ten builds), so a
/// byte of an object's row here costs every one of them.
pub struct Bins;

impl Table for Bins {
    type Key = FileKey;
    type Facts = Arc<BinFacts>;
    const DEF: TableDef = TableDef {
        name: "bins",
        family: Family::Objects,
        key: &[key("key")],
        facts: &[bytes("links")],
        shape: Shape::Recorded,
        lookups: &[],
        parts: Some(PartsDef {
            name: "bin_entries",
            owner: "bin",
            by: Some("entry"),
            columns: &[integer("entry"), object("object")],
        }),
    };

    fn cells<'a>(key: &'a FileKey, facts: &'a Arc<BinFacts>) -> impl Iterator<Item = Cell<'a>> {
        [(*key).into(), Cell::Bytes(&facts.links)].into_iter()
    }

    fn parts<'a>(_: &'a FileKey, facts: &'a Arc<BinFacts>) -> Vec<Vec<Cell<'a>>> {
        let entries = facts.entries.iter();
        entries
            .map(|e| vec![e.entry.into(), Cell::Object(e.object, e.class)])
            .collect()
    }
}

/// A file's id in the census history, the git object id of its bytes: the same id is the same
/// bytes.
pub type BlobId = [u8; 20];

/// A bin entry's text in one WAD's bin.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryText {
    /// The object's entry hash.
    pub entry: BinHash,
    /// The path of the bin.
    pub bin: WadHash,
    /// The WAD's path in the manifest.
    pub wad: Arc<str>,
    /// The text's blob in the census history.
    pub blob: BlobId,
}

/// `entry_texts`: the blob of every bin entry's text in every WAD, one row per unbroken life, so
/// an object's text at a build is read from the census history by its id, with no search of
/// the build's tree. The text itself stays in the history.
///
/// The entry leads the key, so the bins that held an entry at a build are one query with no
/// index beside the table: an index of a table without row ids repeats the whole key, which
/// took half of this table's bytes (2026-10-07, the first ten builds).
pub struct EntryTexts;

impl Table for EntryTexts {
    type Key = EntryText;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "entry_texts",
        family: Family::Objects,
        key: &[integer("entry"), integer("bin"), wad("wad"), bytes("blob")],
        facts: &[],
        shape: Shape::Spans,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a EntryText, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [
            key.entry.into(),
            key.bin.into(),
            Cell::Wad(&key.wad),
            Cell::Bytes(&key.blob),
        ]
        .into_iter()
    }
}

/// An object an object links, by its entry hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Linking {
    /// The path of the linking object's bin.
    pub bin: WadHash,
    /// The linking object's entry hash.
    pub entry: BinHash,
    /// The entry hash of the object linked.
    pub linked: BinHash,
}

/// `links`: every `link` value a game object held, one row per unbroken life. With
/// `entry_texts`, which says which bin holds an entry at a build, the objects an object reaches
/// at a build are one recursive query.
pub struct Links;

impl Table for Links {
    type Key = Linking;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "links",
        family: Family::Objects,
        key: &[integer("bin"), integer("entry"), integer("linked")],
        facts: &[],
        shape: Shape::Spans,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a Linking, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [key.bin.into(), key.entry.into(), key.linked.into()].into_iter()
    }
}

/// How `schema` stores a kind slot that does not apply, a map's key kind of a list.
pub const NO_KIND: u8 = u8::MAX;

/// A kind as `schema` stores it: its number in `ltk_meta`.
fn kind_cell<'a>(kind: Option<Kind>) -> Cell<'a> {
    Cell::Integer(i64::from(kind.map_or(NO_KIND, u8::from)))
}

/// A field of a class with one type its values had in a text of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassField {
    /// The class.
    pub class: BinHash,
    /// The field.
    pub field: BinHash,
    /// The type: the value's kind, a map's key kind, an item's kind, and the class of an embed
    /// or a pointer, or of the structs a list, an option or a map holds.
    pub ty: FieldType,
}

/// `schema`: every type a field of a class had in the game's texts, one row per unbroken life,
/// so the field's kind at a build is one query. A field whose values had two kinds at one build
/// has two rows alive there, and a list of pointers a row per class its items had.
pub struct TextSchema;

impl Table for TextSchema {
    type Key = ClassField;
    type Facts = ();
    const DEF: TableDef = TableDef {
        name: "schema",
        family: Family::Objects,
        key: &[
            integer("class"),
            integer("field"),
            integer("kind"),
            integer("key_kind"),
            integer("item_kind"),
            integer("value_class"),
        ],
        facts: &[],
        shape: Shape::Spans,
        lookups: &[],
        parts: None,
    };

    fn cells<'a>(key: &'a ClassField, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        let ty = &key.ty;
        [
            key.class.into(),
            key.field.into(),
            kind_cell(Some(ty.kind)),
            kind_cell(ty.key),
            kind_cell(ty.item),
            ty.class.unwrap_or(BinHash(0)).into(),
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
            by: None,
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

    fn parts<'a>(key: &'a At<Skeleton>, (): &'a ()) -> Vec<Vec<Cell<'a>>> {
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
            by: None,
            columns: &[integer("joint")],
        }),
    };

    fn cells<'a>(key: &'a JointList, (): &'a ()) -> impl Iterator<Item = Cell<'a>> {
        [key.digest().into(), (key.0.len() as u64).into()].into_iter()
    }

    fn parts<'a>(key: &'a JointList, (): &'a ()) -> Vec<Vec<Cell<'a>>> {
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

/// An events bank at a path, by the file that held it (#ADR-0081).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BankAt {
    /// The path.
    pub path: WadHash,
    /// The key of the file.
    pub key: FileKey,
}

/// What a row of `event_banks` keeps: the bank's record, the header, and the records of the
/// bank's objects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BankFacts {
    /// The digest of its header and of its objects' records in order.
    pub record: u64,
    /// The body of the bank's `BKHD` section.
    pub header: Arc<[u8]>,
    /// The record of each object in `bank_objects`, in stored order, eight bytes each, little
    /// endian.
    pub objects: Arc<[u8]>,
}

/// The bytes one object's record takes in `event_banks.objects`.
pub const RECORD_BYTES: usize = 8;

impl BankFacts {
    /// The facts of a bank's row, from its header and the records of its objects in order.
    pub fn of(header: &[u8], records: &[u64]) -> Self {
        let mut d = Digester::new("events-bank");
        d.bytes(header);
        let mut objects = Vec::with_capacity(records.len() * RECORD_BYTES);
        for record in records {
            d.int(*record);
            objects.extend_from_slice(&record.to_le_bytes());
        }
        Self {
            record: d.finish(),
            header: header.into(),
            objects: objects.into(),
        }
    }

    /// The records of the bank's objects, in stored order.
    pub fn records(objects: &[u8]) -> impl Iterator<Item = u64> {
        let (records, _) = objects.as_chunks::<RECORD_BYTES>();
        records.iter().copied().map(u64::from_le_bytes)
    }
}

/// `event_banks`: each events bank the game held at a path, by the file that held it: its
/// record, its version, its bank id, its header, and its objects as their records in
/// `bank_objects`. `placements` on the path and the key says which WADs held it (#ADR-0081).
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
        key: &[integer("path"), key("key")],
        facts: &[
            integer("record"),
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
            key.key.into(),
            facts.record.into(),
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
            by: None,
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

    fn parts<'a>(key: &'a At<Mesh>, (): &'a ()) -> Vec<Vec<Cell<'a>>> {
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
    Placements::DEF,
    WadLives::DEF,
    Objects::DEF,
    EntryTexts::DEF,
    Bins::DEF,
    TextSchema::DEF,
    Links::DEF,
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

/// The name of every table of the index: the tables of lives, their parts, `names`, `wads` and
/// the tables of the object bytes.
pub fn table_names() -> Vec<&'static str> {
    let mut names = Vec::new();
    for table in TABLES {
        names.push(table.name);
        names.extend(table.parts.map(|parts| parts.name));
    }
    names.extend(["names", "wads"]);
    names.extend(OBJECT_TABLES);
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_is_created_by_the_shape_of_its_key() {
        assert_eq!(
            Clips::DEF.create_sql(),
            "CREATE TABLE clips (key INTEGER PRIMARY KEY, first INTEGER NOT NULL, last INTEGER \
             NOT NULL, alive INTEGER NOT NULL);\n"
        );
        assert_eq!(
            Events::DEF.create_sql(),
            "CREATE TABLE events (bank INTEGER NOT NULL, event INTEGER NOT NULL, first INTEGER \
             NOT NULL, last INTEGER NOT NULL, alive INTEGER NOT NULL, PRIMARY KEY (bank, event)) \
             WITHOUT ROWID;\n"
        );
        assert_eq!(
            Placements::DEF.create_sql(),
            "CREATE TABLE placements (path INTEGER NOT NULL, wad INTEGER NOT NULL, key BLOB NOT \
             NULL, checksum INTEGER NOT NULL, first INTEGER NOT NULL, last INTEGER NOT NULL, \
             alive INTEGER NOT NULL, PRIMARY KEY (path, wad, key, checksum, first)) WITHOUT \
             ROWID;\n"
        );
        assert_eq!(
            Placements::DEF.index_sql(),
            "CREATE INDEX placements_key ON placements (key);\n"
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
            Bins::DEF.create_sql(),
            "CREATE TABLE bins (id INTEGER PRIMARY KEY, key BLOB NOT NULL, links BLOB NOT NULL, \
             first INTEGER NOT NULL, last INTEGER NOT NULL, alive INTEGER NOT NULL);\nCREATE \
             TABLE bin_entries (bin INTEGER NOT NULL, entry INTEGER NOT NULL, object INTEGER NOT \
             NULL, PRIMARY KEY (bin, entry)) WITHOUT ROWID;\n"
        );
        assert_eq!(
            Objects::DEF.index_sql(),
            "CREATE UNIQUE INDEX objects_key ON objects (key, class);\nCREATE INDEX objects_norm \
             ON objects (norm);\n"
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
        let facts = BankFacts::of(&[145, 0, 0, 0, 7, 0, 0, 0], &records);
        let back: Vec<u64> = BankFacts::records(&facts.objects).collect();
        assert_eq!(back, records);
        let reversed = BankFacts::of(&[145, 0, 0, 0, 7, 0, 0, 0], &[records[1], records[0]]);
        let other_header = BankFacts::of(&[145, 0, 0, 0, 8, 0, 0, 0], &records);
        assert_ne!(facts.record, reversed.record);
        assert_ne!(facts.record, other_header.record);
        let bank = EventsBank::new(facts.header.to_vec(), vec![object(1)]).unwrap();
        assert_eq!((bank.version(), bank.bank_id()), (145, 7));
        assert_eq!(EventsBank::new(vec![145, 0, 0, 0], Vec::new()), None);
    }

    #[test]
    fn a_bins_links_are_read_back_in_order_and_its_entries_once_each() {
        let entry = |entry, class| BinEntry {
            entry: BinHash(entry),
            class: BinHash(class),
            object: ObjectKey([entry as u8; 16]),
            norm: NormKey(u64::from(entry)),
        };
        let links = [WadHash(9), WadHash(u64::MAX), WadHash(1)];
        let facts = BinFacts::of(&links, &[entry(5, 1), entry(2, 1), entry(5, 2)]);
        assert_eq!(BinFacts::links(&facts.links).collect::<Vec<_>>(), links);
        assert_eq!(facts.entries[..], [entry(2, 1), entry(5, 1)]);
    }

    #[test]
    fn a_life_holds_the_builds_from_its_first_to_its_last() {
        let life = Life::new(2, 4);
        assert!(life.holds(Position(2)) && life.holds(Position(4)));
        assert!(!life.holds(Position(1)) && !life.holds(Position(5)));
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
