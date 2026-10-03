//! A game in memory, for tests: every chunk in one champion WAD, unless the chunk is put in WADs
//! it names.

use std::collections::BTreeMap;

use reforge::game::{Build, Game, GameError};
use reforge::model::{WadHash, WadName, path_hash};

/// A chunk store with no file behind it.
pub struct MemoryGame {
    build: Option<Build>,
    wad: WadName,
    chunks: BTreeMap<WadHash, Vec<u8>>,
    /// The WADs holding a chunk that is not in the one champion WAD alone, the first first.
    wads: BTreeMap<WadHash, Vec<WadName>>,
}

fn wad_name(name: &str) -> WadName {
    WadName::parse(name).unwrap_or_else(|error| panic!("{name:?} is no WAD name: {error}"))
}

impl MemoryGame {
    /// A game of one champion WAD at a build.
    pub fn new(wad: &str, build: &str) -> Self {
        Self {
            build: Some(
                Build::parse(build).unwrap_or_else(|| panic!("{build:?} is no build of the game")),
            ),
            wad: wad_name(wad),
            chunks: BTreeMap::new(),
            wads: BTreeMap::new(),
        }
    }

    /// Puts a chunk in the champion WAD.
    pub fn insert(&mut self, path: &str, bytes: Vec<u8>) {
        self.chunks.insert(path_hash(path), bytes);
    }

    /// Puts a chunk in the WADs named, the first being the one it is read from.
    pub fn insert_in(&mut self, path: &str, bytes: Vec<u8>, wads: &[&str]) {
        self.insert(path, bytes);
        self.wads.insert(
            path_hash(path),
            wads.iter().map(|name| wad_name(name)).collect(),
        );
    }
}

impl Game for MemoryGame {
    fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    fn describe(&self) -> String {
        "memory".to_string()
    }

    fn size(&self, path: WadHash) -> Option<u64> {
        self.chunks.get(&path).map(|bytes| bytes.len() as u64)
    }

    fn read(&self, path: WadHash) -> Result<Option<Vec<u8>>, GameError> {
        Ok(self.chunks.get(&path).cloned())
    }

    fn wads(&self, path: WadHash) -> Vec<WadName> {
        if !self.chunks.contains_key(&path) {
            return Vec::new();
        }
        self.wads
            .get(&path)
            .cloned()
            .unwrap_or_else(|| vec![self.wad.clone()])
    }
}
