//! The game install: the target build. Read only: nothing under it is written.
//!
//! The table of contents of every WAD is read once, champion WADs first, so a path resolves to
//! the first WAD that holds it. A WAD is mounted again the first time a chunk of it is read, and
//! stays mounted.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::sync::Mutex;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use ltk_wad::Wad;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{Build, Game, GameError};
use crate::diag::{Channel, Code, Diag, Input, Message, Severity, Sink, Subject};
use crate::model::{WadHash, WadName};

/// The file beside an install's `DATA` that states its build.
const METADATA: &str = "content-metadata.json";

/// The extension of the game's WADs. A `.wad.client.tmp` a patcher left is not one.
const WAD_SUFFIX: &str = ".wad.client";

/// The directory every champion's WAD sits in.
const CHAMPIONS: &str = "Champions";

/// Why an install does not open.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// The directory is none of the accepted forms.
    #[error("no DATA/FINAL/Champions under {root}")]
    NoData {
        /// The directory given.
        root: Utf8PathBuf,
    },
    /// A directory of the install does not list.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// A file of the install has a name that is not UTF-8.
    #[error("{} is not UTF-8", path.display())]
    PathNotUtf8 {
        /// The file.
        path: PathBuf,
    },
}

/// Why an install does not say its build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "why", rename_all = "kebab-case")]
pub enum NoBuild {
    /// No `content-metadata.json` beside the install's data.
    MetadataMissing,
    /// `content-metadata.json` does not read or is not the JSON expected.
    MetadataUnreadable {
        /// What failed.
        reason: String,
    },
    /// `content-metadata.json` has no `version`, or one that is no build.
    VersionMalformed {
        /// The version as written.
        version: Option<String>,
    },
}

/// A WAD of the install that holds nothing for the run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnreadableWad {
    /// The WAD's file, under `DATA/FINAL`.
    pub wad: String,
    /// What failed.
    pub reason: String,
}

/// WADs of the install whose table of contents does not read. The run sees no chunk of them.
#[derive(Debug, Clone, Serialize)]
pub struct WadsUnreadable {
    /// The WADs, in path order.
    pub wads: Vec<UnreadableWad>,
}

impl Diag for WadsUnreadable {
    const CODE: Code = Code::new("install.wad-unreadable", Channel::Run, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Input(Input::Install)
    }

    fn message(&self, f: &mut Message) {
        f.count(self.wads.len(), "WAD")
            .text(" of the install with a table of contents that does not read: ");
        f.list(&self.wads, |f, wad| {
            f.spelled(&wad.wad);
        });
    }
}

/// An install that does not say its build. The schema is then read at the newest build it
/// describes.
#[derive(Debug, Clone, Serialize)]
pub struct VersionUnknown {
    /// Why.
    #[serde(flatten)]
    pub why: NoBuild,
}

impl Diag for VersionUnknown {
    const CODE: Code = Code::new("install.version-unknown", Channel::Run, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Input(Input::Install)
    }

    fn message(&self, f: &mut Message) {
        f.text("the install does not say its build: ");
        match &self.why {
            NoBuild::MetadataMissing => f.text("no content-metadata.json"),
            NoBuild::MetadataUnreadable { reason } => f
                .text("content-metadata.json does not read: ")
                .spelled(reason),
            NoBuild::VersionMalformed { version: None } => {
                f.text("content-metadata.json has no version")
            }
            NoBuild::VersionMalformed {
                version: Some(version),
            } => f.text("the version ").spelled(version).text(" is no build"),
        };
    }
}

/// The chunks of each champion WAD by their uncompressed size. A byte copy of a file under
/// another path is found by reading the chunks of its size only.
#[derive(Debug, Clone, Default)]
pub struct SizeIndex {
    /// Lowercase WAD name, to size, to the chunks of that size in the order added.
    wads: HashMap<String, HashMap<u64, Vec<WadHash>>>,
}

impl SizeIndex {
    /// An index of no chunk.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a chunk of a WAD.
    pub fn add(&mut self, wad: &WadName, path: WadHash, size: u64) {
        self.wads
            .entry(wad.lower().to_string())
            .or_default()
            .entry(size)
            .or_default()
            .push(path);
    }

    /// Every chunk of a WAD with this uncompressed size, in the order added.
    pub fn same_size(&self, wad: &WadName, size: u64) -> &[WadHash] {
        self.wads
            .get(wad.lower())
            .and_then(|sizes| sizes.get(&size))
            .map_or(&[], Vec::as_slice)
    }
}

/// A WAD open for reading chunks.
type Mounted = Wad<BufReader<fs::File>>;

/// One WAD of the install.
struct InstallWad {
    path: Utf8PathBuf,
    name: WadName,
    champion: bool,
    /// One lock per WAD: reads of two WADs do not wait on each other. Reads of one WAD share
    /// its file and its decoder.
    mounted: Mutex<Option<Mounted>>,
}

impl InstallWad {
    /// A champion's WAD: under `Champions/`, named `<stem>.wad.client`. `lux.en_us.wad.client`
    /// is its voice in one language, not the champion's WAD.
    fn is_champion(path: &Utf8Path, name: &WadName) -> bool {
        let in_champions = path
            .parent()
            .and_then(Utf8Path::file_name)
            .is_some_and(|dir| dir.eq_ignore_ascii_case(CHAMPIONS));
        in_champions && name.lower().strip_suffix(WAD_SUFFIX) == Some(name.stem())
    }

    fn mount(&self) -> Result<Mounted, GameError> {
        let file = fs::File::open(&self.path)?;
        Wad::mount(BufReader::new(file)).map_err(|source| GameError::Mount {
            wad: self.path.clone(),
            source,
        })
    }

    /// The path hash and uncompressed size of every chunk.
    fn table_of_contents(&self) -> Result<Vec<(WadHash, u64)>, GameError> {
        let wad = self.mount()?;
        Ok(wad
            .chunks()
            .iter()
            .map(|chunk| (chunk.path_hash(), chunk.uncompressed_size() as u64))
            .collect())
    }

    fn read(&self, path: WadHash) -> Result<Vec<u8>, GameError> {
        let mut slot = self.mounted.lock().unwrap_or_else(|poisoned| {
            // A reader that a panic left in the middle of a read is dropped, and the WAD is
            // mounted again.
            self.mounted.clear_poison();
            let mut slot = poisoned.into_inner();
            *slot = None;
            slot
        });
        let wad = match slot.take() {
            Some(wad) => wad,
            None => self.mount()?,
        };
        let wad = slot.insert(wad);
        let chunk = *wad.chunks().get(path).ok_or_else(|| GameError::Changed {
            wad: self.path.clone(),
            path,
        })?;
        let bytes = wad
            .load_chunk_decompressed(&chunk)
            .map_err(|source| GameError::Chunk {
                wad: self.path.clone(),
                path,
                source,
            })?;
        Ok(bytes.into_vec())
    }
}

/// Where a path resolves to.
#[derive(Clone, Copy)]
struct Place {
    /// The first WAD holding the path, as an index into the install's WADs.
    wad: usize,
    /// The chunk's uncompressed size in that WAD.
    size: u64,
}

/// A game install on disk.
pub struct Install {
    root: Utf8PathBuf,
    build: Result<Build, NoBuild>,
    /// Champion WADs first, each group in path order.
    wads: Vec<InstallWad>,
    index: HashMap<WadHash, Place>,
    /// The later WADs holding a path the first WAD holds too.
    also: HashMap<WadHash, Vec<usize>>,
    sizes: SizeIndex,
    unreadable: Vec<UnreadableWad>,
}

impl Install {
    /// Opens the install at `root`: the install directory, its `Game` directory or `DATA/FINAL`.
    ///
    /// A WAD whose table of contents does not read holds nothing and is listed in
    /// [`unreadable`](Self::unreadable). It does not fail the open.
    pub fn open(root: &Utf8Path) -> Result<Self, InstallError> {
        let _span = tracing::info_span!("install", %root).entered();
        let data = Self::find_data(root).ok_or_else(|| InstallError::NoData {
            root: root.to_owned(),
        })?;
        let mut files = Vec::new();
        Self::collect_wads(&data, &mut files)?;
        files.sort();

        let within = |path: &Utf8Path| {
            path.strip_prefix(&data)
                .unwrap_or(path)
                .as_str()
                .replace('\\', "/")
        };
        let mut unreadable = Vec::new();
        let mut wads = Vec::with_capacity(files.len());
        for path in files {
            match WadName::parse(path.file_name().unwrap_or_default()) {
                Ok(name) => wads.push(InstallWad {
                    champion: InstallWad::is_champion(&path, &name),
                    path,
                    name,
                    mounted: Mutex::new(None),
                }),
                Err(error) => unreadable.push(UnreadableWad {
                    wad: within(&path),
                    reason: error.to_string(),
                }),
            }
        }
        // Stable, so each group stays in path order.
        wads.sort_by_key(|wad| !wad.champion);

        let tables: Vec<_> = wads.par_iter().map(InstallWad::table_of_contents).collect();
        let mut index: HashMap<WadHash, Place> = HashMap::new();
        let mut also: HashMap<WadHash, Vec<usize>> = HashMap::new();
        let mut sizes = SizeIndex::new();
        for (i, (wad, table)) in wads.iter().zip(tables).enumerate() {
            // An unreadable WAD stays in the list and holds nothing.
            let table = table.unwrap_or_else(|error| {
                unreadable.push(UnreadableWad {
                    wad: within(&wad.path),
                    reason: super::error_chain(&error),
                });
                Vec::new()
            });
            for (path, size) in table {
                match index.entry(path) {
                    Entry::Occupied(_) => also.entry(path).or_default().push(i),
                    Entry::Vacant(first) => {
                        first.insert(Place { wad: i, size });
                    }
                }
                if wad.champion {
                    sizes.add(&wad.name, path, size);
                }
            }
        }
        unreadable.sort_by(|a, b| a.wad.cmp(&b.wad));
        tracing::debug!(
            wads = wads.len(),
            chunks = index.len(),
            unreadable = unreadable.len(),
            "tables of contents read"
        );
        Ok(Self {
            root: root.to_owned(),
            build: Self::read_build(root),
            wads,
            index,
            also,
            sizes,
            unreadable,
        })
    }

    /// `DATA/FINAL` for a root that is the install directory, its `Game` directory or
    /// `DATA/FINAL` itself.
    fn find_data(root: &Utf8Path) -> Option<Utf8PathBuf> {
        [
            root.join("Game").join("DATA").join("FINAL"),
            root.join("DATA").join("FINAL"),
            root.to_owned(),
        ]
        .into_iter()
        .find(|dir| dir.join(CHAMPIONS).is_dir())
    }

    /// Every `.wad.client` under a directory.
    fn collect_wads(dir: &Utf8Path, out: &mut Vec<Utf8PathBuf>) -> Result<(), InstallError> {
        for entry in fs::read_dir(dir)? {
            let path = Utf8PathBuf::from_path_buf(entry?.path())
                .map_err(|path| InstallError::PathNotUtf8 { path })?;
            if path.is_dir() {
                Self::collect_wads(&path, out)?;
            } else if path
                .file_name()
                .is_some_and(|name| name.to_ascii_lowercase().ends_with(WAD_SUFFIX))
            {
                out.push(path);
            }
        }
        Ok(())
    }

    /// The build `content-metadata.json` states, looked for where each accepted form of the
    /// root has it: under `Game`, in the root, and two and three directories above `DATA/FINAL`.
    fn read_build(root: &Utf8Path) -> Result<Build, NoBuild> {
        #[derive(Deserialize)]
        struct Metadata {
            version: Option<String>,
        }

        let mut why = NoBuild::MetadataMissing;
        // The first file that fails says why, when no later one states a build.
        let mut fail = |reason: NoBuild| {
            if why == NoBuild::MetadataMissing {
                why = reason;
            }
        };
        for dir in [
            root.join("Game"),
            root.to_owned(),
            root.join("..").join(".."),
            root.join("..").join("..").join(".."),
        ] {
            let bytes = match fs::read(dir.join(METADATA)) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    fail(NoBuild::MetadataUnreadable {
                        reason: error.to_string(),
                    });
                    continue;
                }
            };
            match serde_json::from_slice::<Metadata>(&bytes) {
                Ok(Metadata { version }) => match version.as_deref().and_then(Build::parse) {
                    Some(build) => return Ok(build),
                    None => fail(NoBuild::VersionMalformed { version }),
                },
                Err(error) => fail(NoBuild::MetadataUnreadable {
                    reason: error.to_string(),
                }),
            }
        }
        Err(why)
    }

    /// The directory the install was opened at.
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// Why the install does not say its build, when it does not.
    pub fn no_build(&self) -> Option<&NoBuild> {
        self.build.as_ref().err()
    }

    /// How many WADs the install has, the unreadable ones included.
    pub fn wad_count(&self) -> usize {
        self.wads.len()
    }

    /// The champions' WADs, in path order.
    pub fn champion_wads(&self) -> impl Iterator<Item = &WadName> {
        self.wads
            .iter()
            .filter(|wad| wad.champion)
            .map(|wad| &wad.name)
    }

    /// The WADs that hold nothing for the run, in path order, each with what failed.
    pub fn unreadable(&self) -> &[UnreadableWad] {
        &self.unreadable
    }

    /// The chunks of each champion WAD by size.
    pub fn sizes(&self) -> &SizeIndex {
        &self.sizes
    }

    /// Raises what opening the install found: the WADs that do not read, and a build it does
    /// not say.
    pub fn report(&self, sink: &mut Sink<'_>) {
        if !self.unreadable.is_empty() {
            sink.raise(WadsUnreadable {
                wads: self.unreadable.clone(),
            });
        }
        if let Err(why) = &self.build {
            sink.raise(VersionUnknown { why: why.clone() });
        }
    }
}

impl fmt::Debug for Install {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Install")
            .field("root", &self.root)
            .field("build", &self.build)
            .field("wads", &self.wads.len())
            .field("chunks", &self.index.len())
            .finish()
    }
}

impl Game for Install {
    fn build(&self) -> Option<&Build> {
        self.build.as_ref().ok()
    }

    fn describe(&self) -> String {
        self.root.to_string()
    }

    fn size(&self, path: WadHash) -> Option<u64> {
        self.index.get(&path).map(|place| place.size)
    }

    fn read(&self, path: WadHash) -> Result<Option<Vec<u8>>, GameError> {
        let Some(place) = self.index.get(&path) else {
            return Ok(None);
        };
        self.wads[place.wad].read(path).map(Some)
    }

    fn wads(&self, path: WadHash) -> Vec<WadName> {
        let Some(place) = self.index.get(&path) else {
            return Vec::new();
        };
        let later = self.also.get(&path).into_iter().flatten().copied();
        std::iter::once(place.wad)
            .chain(later)
            .map(|wad| self.wads[wad].name.clone())
            .collect()
    }
}
