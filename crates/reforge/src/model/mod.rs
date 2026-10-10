//! The mod as the archive ships it, and the small types every stage shares: typed ids, hashes,
//! history keys, checked paths, text made safe to print, the walk over bin values, and what a
//! fixup says of itself.
//!
//! `docs/design/ARCHITECTURE.md`, "The model".

pub mod bank;
pub mod bin;
pub mod blobs;
pub mod dump;
pub mod entity;
pub mod fixup;
pub mod hash;
pub mod id;
pub mod keys;
pub mod known;
pub mod layer;
pub mod material;
pub mod names;
pub mod pairs;
pub mod path;
pub mod record;
pub mod rig;
pub mod text;
pub mod value;
pub mod view;
pub mod walk;

pub use bank::{BankClip, BankRole, EventsBank, INIT_BANK, game_bank_path};
pub use blobs::{BlobAccess, BlobFacts, Blobs, FactAccess, FactRow, Fetched, Head, Missing};
pub use byteview::ByteView;
pub use dump::{Dump, DumpCx};
pub use entity::Mod;
pub use fixup::{FixupId, FixupInfo, Phase, Version};
pub use hash::{BinHash, ContentId, MAX_SLOT, Slot, WadHash, name_hash, path_hash};
pub use id::{Arena, Id, SideTable, Sigil};
pub use keys::{ClipKey, FileKey, NormKey, ObjectKey};
pub use layer::Layer;
pub use names::{NoNames, Spell};
pub use path::{ChunkPath, PathError, WadName};
pub use record::{Record, Series};
pub use text::{Spelled, clean, clean_json, panic_text};
pub use view::{AudioBlock, AudioUnit, SkinEntry};
pub use walk::{FieldPath, Fields, StructMut, StructRef};
