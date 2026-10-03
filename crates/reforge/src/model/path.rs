//! Paths a writer may use. A name spelled by a mod becomes one of these or is refused, so no
//! entry name reaches the file system as written.

use std::fmt;
use std::sync::Arc;

use super::hash::{WadHash, path_hash};
use super::text::is_unsafe;

/// Why a name spelled by a mod is no path a writer may use.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// The name is empty.
    #[error("the name is empty")]
    Empty,
    /// The name is longer than any path the game has.
    #[error("the name is {0} bytes long, over the limit of {MAX_PATH_BYTES}")]
    TooLong(usize),
    /// A part between two separators is empty: a leading, trailing or doubled separator.
    #[error("the name has an empty part")]
    EmptyPart,
    /// A part is `.` or `..`.
    #[error("the name has the part {0:?}")]
    DotPart(String),
    /// A character no file name may hold on every system a package is written on.
    #[error("the name holds the character {0:?}")]
    Character(char),
    /// A part ends in a dot or a space, which Windows drops.
    #[error("the part {0:?} ends in a dot or a space")]
    TrailingDotOrSpace(String),
    /// A part is a device name on Windows.
    #[error("the part {0:?} is a device name")]
    Device(String),
    /// A WAD's name holds a separator.
    #[error("a WAD's name is one file name, and {0:?} holds a separator")]
    Separator(String),
    /// A WAD's name does not have `.wad` as its last or next-to-last extension.
    #[error("{0:?} is not the name of a WAD")]
    NotAWad(String),
}

/// The longest path accepted. A bin the game shares between skins is named for every skin it
/// serves: of the 2.3 million paths the hash cache spells for 16.19, 1,264 are longer than 512
/// bytes and the longest is 2,454 (2026-10-04). One part of such a path is longer than a file
/// name on disk may be, so a writer names such a chunk by its hash (#MOD-ARTIFACT-0007).
pub const MAX_PATH_BYTES: usize = 4096;

/// Device names Windows reserves, whatever extension follows them.
const DEVICES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Checks one part of a path: a file or directory name with no separator in it.
fn check_part(part: &str) -> Result<(), PathError> {
    if part.is_empty() {
        return Err(PathError::EmptyPart);
    }
    if part == "." || part == ".." {
        return Err(PathError::DotPart(part.to_string()));
    }
    // A colon is a drive or a stream on Windows; the rest are wildcards, quotes and redirections.
    if let Some(c) = part
        .chars()
        .find(|&c| is_unsafe(c) || c < ' ' || matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*'))
    {
        return Err(PathError::Character(c));
    }
    if part.ends_with('.') || part.ends_with(' ') {
        return Err(PathError::TrailingDotOrSpace(part.to_string()));
    }
    let stem = part.split('.').next().unwrap_or(part);
    if DEVICES.iter().any(|d| stem.eq_ignore_ascii_case(d)) {
        return Err(PathError::Device(part.to_string()));
    }
    Ok(())
}

/// A path inside a WAD: lowercase, parts joined by `/`, no part that leaves the directory it is
/// written under.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkPath(Arc<str>);

impl ChunkPath {
    /// The path a name spells, lowercased, with `\` read as `/`.
    pub fn parse(spelled: &str) -> Result<Self, PathError> {
        if spelled.is_empty() {
            return Err(PathError::Empty);
        }
        if spelled.len() > MAX_PATH_BYTES {
            return Err(PathError::TooLong(spelled.len()));
        }
        let path = spelled.replace('\\', "/").to_ascii_lowercase();
        for part in path.split('/') {
            check_part(part)?;
        }
        Ok(Self(path.into()))
    }

    /// The path of a chunk no table names: its hash as sixteen hex digits, with an extension.
    pub fn of_hash(hash: WadHash, extension: &str) -> Result<Self, PathError> {
        Self::parse(&format!("{:016x}.{extension}", hash.0))
    }

    /// The path as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The hash the game finds the chunk by.
    pub fn hash(&self) -> WadHash {
        path_hash(&self.0)
    }

    /// The parts of the path, in order.
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// The file name's extension, without the dot.
    pub fn extension(&self) -> Option<&str> {
        let name = self.0.rsplit('/').next()?;
        name.rsplit_once('.')
            .map(|(_, ext)| ext)
            .filter(|ext| !ext.is_empty())
    }
}

impl fmt::Display for ChunkPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for ChunkPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", &*self.0)
    }
}

impl serde::Serialize for ChunkPath {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

/// The name of a WAD: one file name, such as `Lux.wad.client` or `Lux.en_US.wad.client`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WadName {
    spelled: Arc<str>,
    lower: Arc<str>,
    stem_len: usize,
    locale: Option<Arc<str>>,
}

impl WadName {
    /// The WAD a name spells. `.wad` is its last or next-to-last extension, so a
    /// `Yuumi.wad.client.pyntex.json` sidecar is not taken for one.
    pub fn parse(spelled: &str) -> Result<Self, PathError> {
        if spelled.is_empty() {
            return Err(PathError::Empty);
        }
        if spelled.len() > MAX_PATH_BYTES {
            return Err(PathError::TooLong(spelled.len()));
        }
        if spelled.contains(['/', '\\']) {
            return Err(PathError::Separator(spelled.to_string()));
        }
        check_part(spelled)?;
        let lower = spelled.to_ascii_lowercase();
        let parts: Vec<&str> = lower.split('.').collect();
        let n = parts.len();
        let is_wad = n >= 2
            && !parts[0].is_empty()
            && (parts[n - 1] == "wad" || (n >= 3 && parts[n - 2] == "wad"));
        if !is_wad {
            return Err(PathError::NotAWad(spelled.to_string()));
        }
        // `<stem>.<xx_XX>.wad.client`: the part before `.wad` is a locale when it is shaped like one.
        let wad_at = if parts[n - 1] == "wad" { n - 1 } else { n - 2 };
        let locale = (wad_at == 2)
            .then(|| parts[1])
            .filter(|p| is_locale(p))
            .map(Arc::from);
        let stem_len = parts[0].len();
        Ok(Self {
            spelled: spelled.into(),
            lower: lower.into(),
            stem_len,
            locale,
        })
    }

    /// The name as the mod or the game spells it.
    pub fn as_str(&self) -> &str {
        &self.spelled
    }

    /// The name in lowercase.
    pub fn lower(&self) -> &str {
        &self.lower
    }

    /// The name up to its first dot, in lowercase: the champion, for a champion's WAD.
    pub fn stem(&self) -> &str {
        &self.lower[..self.stem_len]
    }

    /// The locale of a locale WAD, in lowercase: `en_us`.
    pub fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }
}

/// Whether a lowercase part is shaped like a locale, `xx_xx`.
fn is_locale(part: &str) -> bool {
    let b = part.as_bytes();
    b.len() == 5 && b[2] == b'_' && [0, 1, 3, 4].iter().all(|&i| b[i].is_ascii_lowercase())
}

impl fmt::Display for WadName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.spelled)
    }
}

impl fmt::Debug for WadName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", &*self.spelled)
    }
}

impl serde::Serialize for WadName {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.spelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_game_path_is_lowercased_and_hashes_as_the_game_hashes_it() {
        let path = ChunkPath::parse("ASSETS\\Characters/Lux/Skins/Base/Lux.skn").unwrap();
        assert_eq!(path.as_str(), "assets/characters/lux/skins/base/lux.skn");
        assert_eq!(
            path.hash(),
            path_hash("assets/characters/lux/skins/base/lux.skn")
        );
        assert_eq!(path.extension(), Some("skn"));
    }

    #[test]
    fn a_path_that_leaves_its_directory_is_refused() {
        assert_eq!(
            ChunkPath::parse("../x.dds"),
            Err(PathError::DotPart("..".into()))
        );
        assert_eq!(
            ChunkPath::parse("a/../../x.dds"),
            Err(PathError::DotPart("..".into()))
        );
        assert_eq!(ChunkPath::parse("/abs/x.dds"), Err(PathError::EmptyPart));
        assert_eq!(ChunkPath::parse("a//x.dds"), Err(PathError::EmptyPart));
        assert_eq!(ChunkPath::parse("a/x.dds/"), Err(PathError::EmptyPart));
        assert_eq!(
            ChunkPath::parse("c:/windows/x.dds"),
            Err(PathError::Character(':'))
        );
        assert_eq!(
            ChunkPath::parse("a/x.dds:stream"),
            Err(PathError::Character(':'))
        );
        assert_eq!(
            ChunkPath::parse("a/nul.dds"),
            Err(PathError::Device("nul.dds".into()))
        );
        assert_eq!(
            ChunkPath::parse("a/x.dds."),
            Err(PathError::TrailingDotOrSpace("x.dds.".into()))
        );
        assert_eq!(
            ChunkPath::parse("a/x\u{1b}.dds"),
            Err(PathError::Character('\u{1b}'))
        );
        assert_eq!(ChunkPath::parse(""), Err(PathError::Empty));
        assert!(matches!(
            ChunkPath::parse(&"a/".repeat(2400)),
            Err(PathError::TooLong(4800))
        ));
    }

    #[test]
    fn a_bin_shared_between_many_skins_has_a_path() {
        // As the game names one: every skin it serves, in one file name.
        let skins: String = (0..120).map(|n| format!("_skins_skin{n}")).collect();
        let name = format!("data/characters/ahri/ahri_multi{skins}.bin");
        assert!(name.len() > 1500);
        let path = ChunkPath::parse(&name).unwrap();
        assert_eq!(path.hash(), path_hash(&name));
    }

    #[test]
    fn a_chunk_no_table_names_is_its_hash() {
        let path = ChunkPath::of_hash(WadHash(0x9455_3db9_9d45_5089), "bin").unwrap();
        assert_eq!(path.as_str(), "94553db99d455089.bin");
    }

    #[test]
    fn a_wad_name_gives_its_stem_and_locale() {
        let wad = WadName::parse("Lux.wad.client").unwrap();
        assert_eq!(
            (wad.as_str(), wad.stem(), wad.locale()),
            ("Lux.wad.client", "lux", None)
        );
        let wad = WadName::parse("Lux.en_US.wad.client").unwrap();
        assert_eq!((wad.stem(), wad.locale()), ("lux", Some("en_us")));
        let wad = WadName::parse("Shaco.wad").unwrap();
        assert_eq!((wad.stem(), wad.locale()), ("shaco", None));
        assert_eq!(
            WadName::parse("Demon Blitzcrank.wad.client")
                .unwrap()
                .stem(),
            "demon blitzcrank"
        );
    }

    #[test]
    fn a_name_that_is_no_wad_or_no_file_name_is_refused() {
        assert_eq!(
            WadName::parse("Yuumi.wad.client.pyntex.json"),
            Err(PathError::NotAWad("Yuumi.wad.client.pyntex.json".into()))
        );
        assert_eq!(WadName::parse("C:x.wad"), Err(PathError::Character(':')));
        assert_eq!(
            WadName::parse("a/Lux.wad.client"),
            Err(PathError::Separator("a/Lux.wad.client".into()))
        );
        assert_eq!(
            WadName::parse("..wad"),
            Err(PathError::NotAWad("..wad".into()))
        );
    }
}
