//! The keys the history is asked by. Each is the head of a SHA-256 in digest order: a file's and
//! an object's leading sixteen bytes, a clip's and a norm key's leading eight, read big-endian so
//! that the integer's hex is what the census history writes.

use std::fmt;

/// A file's key: the head of its bytes' digest.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileKey(pub [u8; 16]);

/// An object's key: the head of the digest of its bytes from its entry hash to its end.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKey(pub [u8; 16]);

/// An object's norm key: the head of the digest of its bytes after its entry hash. The same
/// object under another entry name has the same key.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NormKey(pub u64);

/// A clip's key: the head of its wem bytes' digest.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClipKey(pub u64);

fn head16(digest: &[u8; 32]) -> [u8; 16] {
    let mut out = [0; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn head8(digest: &[u8; 32]) -> u64 {
    let mut head = [0; 8];
    head.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(head)
}

/// The leading sixteen bytes of a whole digest written as 64 hex digits.
fn parse16(text: &str) -> Option<[u8; 16]> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn parse8(text: &str) -> Option<u64> {
    if text.len() != 16 {
        return None;
    }
    u64::from_str_radix(text, 16).ok()
}

impl FileKey {
    /// The key of a file whose bytes have this digest.
    pub fn from_sha256(digest: &[u8; 32]) -> Self {
        Self(head16(digest))
    }

    /// The key from a whole digest as the census history writes it, 64 hex digits.
    pub fn parse(text: &str) -> Option<Self> {
        parse16(text).map(Self)
    }
}

impl ObjectKey {
    /// The key of an object whose bytes, from its entry hash to its end, have this digest.
    pub fn from_sha256(digest: &[u8; 32]) -> Self {
        Self(head16(digest))
    }

    /// The key from a whole digest as the census history writes it, 64 hex digits.
    pub fn parse(text: &str) -> Option<Self> {
        parse16(text).map(Self)
    }
}

impl NormKey {
    /// The key of an object whose bytes after its entry hash have this digest.
    pub fn from_sha256(digest: &[u8; 32]) -> Self {
        Self(head8(digest))
    }

    /// The key as the census history writes it, 16 hex digits.
    pub fn parse(text: &str) -> Option<Self> {
        parse8(text).map(Self)
    }
}

impl ClipKey {
    /// The key of a clip whose wem bytes have this digest.
    pub fn from_sha256(digest: &[u8; 32]) -> Self {
        Self(head8(digest))
    }

    /// The key as the census history writes it, 16 hex digits.
    pub fn parse(text: &str) -> Option<Self> {
        parse8(text).map(Self)
    }
}

macro_rules! hex_bytes {
    ($t:ty) => {
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for b in self.0 {
                    write!(f, "{b:02x}")?;
                }
                Ok(())
            }
        }
        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self, f)
            }
        }
    };
}
hex_bytes!(FileKey);
hex_bytes!(ObjectKey);

macro_rules! hex_number {
    ($t:ty) => {
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:016x}", self.0)
            }
        }
        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self, f)
            }
        }
    };
}
hex_number!(NormKey);
hex_number!(ClipKey);

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    fn abc() -> [u8; 32] {
        Sha256::digest(b"abc").into()
    }

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn file_and_object_keys_are_the_digest_head() {
        assert_eq!(
            FileKey::from_sha256(&abc()).to_string(),
            "ba7816bf8f01cfea414140de5dae2223"
        );
        assert_eq!(
            ObjectKey::from_sha256(&abc()).to_string(),
            "ba7816bf8f01cfea414140de5dae2223"
        );
        assert_eq!(FileKey::parse(ABC), Some(FileKey::from_sha256(&abc())));
        assert_eq!(FileKey::parse(&ABC[..32]), None);
    }

    #[test]
    fn clip_and_norm_keys_are_the_digest_head_in_digest_order() {
        assert_eq!(ClipKey::from_sha256(&abc()).to_string(), "ba7816bf8f01cfea");
        assert_eq!(
            ClipKey::parse("ba7816bf8f01cfea"),
            Some(ClipKey::from_sha256(&abc()))
        );
        assert_eq!(NormKey::from_sha256(&abc()), NormKey(0xba78_16bf_8f01_cfea));
        assert_eq!(NormKey::parse("ba7816bf8f01cfe"), None);
    }
}
