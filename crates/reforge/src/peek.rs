//! A read of an archive alone (#ADR-0091): what a mod says of itself, the characters and
//! skins its paths name, and the package's thumbnail, with no game and no history. The read
//! is the stage `read` under its caps, into a memory store that goes with the peek, and the
//! thumbnail is the package stage's own encoding (#ADR-0051), so what is shown of an archive
//! is what a run makes of it.
//!
//! The characters are found by path, as the survey finds them: a WAD's stem and every
//! character a spelled skin bin path names, each with the slots its skin bins ship. What
//! needs the game, a champion guessed for a WAD named for none and a sub-character the
//! names list, waits on a run.

use std::collections::BTreeSet;
use std::fmt;

use camino::Utf8Path;

use crate::archive::{self, Caps, RawSnapshot, RawWad};
use crate::diag::{Diagnostics, Halted, Stage};
use crate::model::entity::ModMeta;
use crate::model::hash::skin_paths;
use crate::model::{MAX_SLOT, Slot, path_hash};
use crate::package::{Thumbnail, ThumbnailRefusal};
use crate::store::{BlobStore, MemoryStore};

/// A character an archive ships skin bins for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeekCharacter {
    /// Its name, in lowercase, as the paths spell it.
    pub name: String,
    /// The slots its skin bins ship, in order.
    pub slots: Vec<Slot>,
}

/// An archive read alone.
pub struct Peek {
    /// What the archive says of itself.
    pub meta: ModMeta,
    /// The characters its paths name, WAD by WAD in the archive's order, by name within a
    /// WAD.
    pub characters: Vec<PeekCharacter>,
    /// The thumbnail the package stage encodes from its image.
    pub thumbnail: Option<Thumbnail>,
    /// Why its image gives no thumbnail, when it has one that does not.
    pub image_refused: Option<ThumbnailRefusal>,
    /// What the read raised.
    pub log: Diagnostics,
}

/// A fatal diagnostic ended the read.
#[derive(thiserror::Error)]
#[error("the archive does not read")]
pub struct NotRead {
    /// The diagnostic.
    pub halted: Halted,
    /// The log that holds it.
    pub log: Diagnostics,
}

// The log holds a listener, which has no `Debug`.
impl fmt::Debug for Peek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Peek")
            .field("meta", &self.meta)
            .field("characters", &self.characters)
            .field("thumbnail", &self.thumbnail)
            .field("image_refused", &self.image_refused)
            .field("log", &self.log.len())
            .finish()
    }
}

impl fmt::Debug for NotRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NotRead")
            .field("halted", &self.halted)
            .field("log", &self.log.len())
            .finish()
    }
}

/// Reads an archive alone from its bytes, which are borrowed for the call. `file_name` is for
/// display and is never opened.
pub fn peek(bytes: &[u8], file_name: &str, caps: &Caps) -> Result<Peek, NotRead> {
    let mut log = Diagnostics::new();
    let store = MemoryStore::new();
    let read = archive::read(bytes, file_name, caps, &store, &mut log.at(Stage::Read));
    finish(read, &store, log)
}

/// Reads an archive file alone. A file over the cap for the archive file is not read.
pub fn peek_file(path: &Utf8Path, caps: &Caps) -> Result<Peek, NotRead> {
    let mut log = Diagnostics::new();
    let store = MemoryStore::new();
    let read = archive::read_file(path, caps, &store, &mut log.at(Stage::Read));
    finish(read, &store, log)
}

fn finish(
    read: Result<RawSnapshot, Halted>,
    store: &dyn BlobStore,
    log: Diagnostics,
) -> Result<Peek, NotRead> {
    let raw = match read {
        Ok(raw) => raw,
        Err(halted) => return Err(NotRead { halted, log }),
    };
    let image = raw.image.and_then(|image| image.open(store).ok());
    let (thumbnail, image_refused) = match image.as_ref().map(Thumbnail::of) {
        Some(Ok(thumbnail)) => (Some(thumbnail), None),
        Some(Err(why)) => (None, Some(why)),
        None => (None, None),
    };
    let characters = raw.wads.iter().flat_map(characters_of).collect();
    Ok(Peek {
        meta: raw.meta,
        characters,
        thumbnail,
        image_refused,
        log,
    })
}

/// The characters a WAD ships skin bins for: its stem and the characters its spelled paths
/// name, each with the slots found by the hash of their skin bin's path, so a chunk named by
/// its hash counts too.
fn characters_of(wad: &RawWad) -> Vec<PeekCharacter> {
    let mut names = BTreeSet::from([wad.name.stem().to_string()]);
    let spelled = wad.files.values().filter_map(|file| file.path.as_ref());
    for path in spelled {
        if let Some((character, _)) = skin_paths::parse_skin_bin(path.as_str()) {
            names.insert(character.to_ascii_lowercase());
        }
    }
    names
        .into_iter()
        .filter_map(|name| {
            let slots: Vec<Slot> = (0..=MAX_SLOT)
                .map(Slot)
                .filter(|&slot| {
                    let hash = path_hash(&skin_paths::skin_bin(&name, slot));
                    wad.files.contains_key(&hash)
                })
                .collect();
            (!slots.is_empty()).then_some(PeekCharacter { name, slots })
        })
        .collect()
}
