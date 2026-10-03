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

use serde::Serialize;

pub use categories::Categories;
pub use kind::{Info, Kind};

/// How many leading bytes [`identify`] reads. It is given fewer for a shorter file.
pub const HEAD_LEN: usize = 64;

/// How [`identify`] knew.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
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

/// What a file is, from its first bytes ([`HEAD_LEN`] of them, fewer for a short file) and its
/// name where it has one. A chunk named by its hash has none.
pub fn identify(head: &[u8], name: Option<&str>) -> Identity {
    let ext = name
        .and_then(|n| n.rsplit_once('.'))
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    let is = |kind, confidence| Identity { kind, confidence };
    if let Some(kind) = name.and_then(sniff::by_name) {
        return is(kind, Confidence::Name);
    }
    if head.is_empty() {
        return is(Kind::Empty, Confidence::Magic);
    }
    if let Some(kind) = table::by_magic(head) {
        // An extension naming a format whose magic is a container's (a Max scene is an OLE file,
        // an OLE file named `thumbs.db` was caught above) refines it.
        if kind == Kind::Ole
            && let Some(named) = table::by_extension(&ext)
        {
            return is(named, Confidence::Name);
        }
        return is(kind, Confidence::Magic);
    }
    if let Some(kind) = sniff::structured(head) {
        return is(kind, Confidence::Magic);
    }
    if let Some(text) = sniff::text_of(head) {
        return is(sniff::text_kind(text, &ext), Confidence::Magic);
    }
    if let Some(kind) = table::by_extension(&ext) {
        return is(kind, Confidence::Name);
    }
    match sniff::heuristic(head) {
        Some(kind) => is(kind, Confidence::Heuristic),
        None => is(Kind::Unknown, Confidence::Magic),
    }
}
