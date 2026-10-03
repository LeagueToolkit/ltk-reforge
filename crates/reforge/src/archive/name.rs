//! What an entry's name says the entry is. A name becomes a `WadName`, a `ChunkPath` or a hash
//! here, or it is refused, so the reader never holds a name as a path.

use crate::model::{ChunkPath, PathError, Spelled, WadHash, WadName};

/// What an entry under a WAD's name is.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum WadPart {
    /// `WAD/<wad>` as a file: the WAD itself.
    Packed,
    /// `WAD/<wad>/<path>`: a chunk at a path.
    Chunk(ChunkPath),
    /// `WAD/<wad>/<16 hex>[.ext]`: a chunk named by its path hash.
    Hashed(WadHash),
    /// `WAD/<wad>/<16 hex>.<number>`: a differing duplicate a repack kept beside the chunk.
    RepackDuplicate,
    /// A path that is no path inside a WAD.
    Unsafe(PathError),
}

/// What an entry's name says the entry is.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum EntryName {
    /// A directory.
    Directory,
    /// `META/info.json`.
    Info,
    /// An entry of a WAD.
    InWad {
        /// The WAD.
        wad: WadName,
        /// What of the WAD the entry is.
        part: WadPart,
    },
    /// An entry under a name that ends as a WAD's does and is no file name.
    UnsafeWad {
        /// The WAD's name as spelled.
        wad: Spelled,
        /// Why it is no file name.
        why: PathError,
    },
    /// Anything else.
    Extra,
}

impl EntryName {
    /// Reads a name whose separators are `/`. `META` and `WAD` match in any case, and a leading
    /// separator is dropped.
    pub(super) fn parse(name: &str) -> Self {
        let name = name.trim_start_matches('/');
        if name.ends_with('/') {
            return Self::Directory;
        }
        if name.eq_ignore_ascii_case("META/info.json") {
            return Self::Info;
        }
        let Some(rest) = name
            .get(..4)
            .filter(|prefix| prefix.eq_ignore_ascii_case("WAD/"))
            .map(|_| &name[4..])
        else {
            return Self::Extra;
        };
        let (wad, inner) = match rest.split_once('/') {
            Some((wad, inner)) => (wad, Some(inner)),
            None => (rest, None),
        };
        if !ends_as_a_wad(wad) {
            return Self::Extra;
        }
        match WadName::parse(wad) {
            Ok(wad) => Self::InWad {
                wad,
                part: inner.map_or(WadPart::Packed, WadPart::parse),
            },
            Err(why) => Self::UnsafeWad {
                wad: Spelled::new(wad),
                why,
            },
        }
    }
}

impl WadPart {
    /// Reads the part of a name after `WAD/<wad>/`.
    fn parse(inner: &str) -> Self {
        match hashed(inner) {
            Some((_, Some(_))) => Self::RepackDuplicate,
            Some((hash, None)) => Self::Hashed(hash),
            None => match ChunkPath::parse(inner) {
                Ok(path) => Self::Chunk(path),
                Err(why) => Self::Unsafe(why),
            },
        }
    }
}

/// Whether a name ends as a WAD's does: `.wad` as its last or next-to-last extension, so a
/// `Yuumi.wad.client.pyntex.json` sidecar is not taken for one. `WadName::parse` holds the same
/// rule and checks the characters first, which hides whether a refused name was a WAD's.
fn ends_as_a_wad(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let parts: Vec<&str> = lower.split('.').collect();
    let n = parts.len();
    n >= 2 && !parts[0].is_empty() && (parts[n - 1] == "wad" || (n >= 3 && parts[n - 2] == "wad"))
}

/// A chunk named by its hash (#MOD-ARTIFACT-0007): one file name of sixteen hex digits and an
/// optional extension. A numeric extension is a repack duplicate's (#MOD-ARTIFACT-0008), and
/// its number is returned.
fn hashed(inner: &str) -> Option<(WadHash, Option<u32>)> {
    if inner.contains('/') {
        return None;
    }
    let (stem, extension) = match inner.split_once('.') {
        Some((stem, extension)) => (stem, Some(extension)),
        None => (inner, None),
    };
    if stem.len() != 16 || !stem.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let hash = WadHash(u64::from_str_radix(stem, 16).ok()?);
    let duplicate = extension.and_then(|e| e.parse::<u32>().ok());
    Some((hash, duplicate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_ends_as_a_wad_by_its_last_two_extensions() {
        assert!(ends_as_a_wad("Lux.wad.client"));
        assert!(ends_as_a_wad("Lux.wad"));
        assert!(ends_as_a_wad("Lux.en_US.wad.client"));
        assert!(ends_as_a_wad("C:x.wad"));
        assert!(!ends_as_a_wad("Yuumi.wad.client.pyntex.json"));
        assert!(!ends_as_a_wad("readme.txt"));
        assert!(!ends_as_a_wad(".wad"));
    }

    #[test]
    fn a_hashed_name_gives_its_hash_and_its_repack_number() {
        let hash = WadHash(0x0123_4567_89ab_cdef);
        assert_eq!(hashed("0123456789abcdef"), Some((hash, None)));
        assert_eq!(hashed("0123456789ABCDEF.bin"), Some((hash, None)));
        assert_eq!(hashed("0123456789abcdef.1"), Some((hash, Some(1))));
        assert_eq!(hashed("data/characters/lux/skins/skin0.bin"), None);
        assert_eq!(hashed("0123456789abcdef.dir/x.bin"), None);
    }

    #[test]
    fn a_name_is_read_in_any_case_of_its_folders() {
        assert_eq!(EntryName::parse("meta/INFO.JSON"), EntryName::Info);
        assert_eq!(EntryName::parse("/META/info.json"), EntryName::Info);
        assert_eq!(
            EntryName::parse("WAD/Lux.wad.client/"),
            EntryName::Directory
        );
        assert_eq!(EntryName::parse("RAW/readme.txt"), EntryName::Extra);
        assert_eq!(EntryName::parse("WAD/readme.txt"), EntryName::Extra);
        assert_eq!(
            EntryName::parse("WAD/Lux.wad.client.pyntex.json"),
            EntryName::Extra
        );
        let lux = WadName::parse("Lux.wad.client").unwrap();
        assert_eq!(
            EntryName::parse("wad/Lux.wad.client"),
            EntryName::InWad {
                wad: lux.clone(),
                part: WadPart::Packed
            }
        );
        assert_eq!(
            EntryName::parse("Wad/Lux.wad.client/DATA/X.bin"),
            EntryName::InWad {
                wad: lux,
                part: WadPart::Chunk(ChunkPath::parse("data/x.bin").unwrap())
            }
        );
    }
}
