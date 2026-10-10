//! The names one run learns.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::names::NameTable;
use crate::model::{BinHash, WadHash, name_hash, path_hash};

/// The names one run learns: the paths the archive and its bins spell, and the entry, field,
/// class and hash names the run meets or makes. The run owns it.
///
/// The first name learnt for a hash stays, and a later name with the same hash is ignored. What
/// a hash spells then depends only on the order the run learns in, which is the same for the
/// same archive.
#[derive(Debug, Clone, Default)]
pub struct NameBook {
    paths: BTreeMap<WadHash, String>,
    /// The first path learnt for each name hash.
    paths_by_name_hash: BTreeMap<BinHash, WadHash>,
    entries: BTreeMap<BinHash, String>,
    fields: BTreeMap<BinHash, String>,
    classes: BTreeMap<BinHash, String>,
    hashes: BTreeMap<BinHash, String>,
}

impl NameBook {
    /// A book with nothing learnt.
    pub fn new() -> Self {
        Self::default()
    }

    /// Learns a path, lowercased, and returns its hash.
    pub fn learn_path(&mut self, path: &str) -> WadHash {
        let path = path.to_ascii_lowercase();
        let hash = path_hash(&path);
        self.paths_by_name_hash
            .entry(name_hash(&path))
            .or_insert(hash);
        self.paths.entry(hash).or_insert(path);
        hash
    }

    /// Learns an entry name and returns its hash.
    pub fn learn_entry(&mut self, name: &str) -> BinHash {
        Self::learn(&mut self.entries, name)
    }

    /// Learns a field name and returns its hash.
    pub fn learn_field(&mut self, name: &str) -> BinHash {
        Self::learn(&mut self.fields, name)
    }

    /// Learns a class name and returns its hash.
    pub fn learn_class(&mut self, name: &str) -> BinHash {
        Self::learn(&mut self.classes, name)
    }

    /// Learns the plaintext behind a hash value or a hashed map key, and returns its hash.
    pub fn learn_hash(&mut self, name: &str) -> BinHash {
        Self::learn(&mut self.hashes, name)
    }

    fn learn(names: &mut BTreeMap<BinHash, String>, name: &str) -> BinHash {
        let hash = name_hash(name);
        names.entry(hash).or_insert_with(|| name.to_string());
        hash
    }

    /// The name learnt for a hash of a table, the hash widened to 64 bits.
    pub(super) fn spell(&self, table: NameTable, hash: u64) -> Option<&str> {
        let names = match table {
            NameTable::Paths => return self.path(WadHash(hash)),
            NameTable::Entries => &self.entries,
            NameTable::Fields => &self.fields,
            NameTable::Classes => &self.classes,
            NameTable::Hashes => &self.hashes,
        };
        let hash = BinHash(u32::try_from(hash).ok()?);
        names.get(&hash).map(String::as_str)
    }

    /// The path learnt for a chunk hash.
    pub fn path(&self, hash: WadHash) -> Option<&str> {
        self.paths.get(&hash).map(String::as_str)
    }

    /// The entry name learnt for an entry hash.
    pub fn entry(&self, hash: BinHash) -> Option<&str> {
        self.entries.get(&hash).map(String::as_str)
    }

    /// The field name learnt for a field hash.
    pub fn field(&self, hash: BinHash) -> Option<&str> {
        self.fields.get(&hash).map(String::as_str)
    }

    /// The class name learnt for a class hash.
    pub fn class(&self, hash: BinHash) -> Option<&str> {
        self.classes.get(&hash).map(String::as_str)
    }

    /// The plaintext learnt for a hash value or a hashed map key.
    pub fn hash(&self, hash: BinHash) -> Option<&str> {
        self.hashes.get(&hash).map(String::as_str)
    }

    /// The first path learnt whose name hash is `hash`: the FNV-1a hash an entry name has.
    pub fn path_by_name_hash(&self, hash: BinHash) -> Option<&str> {
        self.path(*self.paths_by_name_hash.get(&hash)?)
    }

    /// Every path learnt, in the order of their hashes.
    pub fn paths(&self) -> impl Iterator<Item = (WadHash, &str)> {
        self.paths.iter().map(|(hash, path)| (*hash, path.as_str()))
    }

    /// Every entry name learnt, in the order of their hashes.
    pub fn entries(&self) -> impl Iterator<Item = (BinHash, &str)> {
        Self::each(&self.entries)
    }

    /// Every field name learnt, in the order of their hashes.
    pub fn fields(&self) -> impl Iterator<Item = (BinHash, &str)> {
        Self::each(&self.fields)
    }

    /// Every class name learnt, in the order of their hashes.
    pub fn classes(&self) -> impl Iterator<Item = (BinHash, &str)> {
        Self::each(&self.classes)
    }

    /// Every plaintext learnt for a hash value or a hashed map key, in the order of their
    /// hashes.
    pub fn hashes(&self) -> impl Iterator<Item = (BinHash, &str)> {
        Self::each(&self.hashes)
    }

    fn each(names: &BTreeMap<BinHash, String>) -> impl Iterator<Item = (BinHash, &str)> {
        names.iter().map(|(hash, name)| (*hash, name.as_str()))
    }
}

/// A book as a snapshot keeps it: each table by hash, a chunk hash as sixteen hex digits and a
/// bin hash as eight.
#[derive(Serialize, Deserialize)]
struct Rows {
    paths: BTreeMap<String, String>,
    paths_by_name_hash: BTreeMap<String, String>,
    entries: BTreeMap<String, String>,
    fields: BTreeMap<String, String>,
    classes: BTreeMap<String, String>,
    hashes: BTreeMap<String, String>,
}

fn wad_key(hash: WadHash) -> String {
    format!("{:016x}", hash.0)
}

fn bin_key(hash: BinHash) -> String {
    format!("{:08x}", hash.0)
}

fn names_by_key(names: &BTreeMap<BinHash, String>) -> BTreeMap<String, String> {
    names
        .iter()
        .map(|(&hash, name)| (bin_key(hash), name.clone()))
        .collect()
}

/// A table read back, each name checked against the hash it is kept under.
fn names_of<E: serde::de::Error>(
    rows: BTreeMap<String, String>,
) -> Result<BTreeMap<BinHash, String>, E> {
    let mut names = BTreeMap::new();
    for (key, name) in rows {
        let hash = name_hash(&name);
        if bin_key(hash) != key {
            return Err(E::custom(format_args!("{name:?} does not hash to {key}")));
        }
        names.insert(hash, name);
    }
    Ok(names)
}

impl Serialize for NameBook {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let paths = self
            .paths
            .iter()
            .map(|(&hash, path)| (wad_key(hash), path.clone()));
        let firsts = self.paths_by_name_hash.iter();
        let firsts = firsts.map(|(&name, &path)| (bin_key(name), wad_key(path)));
        Rows {
            paths: paths.collect(),
            paths_by_name_hash: firsts.collect(),
            entries: names_by_key(&self.entries),
            fields: names_by_key(&self.fields),
            classes: names_by_key(&self.classes),
            hashes: names_by_key(&self.hashes),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for NameBook {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let rows = Rows::deserialize(d)?;
        let mut book = NameBook::new();
        for (key, path) in rows.paths {
            let hash = path_hash(&path);
            if wad_key(hash) != key || path.to_ascii_lowercase() != path {
                return Err(D::Error::custom(format_args!(
                    "{path:?} is no path of {key}"
                )));
            }
            book.paths.insert(hash, path);
        }
        let by_key: BTreeMap<String, WadHash> = book
            .paths
            .keys()
            .map(|&hash| (wad_key(hash), hash))
            .collect();
        for (key, value) in rows.paths_by_name_hash {
            let hash = by_key.get(&value).copied();
            let first = hash.filter(|hash| bin_key(name_hash(&book.paths[hash])) == key);
            let Some(hash) = first else {
                return Err(D::Error::custom(format_args!(
                    "{value} is no path of {key}"
                )));
            };
            book.paths_by_name_hash
                .insert(name_hash(&book.paths[&hash]), hash);
        }
        book.entries = names_of(rows.entries)?;
        book.fields = names_of(rows.fields)?;
        book.classes = names_of(rows.classes)?;
        book.hashes = names_of(rows.hashes)?;
        Ok(book)
    }
}
