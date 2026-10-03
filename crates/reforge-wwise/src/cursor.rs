//! Reads off a slice of the file. Every read is checked against the bytes present and gives
//! `None` past them, so a length the file states never reaches an index or an allocation.

use std::iter;

/// A position in a slice, moved only by reads that fit.
pub(crate) struct Cur<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cur<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    /// The bytes not yet read.
    pub(crate) fn left(&self) -> usize {
        self.bytes.len() - self.pos
    }

    pub(crate) fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let out = self.bytes.get(self.pos..end)?;
        self.pos = end;
        Some(out)
    }

    /// The bytes of a length the file states.
    pub(crate) fn take_u32(&mut self, n: u32) -> Option<&'a [u8]> {
        self.take(usize::try_from(n).ok()?)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    pub(crate) fn tag(&mut self) -> Option<[u8; 4]> {
        self.array()
    }

    pub(crate) fn u8(&mut self) -> Option<u8> {
        self.array().map(|[b]| b)
    }

    pub(crate) fn u16(&mut self) -> Option<u16> {
        self.array().map(u16::from_le_bytes)
    }

    pub(crate) fn u32(&mut self) -> Option<u32> {
        self.array().map(u32::from_le_bytes)
    }

    /// Wwise's variable-size count: seven bits a byte, high bits first, the top bit continuing.
    pub(crate) fn var(&mut self) -> Option<u32> {
        let mut value = 0u32;
        for _ in 0..5 {
            let b = self.u8()?;
            value = (value << 7) | u32::from(b & 0x7f);
            if b & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }
}

/// The variable-size count as Wwise writes it.
pub(crate) fn put_var(out: &mut Vec<u8>, value: u32) {
    let mut groups = vec![(value & 0x7f) as u8];
    let mut rest = value >> 7;
    while rest > 0 {
        groups.push((rest & 0x7f) as u8 | 0x80);
        rest >>= 7;
    }
    out.extend(groups.iter().rev());
}

/// The little-endian `u32` at `at`, when its four bytes are there.
pub(crate) fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..)?
        .first_chunk()
        .copied()
        .map(u32::from_le_bytes)
}

/// The whole little-endian `u16`s of a slice, in order.
pub(crate) fn u16s(bytes: &[u8]) -> impl Iterator<Item = u16> {
    let mut cur = Cur::new(bytes);
    iter::from_fn(move || cur.u16())
}

/// The whole little-endian `u32`s of a slice, in order.
pub(crate) fn u32s(bytes: &[u8]) -> impl Iterator<Item = u32> {
    let mut cur = Cur::new(bytes);
    iter::from_fn(move || cur.u32())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn var_counts_round_trip() {
        for v in [0u32, 1, 127, 128, 300, 16_383, 16_384, 1 << 20, u32::MAX] {
            let mut out = Vec::new();
            put_var(&mut out, v);
            assert_eq!(Cur::new(&out).var(), Some(v), "{v}");
        }
    }
}
