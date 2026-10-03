//! The bank as read: its header, its clips, its objects, and where each section sat.

use crate::cursor::u32_at;
use crate::{BankError, CLIENT_VERSION, Object, is_bnk, is_wpk, reader, writer};

/// One clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wem {
    /// The id sounds name the clip by.
    pub id: u32,
    /// The `.wem` file.
    pub bytes: Vec<u8>,
}

/// The file format of a bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankKind {
    /// A sound bank, `.bnk`.
    Bnk,
    /// A package of clips, `.wpk`.
    Wpk,
}

/// Where each section sat, so a bank writes back in its own order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Part {
    Header,
    /// `DIDX` and `DATA`, written from `media`.
    Index,
    Data,
    Hirc,
    Other([u8; 4], Vec<u8>),
}

/// A sound bank or a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bank {
    /// The format the bank was read from and writes to.
    pub kind: BankKind,
    /// The bank's version, or the package's.
    pub version: u32,
    /// The `BKHD` body; empty for a package.
    pub header: Vec<u8>,
    /// The clips, in the order of their index.
    pub media: Vec<Wem>,
    /// The `HIRC` objects, in order.
    pub objects: Vec<Object>,
    /// The alignment of clips in `DATA`.
    pub alignment: usize,
    pub(crate) parts: Vec<Part>,
}

/// Where the bank id sits in a `BKHD` body.
const BANK_ID_AT: usize = 4;

/// The shortest `BKHD` body: the version and the bank id.
pub(crate) const HEADER_MIN: usize = BANK_ID_AT + 4;

/// The clips' alignment in a bank whose `BKHD` states none.
pub(crate) const DEFAULT_ALIGNMENT: usize = 16;

/// The alignment of a package's entries and clips.
pub(crate) const PACKAGE_ALIGNMENT: usize = 8;

/// The clips' alignment a `BKHD` body states.
pub(crate) fn alignment_of(header: &[u8]) -> usize {
    let stated = header.get(12..).and_then(|b| b.first_chunk().copied());
    match stated.map(u16::from_le_bytes) {
        Some(a) if a > 0 => usize::from(a),
        _ => DEFAULT_ALIGNMENT,
    }
}

impl Bank {
    /// The bank or the package the bytes hold.
    pub fn read(bytes: &[u8]) -> Result<Bank, BankError> {
        if is_wpk(bytes) {
            reader::read_wpk(bytes)
        } else if is_bnk(bytes) {
            reader::read_bnk(bytes)
        } else {
            let head = bytes[..bytes.len().min(8)].to_vec();
            Err(BankError::NotABank {
                len: bytes.len(),
                head,
            })
        }
    }

    /// A new bank of the given header (whose version it takes), with no media and no objects. The
    /// sections a bank of this kind writes: `BKHD`, then `DIDX` and `DATA` when there are clips,
    /// then `HIRC` when `hirc` is set (an events bank writes it even empty).
    pub fn new_bnk(header: Vec<u8>, hirc: bool) -> Bank {
        let version = u32_at(&header, 0).unwrap_or(CLIENT_VERSION);
        let mut parts = vec![Part::Header, Part::Index, Part::Data];
        if hirc {
            parts.push(Part::Hirc);
        }
        let alignment = alignment_of(&header);
        Bank {
            kind: BankKind::Bnk,
            version,
            header,
            media: Vec::new(),
            objects: Vec::new(),
            alignment,
            parts,
        }
    }

    /// A new package of these clips, at version 1.
    pub fn new_wpk(media: Vec<Wem>) -> Bank {
        Bank {
            kind: BankKind::Wpk,
            version: 1,
            header: Vec::new(),
            media,
            objects: Vec::new(),
            alignment: PACKAGE_ALIGNMENT,
            parts: Vec::new(),
        }
    }

    /// The bank's id from its header; zero for a package.
    pub fn id(&self) -> u32 {
        u32_at(&self.header, BANK_ID_AT).unwrap_or(0)
    }

    /// Writes the bank's id into its header; a package has none to write.
    pub fn set_id(&mut self, id: u32) {
        let at = self.header.get_mut(BANK_ID_AT..);
        if let Some(bytes) = at.and_then(|b| b.first_chunk_mut()) {
            *bytes = id.to_le_bytes();
        }
    }

    /// Whether the bank holds a `HIRC` section, even an empty one.
    pub fn has_hirc(&self) -> bool {
        self.parts.contains(&Part::Hirc)
    }

    /// The first object of this id.
    pub fn object(&self, id: u32) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    /// The bank's file, in the format of its kind.
    pub fn write(&self) -> Vec<u8> {
        match self.kind {
            BankKind::Bnk => writer::write_bnk(self),
            BankKind::Wpk => writer::write_wpk(&self.media, self.version),
        }
    }
}
