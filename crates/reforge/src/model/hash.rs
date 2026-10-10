//! The hashes and small value types every module shares.

use std::fmt;

use ltk_hash::Hash as _;
pub use ltk_hash::{BinHash, WadHash};
use sha2::{Digest, Sha256};

/// A skin number: `skin<N>.bin`, `Skins/Skin<N>`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct Slot(pub u16);

impl Slot {
    /// The base skin, which every player has.
    pub const BASE: Slot = Slot(0);

    /// Whether this is the base skin.
    pub fn is_base(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The highest skin number probed anywhere. Riot's highest skin number is below it.
pub const MAX_SLOT: u16 = 120;

/// The SHA-256 of a file's bytes, which every comparison and history lookup starts from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentId(pub [u8; 32]);

impl ContentId {
    /// The content id of `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// The digest as 64 hex digits.
    pub fn hex(&self) -> String {
        hex(&self.0)
    }
}

impl fmt::Debug for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.hex()[..16])
    }
}

impl serde::Serialize for ContentId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> serde::Deserialize<'de> for ContentId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(d)?;
        let digest = hex_digits(&text, 64).map(|digits| {
            let mut digest = [0u8; 32];
            for (byte, pair) in digest.iter_mut().zip(digits.chunks(2)) {
                *byte = pair.iter().fold(0, |byte, &digit| byte << 4 | digit);
            }
            digest
        });
        digest
            .map(Self)
            .ok_or_else(|| serde::de::Error::custom(format_args!("{text:?} is no SHA-256")))
    }
}

/// The values of `len` hex digits, in either case. None when the text is of another length or
/// holds anything else.
fn hex_digits(text: &str, len: usize) -> Option<Vec<u8>> {
    let digits = text
        .chars()
        .map(|c| c.to_digit(16).and_then(|d| u8::try_from(d).ok()));
    let digits: Vec<u8> = digits.collect::<Option<_>>()?;
    (digits.len() == len).then_some(digits)
}

/// The number `len` hex digits spell.
fn hex_number(text: &str, len: usize) -> Option<u64> {
    let digits = hex_digits(text, len)?;
    Some(digits.iter().fold(0u64, |n, &d| n << 4 | u64::from(d)))
}

/// The chunk hash sixteen hex digits spell, as a report writes one.
pub fn wad_hash_of_hex(text: &str) -> Option<WadHash> {
    hex_number(text, 16).map(WadHash)
}

/// A chunk hash as a report writes it: sixteen hex digits. For `#[serde(with)]`.
pub mod wad_hex {
    use super::{WadHash, hex_number};

    /// Writes the hash.
    pub fn serialize<S: serde::Serializer>(hash: &WadHash, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&format_args!("{:016x}", hash.0))
    }

    /// Reads the hash.
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<WadHash, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(d)?;
        hex_number(&text, 16)
            .map(WadHash)
            .ok_or_else(|| serde::de::Error::custom(format_args!("{text:?} is no chunk hash")))
    }
}

/// Chunk hashes as a report writes them: each sixteen hex digits. For `#[serde(with)]`.
pub mod wad_hexes {
    use serde::de::Error as _;

    use super::{WadHash, hex_number};

    /// Writes the hashes.
    pub fn serialize<S: serde::Serializer>(hashes: &[WadHash], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(hashes.iter().map(|hash| format!("{:016x}", hash.0)))
    }

    /// Reads the hashes.
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<WadHash>, D::Error> {
        let texts = <Vec<String> as serde::Deserialize>::deserialize(d)?;
        let hash = |text: &String| {
            hex_number(text, 16)
                .map(WadHash)
                .ok_or_else(|| D::Error::custom(format_args!("{text:?} is no chunk hash")))
        };
        texts.iter().map(hash).collect()
    }
}

/// A bin hash as a report writes it: eight hex digits. For `#[serde(with)]`.
pub mod bin_hex {
    use super::{BinHash, hex_number};

    /// Writes the hash.
    pub fn serialize<S: serde::Serializer>(hash: &BinHash, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&format_args!("{:08x}", hash.0))
    }

    /// Reads the hash.
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BinHash, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(d)?;
        let hash = hex_number(&text, 8).and_then(|n| u32::try_from(n).ok());
        hash.map(BinHash)
            .ok_or_else(|| serde::de::Error::custom(format_args!("{text:?} is no bin hash")))
    }
}

/// Bytes as lowercase hex digits.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// The chunk hash of a path, lowercased.
pub fn path_hash(path: &str) -> WadHash {
    WadHash::hash_str(path)
}

/// The FNV-1a hash of an entry, field or class name, lowercased.
pub fn name_hash(name: &str) -> BinHash {
    BinHash::hash_str(name)
}

/// Paths and entry names of a character's skins.
pub mod skin_paths {
    use super::Slot;

    /// `data/characters/<c>/skins/skin<N>.bin`.
    pub fn skin_bin(character: &str, slot: Slot) -> String {
        format!(
            "data/characters/{}/skins/skin{}.bin",
            character.to_ascii_lowercase(),
            slot.0
        )
    }

    /// `data/characters/<c>/animations/skin<N>.bin`.
    pub fn animation_bin(character: &str, slot: Slot) -> String {
        format!(
            "data/characters/{}/animations/skin{}.bin",
            character.to_ascii_lowercase(),
            slot.0
        )
    }

    /// `data/characters/<c>/<c>.bin`.
    pub fn root_bin(character: &str) -> String {
        format!(
            "data/characters/{0}/{0}.bin",
            character.to_ascii_lowercase()
        )
    }

    /// `Characters/<C>/Skins/Skin<N>`. Entry hashes ignore case, so the spelling is cosmetic.
    pub fn skin_entry(character: &str, slot: Slot) -> String {
        format!("Characters/{character}/Skins/Skin{}", slot.0)
    }

    /// `Characters/<C>/Skins/Skin<N>/Resources`.
    pub fn resolver_entry(character: &str, slot: Slot) -> String {
        format!("Characters/{character}/Skins/Skin{}/Resources", slot.0)
    }

    /// `Characters/<C>/Animations/Skin<N>`.
    pub fn graph_entry(character: &str, slot: Slot) -> String {
        format!("Characters/{character}/Animations/Skin{}", slot.0)
    }

    /// The character and slot of a skin bin path, `data/characters/<c>/skins/skin<N>.bin`.
    pub fn parse_skin_bin(path: &str) -> Option<(String, Slot)> {
        let rest = path.strip_prefix("data/characters/")?;
        let (character, rest) = rest.split_once('/')?;
        let number = rest.strip_prefix("skins/skin")?.strip_suffix(".bin")?;
        if character.is_empty() || number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        // `skin07.bin` is not a name the client asks for.
        if number.len() > 1 && number.starts_with('0') {
            return None;
        }
        Some((character.to_string(), Slot(number.parse().ok()?)))
    }

    /// The character and slot a path under a skin folder names:
    /// `assets/characters/<c>/skins/skin07/...` or `.../skins/skin7/...`. Riot's own skins reuse
    /// other champions' files, so a caller asks whose folder it is before reading anything into
    /// the slot.
    pub fn skin_folder(path: &str) -> Option<(String, Slot)> {
        let lower = path.to_ascii_lowercase();
        let slot = slot_in_path(&lower)?;
        let before = &lower[..lower.find("/skins/skin")?];
        let character = before.rsplit('/').next().filter(|c| !c.is_empty())?;
        before
            .contains("characters/")
            .then(|| (character.to_string(), slot))
    }

    /// The slot a path under a skin folder names, whoever's folder it is:
    /// `.../skins/skin07/...` or `.../skins/skin7/...`.
    pub fn slot_in_path(path: &str) -> Option<Slot> {
        let lower = path.to_ascii_lowercase();
        let at = lower.find("/skins/skin")? + "/skins/skin".len();
        let digits: String = lower[at..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if digits.is_empty() {
            return None;
        }
        let after = lower[at + digits.len()..].chars().next();
        if !matches!(after, Some('/' | '_' | '.')) {
            return None;
        }
        digits.parse().ok().map(Slot)
    }
}

#[cfg(test)]
mod tests {
    use super::skin_paths::*;
    use super::*;

    #[test]
    fn hex_reads_back_what_it_wrote_and_nothing_else() {
        let id = ContentId::of(b"bytes");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(serde_json::from_str::<ContentId>(&json).unwrap(), id);
        assert_eq!(hex_number("00000000000000ff", 16), Some(255));
        assert_eq!(hex_number("000000FF", 8), Some(255));
        for wrong in [
            "+00000000000000f",
            "00000000000000f",
            "00000000000000fg",
            "ffffffffffffffé",
        ] {
            assert_eq!(hex_number(wrong, 16), None, "{wrong}");
        }
        let short = serde_json::to_string(&id.hex()[1..]).unwrap();
        assert!(serde_json::from_str::<ContentId>(&short).is_err());
    }

    #[test]
    fn skin_bin_paths_round_trip() {
        assert_eq!(
            parse_skin_bin("data/characters/lux/skins/skin7.bin"),
            Some(("lux".into(), Slot(7)))
        );
        assert_eq!(
            parse_skin_bin(&skin_bin("Lux", Slot(0))),
            Some(("lux".into(), Slot(0)))
        );
        assert_eq!(parse_skin_bin("data/characters/lux/skins/skin07.bin"), None);
        assert_eq!(parse_skin_bin("data/characters/lux/skins/base.bin"), None);
    }

    #[test]
    fn a_skin_folder_names_its_character() {
        assert_eq!(
            skin_folder("ASSETS/Characters/RekSai/Skins/Skin26/Particles/x.dds"),
            Some(("reksai".into(), Slot(26)))
        );
        assert_eq!(
            skin_folder("assets/characters/lux/skins/base/lux.skn"),
            None
        );
        assert_eq!(skin_folder("assets/shared/skins/skin3/x.dds"), None);
    }

    #[test]
    fn slot_in_a_skin_folder() {
        assert_eq!(
            slot_in_path("ASSETS/Characters/Lux/Skins/Skin07/Lux_Skin07_TX_CM.tex"),
            Some(Slot(7))
        );
        assert_eq!(
            slot_in_path("assets/characters/lux/skins/base/lux.skn"),
            None
        );
        assert_eq!(
            slot_in_path("assets/characters/lux/skins/skin15_extra/x.tex"),
            Some(Slot(15))
        );
    }
}
