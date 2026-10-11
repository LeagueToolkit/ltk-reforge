//! The shapes a bin's header and rows take for the window.
//!
//! Copied from LTK Manager's `crates/ltk-manager-bin/src/bin_document/types.rs`. Cut: the
//! document id, the sandbox and the asset of a handle, the object tab's header, the read-only
//! gate and the declared state, a row's declared kind, and the header's dependency list and
//! deleted objects, which reach the window as counts.

use ltk_meta::property::Kind;
use serde::{Deserialize, Serialize};

/// How many rows one path of a projected read answers, the front end's `PAGE_SIZE`.
pub const READ_PAGE: usize = 500;

/// How many rows one projected read answers, past which it is refused.
///
/// Four pages. A caller batches its paths under it rather than reading a whole file in one
/// call, which keeps the answer a payload a viewport draws.
pub const READ_ROW_CAP: usize = 4 * READ_PAGE;

/// Which kind of bin file a document holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum BinFileKind {
    /// A `PROP`: the objects themselves.
    Prop,
    /// A `PTCH`: a layer over another bin.
    Patch,
}

/// What the header row says about a bin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BinHeader {
    /// A `PROP` or a `PTCH`.
    pub kind: BinFileKind,
    /// The file version of a `PROP`. A `PTCH` carries none.
    pub version: Option<u32>,
    /// The objects the file declares. For a `PTCH`, the objects it adds.
    pub objects: usize,
    /// How many bins a `PROP` names as its dependencies.
    pub dependencies: usize,
    /// The patch records of a `PTCH`.
    pub patches: usize,
    /// How many objects a `PTCH` deletes.
    pub deleted: usize,
}

/// What an open answers: the header, and the rows at depth zero.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BinDocumentHandle {
    /// The file's facts.
    pub header: BinHeader,
    /// One row per object in file order, then one per object the patch records target.
    pub rows: Vec<BinRow>,
}

/// A window of rows under one node, and how many there are in all.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BinRows {
    /// The rows of the window.
    pub rows: Vec<BinRow>,
    /// How many rows sit under the node.
    pub total: usize,
}

/// Where a row sits in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RowNode {
    /// An object of the file.
    Object,
    /// A property of an object, a struct or an embedded.
    Property,
    /// One element of a container, or the value of a present optional.
    Element,
    /// One entry of a map.
    Entry,
    /// An object the patch records of a `PTCH` target, holding those records.
    Target,
    /// One patch record of a `PTCH`.
    Record,
}

/// One row of the viewer, flat. Every text a person reads in it is cleaned; the path is an
/// address and is not.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BinRow {
    /// The object's path hash, `0x` and eight hex digits.
    pub entry: String,
    /// The hash path, every field a hash. Empty for the object itself, and `#` then the
    /// record's position under a patch target.
    pub path: String,
    /// The same path for a person. Empty for the object itself, and the record's own path
    /// first under a patch record.
    pub label: String,
    /// Where the row sits.
    pub node: RowNode,
    /// What the row is called: the object's path, the property's name, `[i]` or the key.
    pub name: String,
    /// The name is a hash nothing names.
    pub unnamed: bool,
    /// The value's kind. An object row and a target row have none.
    pub kind: Option<PropertyKind>,
    /// The value, in the shape its widget draws.
    pub value: BinValue,
}

/// The 27 kinds `ltk_meta` reads, as they cross IPC.
///
/// A mirror of [`Kind`], spelled in ritobin's words. The spelling is the tag a row draws. An
/// upstream rename or addition is a compile error here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub enum PropertyKind {
    /// `none`.
    #[serde(rename = "none")]
    None,
    /// `bool`.
    #[serde(rename = "bool")]
    Bool,
    /// `i8`.
    #[serde(rename = "i8")]
    I8,
    /// `u8`.
    #[serde(rename = "u8")]
    U8,
    /// `i16`.
    #[serde(rename = "i16")]
    I16,
    /// `u16`.
    #[serde(rename = "u16")]
    U16,
    /// `i32`.
    #[serde(rename = "i32")]
    I32,
    /// `u32`.
    #[serde(rename = "u32")]
    U32,
    /// `i64`.
    #[serde(rename = "i64")]
    I64,
    /// `u64`.
    #[serde(rename = "u64")]
    U64,
    /// `f32`.
    #[serde(rename = "f32")]
    F32,
    /// `vec2`.
    #[serde(rename = "vec2")]
    Vector2,
    /// `vec3`.
    #[serde(rename = "vec3")]
    Vector3,
    /// `vec4`.
    #[serde(rename = "vec4")]
    Vector4,
    /// `mtx44`.
    #[serde(rename = "mtx44")]
    Matrix44,
    /// `rgba`.
    #[serde(rename = "rgba")]
    Color,
    /// `string`.
    #[serde(rename = "string")]
    String,
    /// `hash`.
    #[serde(rename = "hash")]
    Hash,
    /// `file`.
    #[serde(rename = "file")]
    WadChunkLink,
    /// `list`.
    #[serde(rename = "list")]
    Container,
    /// `list2`.
    #[serde(rename = "list2")]
    UnorderedContainer,
    /// `pointer`.
    #[serde(rename = "pointer")]
    Struct,
    /// `embed`.
    #[serde(rename = "embed")]
    Embedded,
    /// `link`.
    #[serde(rename = "link")]
    ObjectLink,
    /// `option`.
    #[serde(rename = "option")]
    Optional,
    /// `map`.
    #[serde(rename = "map")]
    Map,
    /// `flag`.
    #[serde(rename = "flag")]
    BitBool,
}

impl From<Kind> for PropertyKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::None => Self::None,
            Kind::Bool => Self::Bool,
            Kind::I8 => Self::I8,
            Kind::U8 => Self::U8,
            Kind::I16 => Self::I16,
            Kind::U16 => Self::U16,
            Kind::I32 => Self::I32,
            Kind::U32 => Self::U32,
            Kind::I64 => Self::I64,
            Kind::U64 => Self::U64,
            Kind::F32 => Self::F32,
            Kind::Vector2 => Self::Vector2,
            Kind::Vector3 => Self::Vector3,
            Kind::Vector4 => Self::Vector4,
            Kind::Matrix44 => Self::Matrix44,
            Kind::Color => Self::Color,
            Kind::String => Self::String,
            Kind::Hash => Self::Hash,
            Kind::WadChunkLink => Self::WadChunkLink,
            Kind::Container => Self::Container,
            Kind::UnorderedContainer => Self::UnorderedContainer,
            Kind::Struct => Self::Struct,
            Kind::Embedded => Self::Embedded,
            Kind::ObjectLink => Self::ObjectLink,
            Kind::Optional => Self::Optional,
            Kind::Map => Self::Map,
            Kind::BitBool => Self::BitBool,
        }
    }
}

/// A row's value, in the shape its widget draws.
///
/// A hash rides beside the name the run or the tables give it. A person reads the name, and
/// the hash shows on hover.
#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BinValue {
    /// A `None`.
    None,
    /// A `Bool` or a `BitBool`.
    Bool {
        /// The value.
        value: bool,
    },
    /// Any integer kind, as text. A `U64` does not fit a JSON number.
    Integer {
        /// The digits.
        text: String,
    },
    /// An `F32`. A NaN or an infinity crosses as `null`.
    Float {
        /// The value.
        value: f32,
    },
    /// Two, three or four components.
    Vector {
        /// The components.
        values: Vec<f32>,
    },
    /// Sixteen cells, row-major.
    Matrix {
        /// The cells.
        values: Vec<f32>,
    },
    /// A colour of four bytes.
    Color {
        /// Red.
        r: u8,
        /// Green.
        g: u8,
        /// Blue.
        b: u8,
        /// Alpha.
        a: u8,
    },
    /// A string.
    String {
        /// The text.
        value: String,
    },
    /// `0x` and eight hex digits, and the string behind it where a name is known.
    Hash {
        /// The hash.
        hash: String,
        /// Its name.
        name: Option<String>,
    },
    /// Sixteen hex digits, and the chunk's path where a name is known.
    WadChunkLink {
        /// The hash.
        hash: String,
        /// The chunk's path.
        path: Option<String>,
    },
    /// `0x` and eight hex digits, and the object's path where a name is known.
    ObjectLink {
        /// The hash.
        hash: String,
        /// The object's path.
        name: Option<String>,
    },
    /// A `Container` or an `UnorderedContainer`, and the kind of every item.
    Container {
        /// How many items it holds.
        len: usize,
        /// The kind of every item.
        item_kind: PropertyKind,
    },
    /// A `Struct` with a class, or an `Embedded`.
    Struct {
        /// The class, `0x` and eight hex digits.
        class_hash: String,
        /// The class's name.
        class: Option<String>,
        /// How many properties it holds.
        len: usize,
    },
    /// A `Struct` with a class hash of zero.
    Null,
    /// Whether a value is held, and the kind it is or would be.
    Optional {
        /// A value is held.
        present: bool,
        /// The kind it is or would be.
        item_kind: PropertyKind,
    },
    /// The entries, and the kinds the map declares for its keys and its values.
    Map {
        /// How many entries it holds.
        len: usize,
        /// The kind of every key.
        key_kind: PropertyKind,
        /// The kind of every value.
        value_kind: PropertyKind,
    },
    /// A leaf this build has no widget for.
    Undrawn,
    /// The patch records a `PTCH` writes to one object.
    Records {
        /// How many records.
        len: usize,
    },
}
