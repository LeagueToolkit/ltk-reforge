//! Mod archives for tests of `load`, and one archive taken through `read` and `load`.

use reforge::archive::{Archive, Caps};
use reforge::diag::{Diag, Diagnostics, Render, Stage};
use reforge::env::Env;
use reforge::game::{NameBook, Speller};
use reforge::load::{LoadError, LoadOptions, Loader};
use reforge::model::entity::{FileId, FileNode, UnitId};
use reforge::model::{Dump, DumpCx, Mod, path_hash};

use crate::archive::{STORED, packed_wad, zip};

/// The file name every archive of a test is read under.
pub const FILE_NAME: &str = "synthetic.fantome";

/// A mod archive built entry by entry: a `META/info.json`, then the chunks of its WADs in the
/// order given.
#[derive(Debug, Clone)]
pub struct ModArchive {
    entries: Vec<(String, Vec<u8>)>,
}

impl Default for ModArchive {
    fn default() -> Self {
        Self::new()
    }
}

impl ModArchive {
    /// An archive that holds its metadata and no WAD.
    pub fn new() -> Self {
        let info = br#"{"Name":"Synthetic","Author":"reforge tests","Version":"1.0.0"}"#;
        Self {
            entries: vec![("META/info.json".to_string(), info.to_vec())],
        }
    }

    /// Adds a chunk of a WAD, loose at its path.
    #[must_use]
    pub fn file(mut self, wad: &str, path: &str, bytes: impl Into<Vec<u8>>) -> Self {
        self.entries
            .push((format!("WAD/{wad}/{path}"), bytes.into()));
        self
    }

    /// Adds chunks of a WAD, each loose at its path.
    #[must_use]
    pub fn files(mut self, wad: &str, files: &[(&str, Vec<u8>)]) -> Self {
        for (path, bytes) in files {
            self = self.file(wad, path, bytes.clone());
        }
        self
    }

    /// Adds a WAD packed whole as one entry.
    #[must_use]
    pub fn packed(mut self, wad: &str, chunks: &[(&str, &[u8])]) -> Self {
        self.entries
            .push((format!("WAD/{wad}"), packed_wad(chunks)));
        self
    }

    /// Adds an entry outside the WADs.
    #[must_use]
    pub fn extra(mut self, name: &str, bytes: impl Into<Vec<u8>>) -> Self {
        self.entries.push((name.to_string(), bytes.into()));
        self
    }

    /// The archive's bytes: a zip of stored entries.
    pub fn build(&self) -> Vec<u8> {
        let entries: Vec<(&str, &[u8], _)> = self
            .entries
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice(), STORED))
            .collect();
        zip(&entries)
    }
}

/// One archive taken through `read` and `load`, with what the run raised and learnt.
pub struct LoadRun<'a> {
    env: &'a Env,
    /// The mod, or why there is none.
    pub result: Result<Mod, LoadError>,
    /// Every diagnostic of `read` and `load`.
    pub log: Diagnostics,
    /// The names the run learnt.
    pub names: NameBook,
}

impl<'a> LoadRun<'a> {
    /// Reads and loads an archive under options.
    ///
    /// # Panics
    ///
    /// When the archive does not read: a test of `load` builds one that does.
    pub fn of(env: &'a Env, options: LoadOptions, archive: &ModArchive) -> Self {
        Self::of_bytes(env, options, &archive.build())
    }

    /// Reads and loads the bytes of an archive under options.
    ///
    /// # Panics
    ///
    /// When the archive does not read.
    pub fn of_bytes(env: &'a Env, options: LoadOptions, bytes: &[u8]) -> Self {
        let mut log = Diagnostics::new();
        let read = Archive::read(bytes, FILE_NAME, &Caps::default(), &mut log.at(Stage::Read));
        let archive = read.unwrap_or_else(|halted| panic!("the archive reads: {halted}"));
        let mut loader = Loader::new(env).options(options);
        let result = loader.load(archive, &mut log.at(Stage::Load));
        Self {
            env,
            result,
            log,
            names: loader.into_names(),
        }
    }

    /// The mod.
    ///
    /// # Panics
    ///
    /// When `load` gave none, with what the run raised.
    pub fn module(&self) -> &Mod {
        match &self.result {
            Ok(module) => module,
            Err(error) => panic!("no mod: {error}\n{:#?}", self.messages()),
        }
    }

    /// Spells hashes as the run does.
    pub fn speller(&self) -> Speller<'_> {
        Speller::new(&self.names, self.env.names())
    }

    /// The code of every diagnostic, in the order raised.
    pub fn codes(&self) -> Vec<&'static str> {
        self.log.iter().map(|d| d.code.name).collect()
    }

    /// Every diagnostic as its code and its line for a person.
    pub fn messages(&self) -> Vec<String> {
        let speller = self.speller();
        let cx = Render {
            names: &speller,
            module: self.result.as_ref().ok(),
        };
        self.log
            .iter()
            .map(|d| format!("{} {}", d.code, d.message(&cx)))
            .collect()
    }

    /// The one finding of a type.
    ///
    /// # Panics
    ///
    /// When the run raised none or several of the type.
    pub fn only<T: Diag>(&self) -> &T {
        let mut found = self.log.of::<T>();
        match (found.next(), found.next()) {
            (Some(finding), None) => finding,
            _ => panic!("not one {}:\n{:#?}", T::CODE, self.messages()),
        }
    }

    /// The first file at a path, in any WAD.
    ///
    /// # Panics
    ///
    /// When the mod has no file at the path.
    pub fn file(&self, path: &str) -> (FileId, &FileNode) {
        let hash = path_hash(path);
        let mut files = self.module().files.iter();
        files
            .find(|(_, file)| file.hash == hash)
            .unwrap_or_else(|| panic!("no file at {path}\n{}", self.dump()))
    }

    /// The unit of a character at a slot.
    ///
    /// # Panics
    ///
    /// When the mod has no such unit.
    pub fn unit(&self, character: &str, slot: u16) -> UnitId {
        let m = self.module();
        let mut units = m.units.iter();
        let found = units.find(|(_, unit)| {
            m.characters[unit.character].name == character && unit.source_slot.0 == slot
        });
        found
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no unit {character} skin{slot}\n{}", self.dump()))
    }

    /// The mod as text, every file listed.
    pub fn dump(&self) -> String {
        let speller = self.speller();
        self.module().dump(&DumpCx::new(&speller).every_file())
    }
}
