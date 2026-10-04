//! A PTCH in the making (#ADR-0026): whole objects, and records that each set one field of a
//! game object, addressed by the path of fields to it.
//!
//! A record's path is written as plain names, which the client hashes (#GAME-BEHAVIOR-0010).
//! A field no table names cannot be written: the record is refused, and the caller says what
//! it leaves out.

use std::io::Cursor;

use ltk_meta::path::PropertyPath;
use ltk_meta::{BinObject, BinOverride, PropertyPatch, PropertyValueEnum};

use crate::model::known::field;
use crate::model::{BinHash, Spell, name_hash};
use crate::port::{CARRIED_FIELDS, ICON_FIELDS};

/// A field on a record's path that no table names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no table names the field {:08x}", .0.0)]
pub struct Unnamed(pub BinHash);

/// Collects the objects and the records of one PTCH.
pub struct PtchBuilder<'a> {
    names: &'a dyn Spell,
    objects: Vec<BinObject>,
    records: Vec<PropertyPatch>,
}

impl<'a> PtchBuilder<'a> {
    /// A builder that spells field names with the run's names.
    pub fn new(names: &'a dyn Spell) -> Self {
        Self {
            names,
            objects: Vec::new(),
            records: Vec::new(),
        }
    }

    /// The name of a field: of the fields a port writes by its own code, else as the tables
    /// spell it.
    fn name(&self, hash: BinHash) -> Option<String> {
        let mut own = CARRIED_FIELDS.iter().chain(ICON_FIELDS).chain(field::ALL);
        let own = own.find(|name| name_hash(name) == hash);
        own.map(|name| (*name).to_string())
            .or_else(|| self.names.field(hash))
    }

    /// The path of fields as a record writes it. Each name hashes to its field.
    pub fn path(&self, fields: &[BinHash]) -> Result<PropertyPath, Unnamed> {
        let (first, rest) = fields.split_first().ok_or(Unnamed(BinHash(0)))?;
        let name = |hash: &BinHash| {
            let name = self.name(*hash).filter(|name| name_hash(name) == *hash);
            name.ok_or(Unnamed(*hash))
        };
        let mut path = PropertyPath::new(name(first)?).map_err(|_| Unnamed(*first))?;
        for hash in rest {
            path.push_field(&name(hash)?).map_err(|_| Unnamed(*hash))?;
        }
        Ok(path)
    }

    /// Adds a record that sets the field at a path of an object to a value.
    pub fn set(
        &mut self,
        entry: BinHash,
        fields: &[BinHash],
        value: PropertyValueEnum,
    ) -> Result<(), Unnamed> {
        let path = self.path(fields)?;
        self.records.push(PropertyPatch::new(entry, path, value));
        Ok(())
    }

    /// Adds a whole object: it is added to the patched bin, or takes the place of the object
    /// under its entry hash.
    pub fn object(&mut self, object: BinObject) {
        self.objects.push(object);
    }

    /// How many records and whole objects the patch holds.
    pub fn len(&self) -> usize {
        self.records.len() + self.objects.len()
    }

    /// Whether the patch holds nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The patch.
    pub fn build(self) -> BinOverride {
        BinOverride::builder()
            .objects(self.objects)
            .patches(self.records)
            .build()
    }

    /// The patch as the bytes of a PTCH.
    ///
    /// # Panics
    ///
    /// When `ltk_meta` fails to write into memory, which no patch makes it do.
    pub fn bytes(self) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let patch = self.build();
        patch
            .to_writer(&mut out)
            .unwrap_or_else(|error| panic!("a PTCH does not write into memory: {error}"));
        out.into_inner()
    }
}
