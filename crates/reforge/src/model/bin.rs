//! Property bins read from bytes: the tree through `ltk_meta`, and each object's byte range from
//! the same bytes, so the history's object key and the norm key come from the reader and not from
//! a parser of this crate's.

use std::collections::HashMap;
use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ltk_meta::{Bin, BinObject, BinStream};

use super::hash::{BinHash, ContentId};
use super::text::panic_text;

/// Why bytes do not read as a property bin.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BinError {
    /// The bytes are a PTCH, which patches a bin and is none.
    #[error("a patch (PTCH), not a bin")]
    Patch,
    /// The bytes do not begin as a bin does.
    #[error("not a property bin")]
    NotABin,
    /// `ltk_meta` refused the bytes.
    #[error("{0}")]
    Read(String),
    /// `ltk_meta` panicked on the bytes.
    #[error("the reader panicked: {0}")]
    Panicked(String),
    /// An object's range in the table of contents ends past the file.
    #[error("object {0:08x} runs past the end of the file")]
    PastEnd(u32),
    /// The table of contents lists an object the tree does not hold.
    #[error("object {0:08x} is in the table of contents and not in the tree")]
    NotInTree(u32),
    /// The tree and the table of contents hold different numbers of objects.
    #[error("{tree} objects in the tree, {listed} in the table of contents")]
    Count {
        /// Objects in the tree.
        tree: usize,
        /// Distinct objects in the table of contents.
        listed: usize,
    },
}

/// One object of a bin, with the digests of its bytes.
#[derive(Debug, Clone)]
pub struct ParsedObject {
    /// Its properties.
    pub value: BinObject,
    /// The digest of its bytes from its entry hash to its end: the history's object key.
    pub raw: ContentId,
    /// The digest of its bytes after its entry hash: the same object under another name has the
    /// same body.
    pub body: ContentId,
}

/// A bin read from bytes.
#[derive(Debug, Clone)]
pub struct ParsedBin {
    /// The paths of the bins it links, as it spells them.
    pub dependencies: Vec<String>,
    /// Its objects, in file order.
    pub objects: Vec<ParsedObject>,
    /// Entry hashes the table of contents lists more than once. The tree keeps the last object
    /// under each, at the first one's place, and so does `objects`.
    pub duplicates: Vec<BinHash>,
}

/// Reads a `PROP` bin. The bytes may be a mod's, so the reader runs where its panic is caught.
pub fn parse(bytes: &[u8]) -> Result<ParsedBin, BinError> {
    match bytes.get(..4) {
        Some(b"PROP") => {}
        Some(b"PTCH") => return Err(BinError::Patch),
        _ => return Err(BinError::NotABin),
    }
    catch_unwind(AssertUnwindSafe(|| read(bytes)))
        .unwrap_or_else(|payload| Err(BinError::Panicked(panic_text(&*payload))))
}

fn read(bytes: &[u8]) -> Result<ParsedBin, BinError> {
    let text = |e: &dyn std::fmt::Display| BinError::Read(e.to_string());
    let mut stream = BinStream::mount(Cursor::new(bytes)).map_err(|e| text(&e))?;
    let toc = stream.toc().map_err(|e| text(&e))?.clone();
    let bin = stream.into_bin().map_err(|e| text(&e))?;
    let mut objects: Vec<ParsedObject> = Vec::new();
    let mut place: HashMap<u32, usize> = HashMap::new();
    let mut duplicates = Vec::new();
    for entry in toc.entries() {
        let hash = entry.path_hash.0;
        // An entry's offset is of its size field; the object's bytes begin four bytes on.
        let range = usize::try_from(entry.offset)
            .ok()
            .and_then(|offset| offset.checked_add(4))
            .and_then(|start| Some(start..start.checked_add(entry.size as usize)?))
            .and_then(|range| bytes.get(range))
            .ok_or(BinError::PastEnd(hash))?;
        let value = bin
            .objects
            .get(&entry.path_hash)
            .cloned()
            .ok_or(BinError::NotInTree(hash))?;
        let parsed = ParsedObject {
            value,
            raw: ContentId::of(range),
            body: ContentId::of(range.get(4..).unwrap_or_default()),
        };
        match place.get(&hash) {
            Some(&i) => {
                duplicates.push(entry.path_hash);
                objects[i] = parsed;
            }
            None => {
                place.insert(hash, objects.len());
                objects.push(parsed);
            }
        }
    }
    if objects.len() != bin.objects.len() {
        return Err(BinError::Count {
            tree: bin.objects.len(),
            listed: objects.len(),
        });
    }
    Ok(ParsedBin {
        dependencies: bin.dependencies,
        objects,
        duplicates,
    })
}

/// A bin as bytes.
///
/// # Panics
///
/// When `ltk_meta` fails to write into memory, which no value of a bin makes it do.
pub fn write(bin: &Bin) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    bin.to_writer(&mut out).unwrap_or_else(|e| {
        panic!(
            "writing a bin of {} objects into memory failed: {e}",
            bin.objects.len()
        )
    });
    out.into_inner()
}
