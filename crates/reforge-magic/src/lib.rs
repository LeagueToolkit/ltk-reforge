//! What a file is, from its first bytes and, where it has one, its name: every League format and
//! what else mods carry (authoring sources, images, executables, archives, OS leftovers). Each
//! kind has a MIME type and a set of categories ([`Kind::info`]).
//!
//! The order of the checks decides ties. A name that says what the file is whatever it holds
//! comes first. The bytes follow in `ltk_file`'s order: fixed magics, the eight-byte `r3d2`
//! family before any looser `r3d2` match, magics at an offset next, then text sniffing, a name
//! without a magic, and last the heuristics for formats with no magic at all (TGA, the light
//! grid, inibin), which would claim random bytes if tried earlier.
//!
//! What is a fact about one kind (its MIME type, its fixed magics, the extensions that name it)
//! is a row of `table`. A check that is code is in `sniff`.

mod categories;
mod kind;
mod sniff;
mod table;

use serde::{Deserialize, Serialize};

pub use categories::Categories;
pub use kind::{Info, Kind};

/// How many leading bytes [`identify`] reads, fewer for a shorter file: the longest prefix any
/// check reads. A DOS header's `e_lfanew` ends at byte 64, and the text and IFF checks read
/// the whole head they are given, so a head of this length decides the kind and no check
/// needs the rest of the file.
pub const HEAD_LEN: usize = 64;

/// How [`identify`] knew.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    /// A fixed magic, or text whose opening says what it is.
    Magic,
    /// A format with no magic, judged by its header's plausible values.
    Heuristic,
    /// The name's extension alone.
    Name,
}

/// A file's kind, and how [`identify`] knew it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Identity {
    /// What the file is.
    pub kind: Kind,
    /// How the kind was known.
    pub confidence: Confidence,
}

impl Identity {
    /// The MIME type, usual extension and categories of the file's kind.
    pub fn info(self) -> Info {
        self.kind.info()
    }
}

/// What a file's first bytes say of its kind, before its name is read. [`identify`] is this
/// and the name: a shape of text, or bytes no magic names, take their kind from the name.
///
/// It is a fact of the bytes alone, so it is kept per blob and read back in place of the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeadKind {
    /// No bytes.
    Empty,
    /// A magic, fixed or held to a value, or text whose opening says what it is.
    Magic(Kind),
    /// Text that opens `{` and is no bin: JSON, or glTF by its name.
    Object,
    /// Text whose first line is an INI section: INI, or a particle system by its name.
    Section,
    /// Text whose opening names nothing: what its name says, else plain text.
    Text,
    /// No magic and no text, judged by its header's plausible values: what its name says
    /// where it says something, else this kind.
    Heuristic(Kind),
    /// Nothing the bytes say: what its name says, else unknown.
    Unknown,
}

impl HeadKind {
    /// What a file's first bytes say, read from at most [`HEAD_LEN`] of them.
    pub fn of(head: &[u8]) -> HeadKind {
        let head = &head[..head.len().min(HEAD_LEN)];
        if head.is_empty() {
            return HeadKind::Empty;
        }
        if let Some(kind) = table::by_magic(head).or_else(|| sniff::structured(head)) {
            return HeadKind::Magic(kind);
        }
        if let Some(text) = sniff::text_of(head) {
            return sniff::text_head(text);
        }
        match sniff::heuristic(head) {
            Some(kind) => HeadKind::Heuristic(kind),
            None => HeadKind::Unknown,
        }
    }

    /// What the file is, from what its bytes say and its name where it has one.
    pub fn identify(self, name: Option<&str>) -> Identity {
        let ext = name
            .and_then(|n| n.rsplit_once('.'))
            .map(|(_, e)| e.to_ascii_lowercase())
            .unwrap_or_default();
        let is = |kind, confidence| Identity { kind, confidence };
        if let Some(kind) = name.and_then(sniff::by_name) {
            return is(kind, Confidence::Name);
        }
        match (self, table::by_extension(&ext)) {
            (HeadKind::Empty, _) => is(Kind::Empty, Confidence::Magic),
            // An extension naming a format whose magic is a container's (a Max scene is an OLE
            // file, an OLE file named `thumbs.db` was caught above) refines it.
            (HeadKind::Magic(Kind::Ole), Some(named)) => is(named, Confidence::Name),
            (HeadKind::Magic(kind), _) => is(kind, Confidence::Magic),
            (HeadKind::Object, _) if ext == "gltf" => is(Kind::Gltf, Confidence::Magic),
            (HeadKind::Object, _) => is(Kind::Json, Confidence::Magic),
            (HeadKind::Section, _) if ext == "troy" => is(Kind::Troy, Confidence::Magic),
            (HeadKind::Section, _) => is(Kind::Ini, Confidence::Magic),
            (HeadKind::Text, _) => {
                let kind = table::by_text_extension(&ext).unwrap_or(Kind::Text);
                is(kind, Confidence::Magic)
            }
            (HeadKind::Heuristic(_) | HeadKind::Unknown, Some(named)) => {
                is(named, Confidence::Name)
            }
            (HeadKind::Heuristic(kind), None) => is(kind, Confidence::Heuristic),
            (HeadKind::Unknown, None) => is(Kind::Unknown, Confidence::Magic),
        }
    }
}

/// What a file is, from its first bytes ([`HEAD_LEN`] of them, fewer for a short file) and its
/// name where it has one. A chunk named by its hash has none.
pub fn identify(head: &[u8], name: Option<&str>) -> Identity {
    HeadKind::of(head).identify(name)
}
