//! The readers. A clip is located twice, once to add its size up and once to copy it, so no
//! list of the entries is held between the two.

use std::iter;

use crate::bank::{
    Bank, BankKind, DEFAULT_ALIGNMENT, HEADER_MIN, PACKAGE_ALIGNMENT, Part, Wem, alignment_of,
};
use crate::cursor::{Cur, u16s, u32_at, u32s};
use crate::error::{BankError, NAME_SHOWN};
use crate::object::Object;

/// A clip's id and its bytes where they sit in the file.
type Clip<'a> = Result<(u32, &'a [u8]), BankError>;

/// The clips copied out of data of `data_len` bytes. `clips` locates them and is called twice.
///
/// The sizes are added up first. Entries may name one range many times, and each clip is a copy:
/// without the refusal a small file reads into many times its size.
fn copy_clips<'a, I>(
    kind: BankKind,
    data_len: usize,
    clips: impl Fn() -> I,
) -> Result<Vec<Wem>, BankError>
where
    I: Iterator<Item = Clip<'a>>,
{
    let mut total = 0u64;
    for clip in clips() {
        let (_, bytes) = clip?;
        total = total.saturating_add(bytes.len() as u64);
    }
    let data = data_len as u64;
    if total > data {
        return Err(BankError::ClipsLargerThanData {
            kind,
            clips: total,
            data,
        });
    }
    clips()
        .map(|clip| {
            clip.map(|(id, bytes)| Wem {
                id,
                bytes: bytes.to_vec(),
            })
        })
        .collect()
}

/// `size` bytes at `offset`, both as the file states them.
fn range(data: &[u8], offset: u32, size: u32) -> Option<&[u8]> {
    let start = usize::try_from(offset).ok()?;
    let end = start.checked_add(usize::try_from(size).ok()?)?;
    data.get(start..end)
}

pub(crate) fn read_bnk(bytes: &[u8]) -> Result<Bank, BankError> {
    let len = bytes.len();
    let mut cur = Cur::new(bytes);
    let mut bank = Bank {
        kind: BankKind::Bnk,
        version: 0,
        header: Vec::new(),
        media: Vec::new(),
        objects: Vec::new(),
        alignment: DEFAULT_ALIGNMENT,
        parts: Vec::new(),
    };
    let mut index: &[u8] = &[];
    let mut data = None;
    while cur.left() > 0 {
        let at = cur.pos();
        let (Some(tag), Some(size)) = (cur.tag(), cur.u32()) else {
            return Err(BankError::SectionHeaderCutShort { at, len });
        };
        let body = cur
            .take_u32(size)
            .ok_or(BankError::SectionPastEnd { tag, at, size, len })?;
        let part = match &tag {
            b"BKHD" => Part::Header,
            b"DIDX" => Part::Index,
            b"DATA" => Part::Data,
            b"HIRC" => Part::Hirc,
            _ => Part::Other(tag, body.to_vec()),
        };
        // The bank holds one of each, and the writer writes every part from it: a second one
        // would write the same section again.
        if !matches!(part, Part::Other(..)) && bank.parts.contains(&part) {
            return Err(BankError::DuplicateSection { tag, at });
        }
        match part {
            Part::Header => {
                let version = u32_at(body, 0).filter(|_| body.len() >= HEADER_MIN);
                bank.version = version.ok_or(BankError::HeaderTooShort { size: body.len() })?;
                bank.alignment = alignment_of(body);
                bank.header = body.to_vec();
            }
            Part::Index => index = body,
            Part::Data => data = Some(body),
            Part::Hirc => bank.objects = read_objects(body)?,
            Part::Other(..) => {}
        }
        bank.parts.push(part);
    }
    bank.media = bank_clips(index, data)?;
    Ok(bank)
}

fn read_objects(hirc: &[u8]) -> Result<Vec<Object>, BankError> {
    let len = hirc.len();
    let mut cur = Cur::new(hirc);
    let count = cur.u32().ok_or(BankError::HircWithoutCount { size: len })?;
    let mut objects = Vec::new();
    for index in 0..count {
        let at = cur.pos();
        let (Some(kind), Some(size)) = (cur.u8(), cur.u32()) else {
            return Err(BankError::ObjectCutShort {
                index,
                count,
                at,
                len,
            });
        };
        let left = cur.left();
        let object = cur.take_u32(size).ok_or(BankError::ObjectPastSection {
            index,
            kind,
            size,
            left,
        })?;
        let (id, body) = object
            .split_first_chunk()
            .ok_or(BankError::ObjectWithoutId { index, kind, size })?;
        objects.push(Object {
            kind,
            id: u32::from_le_bytes(*id),
            body: body.to_vec(),
        });
    }
    Ok(objects)
}

/// The size of a `DIDX` entry: the clip's id, its offset into `DATA`, its size.
const INDEX_ENTRY: usize = 12;

fn bank_clips(index: &[u8], data: Option<&[u8]>) -> Result<Vec<Wem>, BankError> {
    let count = index.len() / INDEX_ENTRY;
    if count == 0 {
        return Ok(Vec::new());
    }
    let data = data.ok_or(BankError::IndexWithoutData { clips: count })?;
    let clips = || {
        let mut cur = Cur::new(index);
        iter::from_fn(move || Some((cur.u32()?, cur.u32()?, cur.u32()?))).map(
            |(id, offset, size)| {
                let bytes = range(data, offset, size).ok_or(BankError::ClipPastData {
                    id,
                    offset,
                    size,
                    data: data.len(),
                })?;
                Ok((id, bytes))
            },
        )
    };
    copy_clips(BankKind::Bnk, data.len(), clips)
}

pub(crate) fn read_wpk(bytes: &[u8]) -> Result<Bank, BankError> {
    let len = bytes.len();
    let mut cur = Cur::new(bytes);
    let (Some(_magic), Some(version), Some(count)) = (cur.take(4), cur.u32(), cur.u32()) else {
        return Err(BankError::PackageHeaderCutShort { len });
    };
    let offsets = usize::try_from(count).ok().and_then(|n| n.checked_mul(4));
    let offsets = offsets
        .and_then(|n| cur.take(n))
        .ok_or(BankError::PackageOffsetsCutShort { count, len })?;
    let clips = || {
        u32s(offsets)
            .enumerate()
            // A zero offset names no entry.
            .filter(|(_, offset)| *offset != 0)
            .map(|(entry, offset)| package_clip(bytes, entry, offset))
    };
    let media = copy_clips(BankKind::Wpk, len, clips)?;
    Ok(Bank {
        kind: BankKind::Wpk,
        version,
        header: Vec::new(),
        media,
        objects: Vec::new(),
        alignment: PACKAGE_ALIGNMENT,
        parts: Vec::new(),
    })
}

/// The clip of the package entry at `offset`.
fn package_clip(bytes: &[u8], entry: usize, offset: u32) -> Clip<'_> {
    let len = bytes.len();
    let at = usize::try_from(offset).ok();
    let mut cur = Cur::new(at.and_then(|at| bytes.get(at..)).unwrap_or_default());
    let (Some(data_offset), Some(size), Some(name_len)) = (cur.u32(), cur.u32(), cur.u32()) else {
        return Err(BankError::EntryCutShort { entry, offset, len });
    };
    let left = cur.left();
    let name = usize::try_from(name_len)
        .ok()
        .and_then(|n| n.checked_mul(2));
    let name = name
        .and_then(|n| cur.take(n))
        .ok_or(BankError::EntryNameCutShort {
            entry,
            name_len,
            left,
        })?;
    let id = clip_id(name).ok_or_else(|| BankError::EntryName {
        entry,
        name: shown(name),
    })?;
    let clip = range(bytes, data_offset, size).ok_or(BankError::ClipPastData {
        id,
        offset: data_offset,
        size,
        data: len,
    })?;
    Ok((id, clip))
}

/// The most decimal digits an id of 32 bits has.
const ID_DIGITS: usize = 10;

/// The clip id a package entry's name starts with (`<id>.wem`).
///
/// Read off the first digits alone. The file states the name's length and many offsets may name
/// one entry, so an entry must cost the same whatever its name's length: the name is not
/// decoded, and a longer run of digits is no id, leading zeros included.
fn clip_id(name: &[u8]) -> Option<u32> {
    let mut id = None;
    for (at, unit) in u16s(name).enumerate() {
        let Some(digit) = unit.checked_sub(u16::from(b'0')).filter(|d| *d < 10) else {
            break;
        };
        if at == ID_DIGITS {
            return None;
        }
        id = Some(
            id.unwrap_or(0u32)
                .checked_mul(10)?
                .checked_add(u32::from(digit))?,
        );
    }
    id
}

/// The start of an entry's name, for an error.
fn shown(name: &[u8]) -> String {
    char::decode_utf16(u16s(name).take(NAME_SHOWN))
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}
