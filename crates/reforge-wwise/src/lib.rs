//! Wwise sound banks (`.bnk`) and packages (`.wpk`) as the League client loads them. A leaf
//! crate: it knows nothing of a mod or a port.
//!
//! - A `.bnk` is a run of sections, each a four-byte tag and a length: `BKHD` (the version, then
//!   the bank id, the language, the clips' alignment), `DIDX` (twelve bytes per clip: id, offset
//!   into `DATA`, size), `DATA` (the clips, each at a multiple of the alignment counted from the
//!   start of the file, not of `DATA`), `HIRC` (a count, then objects: a type byte, a size, the
//!   id, the body). Other sections are kept as bytes, in their place.
//! - A `.wpk` is `r3d2`, a version, a count, one offset per entry, and at each offset (a multiple
//!   of eight) the data's offset, its size, the name's length and the name in UTF-16
//!   (`<id>.wem`); the clips follow the entries, each at a multiple of eight.
//!
//! An object's body is kept whole; `Object` reads off it only what a cut needs: a node's parent,
//! a sound's clip, an action's target and the bank it expects the target in, an event's actions.
//! A well-formed bank read and written unchanged comes back byte for byte.
//!
//! The bytes are a mod's. The reader checks every offset, size and count against the bytes
//! present and returns a `BankError`; it allocates nothing from a number alone. Clips that
//! together are larger than the data they are read from are refused before any is copied, so
//! index entries naming one range many times cannot make a bank read larger than its file.

mod bank;
mod cursor;
mod cut;
mod error;
mod object;
mod reader;
mod writer;

pub use bank::{Bank, BankKind, Wem};
pub use cut::Cut;
pub use error::BankError;
pub use object::{Action, Object};

/// Object types the readers follow.
pub mod kind {
    /// A sound: one clip under a node.
    pub const SOUND: u8 = 2;
    /// An action of an event.
    pub const ACTION: u8 = 3;
    /// An event: a list of actions.
    pub const EVENT: u8 = 4;
    /// A random or sequence container.
    pub const RANDOM: u8 = 5;
    /// A switch container.
    pub const SWITCH: u8 = 6;
    /// An actor-mixer.
    pub const ACTOR_MIXER: u8 = 7;
    /// An audio bus.
    pub const BUS: u8 = 8;
    /// A layer container.
    pub const LAYER: u8 = 9;
    /// A music segment.
    pub const MUSIC_SEGMENT: u8 = 10;
    /// A music switch container.
    pub const MUSIC_SWITCH: u8 = 12;
    /// A music playlist container.
    pub const MUSIC_PLAYLIST: u8 = 13;
    /// An auxiliary bus.
    pub const AUX_BUS: u8 = 18;
}

/// The action type that plays its target.
pub const ACTION_PLAY: u8 = 4;
/// The bank version the client loads events banks at.
pub const CLIENT_VERSION: u32 = 145;

/// Wwise's id of a name: 32-bit FNV-1 (multiply, then xor) of the lowercase name. Events and banks
/// (by file stem) are named this way.
pub fn id_of(name: &str) -> u32 {
    name.to_ascii_lowercase()
        .bytes()
        .fold(0x811c_9dc5u32, |h, b| {
            h.wrapping_mul(0x0100_0193) ^ u32::from(b)
        })
}

/// Whether the bytes start as a package: `r3d2` and a small version, not one of the other formats
/// that spell `r3d2` on.
pub fn is_wpk(bytes: &[u8]) -> bool {
    bytes.starts_with(b"r3d2") && cursor::u32_at(bytes, 4).is_some_and(|version| version < 16)
}

/// Whether the bytes start as a sound bank: a `BKHD` tag and its length.
pub fn is_bnk(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes.starts_with(b"BKHD")
}
