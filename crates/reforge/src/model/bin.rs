//! Property bins read from bytes: the tree through `ltk_meta`, and each object's byte range from
//! the same bytes, so the history's object key and the norm key come from the reader and not from
//! a parser of this crate's.

use std::collections::HashSet;
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
    /// The table of contents lists more or fewer objects than the reader gave.
    #[error("the table lists {listed} objects and {read} were read")]
    ObjectCount {
        /// How many objects the table of contents lists.
        listed: usize,
        /// How many objects the reader gave.
        read: usize,
    },
    /// A bin that should hold one object alone holds another number.
    #[error("the bin holds {0} objects, not one")]
    NotOne(usize),
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
    /// Entry hashes the table of contents lists more than once, once for each further listing.
    /// `objects` holds the first object listed under each, as the client keeps it.
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
    let listed = listed(&mut stream).map_err(|e| text(&e))?;
    if toc.entries().len() != listed.len() {
        return Err(BinError::ObjectCount {
            listed: toc.entries().len(),
            read: listed.len(),
        });
    }
    let mut objects: Vec<ParsedObject> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut duplicates = Vec::new();
    for (entry, value) in toc.entries().iter().zip(listed) {
        let hash = entry.path_hash.0;
        // An entry's offset is of its size field; the object's bytes begin four bytes on.
        let range = usize::try_from(entry.offset)
            .ok()
            .and_then(|offset| offset.checked_add(4))
            .and_then(|start| Some(start..start.checked_add(entry.size as usize)?))
            .and_then(|range| bytes.get(range))
            .ok_or(BinError::PastEnd(hash))?;
        // #GAME-BEHAVIOR-0020: the client keeps the first object listed under a hash.
        if !seen.insert(hash) {
            duplicates.push(entry.path_hash);
            continue;
        }
        objects.push(ParsedObject {
            value,
            raw: ContentId::of(range),
            body: ContentId::of(range.get(4..).unwrap_or_default()),
        });
    }
    Ok(ParsedBin {
        dependencies: stream.dependencies().to_vec(),
        objects,
        duplicates,
    })
}

/// Every object the table lists, in file order, a hash listed again included.
///
/// `BinStream::into_bin` keeps one object per hash. An object that reads only under the legacy
/// numbering latches the stream onto it, and the objects read before it are read again under
/// it, as `into_bin` does.
fn listed(stream: &mut BinStream<Cursor<&[u8]>>) -> Result<Vec<BinObject>, ltk_meta::Error> {
    let numbering = stream.numbering();
    let objects = sweep(stream)?;
    if stream.numbering() == numbering {
        return Ok(objects);
    }
    sweep(stream)
}

/// Reads every object of the table from the top.
fn sweep(stream: &mut BinStream<Cursor<&[u8]>>) -> Result<Vec<BinObject>, ltk_meta::Error> {
    let mut objects = Vec::with_capacity(stream.class_hashes().len());
    let mut cursor = stream.objects();
    while let Some(mut object) = cursor.next()? {
        objects.push(object.read()?);
    }
    Ok(objects)
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

/// One object as the bytes of a bin that holds it alone: how a snapshot keeps an object by
/// content id (#ADR-0094).
pub fn write_one(object: &BinObject) -> Vec<u8> {
    write(&Bin::new([object.clone()], Vec::<String>::new()))
}

/// The object of a bin that holds one alone, as [`write_one`] wrote it.
pub fn read_one(bytes: &[u8]) -> Result<BinObject, BinError> {
    let parsed = parse(bytes)?;
    let count = parsed.objects.len() + parsed.duplicates.len();
    let mut objects = parsed.objects.into_iter();
    match (objects.next(), count) {
        (Some(object), 1) => Ok(object.value),
        _ => Err(BinError::NotOne(count)),
    }
}
