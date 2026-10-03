//! The mod as the archive ships it, and the small types every stage shares: typed ids, hashes,
//! history keys, checked paths, text made safe to print.
//!
//! `docs/design/ARCHITECTURE.md`, "The model".

pub mod bin;
pub mod entity;
pub mod hash;
pub mod id;
pub mod keys;
pub mod names;
pub mod path;
pub mod text;
pub mod value;

pub use entity::Mod;
pub use hash::{BinHash, ContentId, MAX_SLOT, Slot, WadHash, name_hash, path_hash};
pub use id::{Arena, Id, SideTable, Sigil};
pub use keys::{ClipKey, FileKey, NormKey, ObjectKey};
pub use names::{NoNames, Spell};
pub use path::{ChunkPath, PathError, WadName};
pub use text::{Spelled, clean, clean_json};
