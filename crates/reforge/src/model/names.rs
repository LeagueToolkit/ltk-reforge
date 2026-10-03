//! Spelling hashes for a person. Names are cosmetic: a hash no table spells prints as hex, and
//! nothing a run decides depends on a name.

use super::hash::{BinHash, WadHash};

/// Spells the hashes a mod and the game are made of.
pub trait Spell {
    /// The path a chunk hash is the hash of.
    fn path(&self, hash: WadHash) -> Option<String>;
    /// The entry name an entry hash is the hash of.
    fn entry(&self, hash: BinHash) -> Option<String>;
    /// The field name a field hash is the hash of.
    fn field(&self, hash: BinHash) -> Option<String>;
    /// The class name a class hash is the hash of.
    fn class(&self, hash: BinHash) -> Option<String>;

    /// The path, else the hash as sixteen hex digits.
    fn path_or_hex(&self, hash: WadHash) -> String {
        self.path(hash)
            .unwrap_or_else(|| format!("{:016x}", hash.0))
    }

    /// The entry name, else the hash as eight hex digits.
    fn entry_or_hex(&self, hash: BinHash) -> String {
        self.entry(hash)
            .unwrap_or_else(|| format!("{:08x}", hash.0))
    }

    /// The field name, else the hash as eight hex digits.
    fn field_or_hex(&self, hash: BinHash) -> String {
        self.field(hash)
            .unwrap_or_else(|| format!("{:08x}", hash.0))
    }

    /// The class name, else the hash as eight hex digits.
    fn class_or_hex(&self, hash: BinHash) -> String {
        self.class(hash)
            .unwrap_or_else(|| format!("{:08x}", hash.0))
    }
}

/// Spells nothing: every hash prints as hex.
pub struct NoNames;

impl Spell for NoNames {
    fn path(&self, _: WadHash) -> Option<String> {
        None
    }
    fn entry(&self, _: BinHash) -> Option<String> {
        None
    }
    fn field(&self, _: BinHash) -> Option<String> {
        None
    }
    fn class(&self, _: BinHash) -> Option<String> {
        None
    }
}
