//! Why a bank does not read. Each variant carries the numbers as the file states them. A tag or
//! a name from the file is shown escaped: it is a mod's text.

use crate::BankKind;

/// The UTF-16 units of an entry's name an error keeps: the file states the name's length.
pub(crate) const NAME_SHOWN: usize = 64;

/// The reason a bank or a package does not read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BankError {
    /// The bytes start as neither a `.bnk` nor a `.wpk`. `head` is their first eight, or fewer.
    #[error("neither a sound bank nor a package: {len} bytes starting \"{}\"", .head.escape_ascii())]
    NotABank { len: usize, head: Vec<u8> },

    /// The file ends inside a section's tag and length.
    #[error("a section's tag and length need 8 bytes at byte {at} of a file of {len} bytes")]
    SectionHeaderCutShort { at: usize, len: usize },

    /// A section states more bytes than the file holds after its length.
    #[error(
        "section \"{}\" at byte {at} states {size} bytes in a file of {len} bytes",
        .tag.escape_ascii()
    )]
    SectionPastEnd {
        tag: [u8; 4],
        at: usize,
        size: u32,
        len: usize,
    },

    /// A second `BKHD`, `DIDX`, `DATA` or `HIRC`: a bank holds one of each.
    #[error("a second \"{}\" section at byte {at}", .tag.escape_ascii())]
    DuplicateSection { tag: [u8; 4], at: usize },

    /// A `BKHD` too short for the version and the bank id.
    #[error("a BKHD of {size} bytes, shorter than the 8 of its version and bank id")]
    HeaderTooShort { size: usize },

    /// A `HIRC` too short for its count.
    #[error("a HIRC of {size} bytes, shorter than the 4 of its count")]
    HircWithoutCount { size: usize },

    /// A `HIRC` that ends inside an object's type and size, before the count it states is met.
    #[error(
        "HIRC object {index} of {count} needs 5 bytes for its type and size at byte {at} of a \
         section of {len} bytes"
    )]
    ObjectCutShort {
        index: u32,
        count: u32,
        at: usize,
        len: usize,
    },

    /// A `HIRC` object that states more bytes than its section holds after it.
    #[error(
        "HIRC object {index} of type {kind} states {size} bytes with {left} left in its section"
    )]
    ObjectPastSection {
        index: u32,
        kind: u8,
        size: u32,
        left: usize,
    },

    /// A `HIRC` object too short for its id.
    #[error("HIRC object {index} of type {kind} is {size} bytes, shorter than the 4 of its id")]
    ObjectWithoutId { index: u32, kind: u8, size: u32 },

    /// A `DIDX` that lists clips in a bank with no `DATA`.
    #[error("a DIDX of {clips} clips in a bank with no DATA")]
    IndexWithoutData { clips: usize },

    /// A clip whose bytes run past the data they are read from: `DATA` in a bank, the file in a
    /// package.
    #[error("clip {id} is {size} bytes at {offset}, past the {data} bytes it is read from")]
    ClipPastData {
        id: u32,
        offset: u32,
        size: u32,
        data: usize,
    },

    /// Clips that together are larger than the data they are read from (`DATA` in a bank, the
    /// file in a package), as when many entries name one range. Refused before any is copied.
    #[error(
        "the clips are {clips} bytes together, more than the {data} bytes of the {} they are \
         read from",
        clip_source(.kind)
    )]
    ClipsLargerThanData {
        kind: BankKind,
        clips: u64,
        data: u64,
    },

    /// A package too short for its magic, version and count.
    #[error("a package of {len} bytes, shorter than the 12 of its magic, version and count")]
    PackageHeaderCutShort { len: usize },

    /// A package that states more entries than it holds offsets for.
    #[error("a package of {len} bytes states {count} entries and ends inside their offsets")]
    PackageOffsetsCutShort { count: u32, len: usize },

    /// A package entry whose data offset, size and name length are not in the file.
    #[error("package entry {entry} needs 12 bytes at {offset} in a file of {len} bytes")]
    EntryCutShort {
        entry: usize,
        offset: u32,
        len: usize,
    },

    /// A package entry that states a longer name than the file holds after it.
    #[error(
        "package entry {entry} states a name of {name_len} characters with {left} bytes left in \
         the file"
    )]
    EntryNameCutShort {
        entry: usize,
        name_len: u32,
        left: usize,
    },

    /// A package entry whose name does not start with a clip id that fits 32 bits. `name` is the
    /// start of the name.
    #[error("package entry {entry} is named \"{}\", which starts with no clip id", .name.escape_default())]
    EntryName { entry: usize, name: String },
}

fn clip_source(kind: &BankKind) -> &'static str {
    match kind {
        BankKind::Bnk => "DATA section",
        BankKind::Wpk => "file",
    }
}
