//! Spelling a hash from the run's names and the tables together.

use std::borrow::Cow;

use ltk_meta::path::FieldNames;

use super::book::NameBook;
use super::namehash::NameHashError;
use super::names::{NameTable, Names};
use crate::model::{BinHash, Spell, WadHash};

/// Spells hashes from a run's [`NameBook`] and the [`Names`] tables. The order is the same for
/// every kind of hash: the run's names, then the hash cache, then the text tables.
#[derive(Debug, Clone, Copy)]
pub struct Speller<'a> {
    book: &'a NameBook,
    names: &'a Names,
}

impl<'a> Speller<'a> {
    /// A speller over a run's names and the tables.
    pub fn new(book: &'a NameBook, names: &'a Names) -> Self {
        Self { book, names }
    }

    /// The one place the source order is written.
    fn spell(&self, table: NameTable, hash: u64) -> Option<String> {
        self.book
            .spell(table, hash)
            .map(str::to_string)
            .or_else(|| self.names.spell(table, hash))
    }

    /// The plaintext behind a hash value or a hashed map key.
    pub fn hash(&self, hash: BinHash) -> Option<String> {
        self.spell(NameTable::Hashes, u64::from(hash.0))
    }

    /// The path whose name hash is `hash`, the FNV-1a hash an entry name has: the first such
    /// path the run learnt, else the one path the tables spell with it.
    pub fn path_by_name_hash(&self, hash: BinHash) -> Result<Option<String>, NameHashError> {
        match self.book.path_by_name_hash(hash) {
            Some(path) => Ok(Some(path.to_string())),
            None => self.names.path_by_name_hash(hash),
        }
    }
}

impl Spell for Speller<'_> {
    fn path(&self, hash: WadHash) -> Option<String> {
        self.spell(NameTable::Paths, hash.0)
    }

    fn entry(&self, hash: BinHash) -> Option<String> {
        self.spell(NameTable::Entries, u64::from(hash.0))
    }

    fn field(&self, hash: BinHash) -> Option<String> {
        self.spell(NameTable::Fields, u64::from(hash.0))
    }

    fn class(&self, hash: BinHash) -> Option<String> {
        self.spell(NameTable::Classes, u64::from(hash.0))
    }
}

/// The names a PTCH record's path is spelled with. A field no source names cannot be spelled,
/// so no record addresses it.
impl FieldNames for Speller<'_> {
    fn field(&self, field: BinHash, _class: Option<BinHash>) -> Option<Cow<'_, str>> {
        Spell::field(self, field).map(Cow::Owned)
    }

    fn hash(&self, hash: BinHash) -> Option<Cow<'_, str>> {
        Speller::hash(self, hash).map(Cow::Owned)
    }
}
