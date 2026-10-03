//! The writers. A bank writes its sections in the order they were read in, the clips laid out
//! again from `media`.

use crate::bank::{Bank, PACKAGE_ALIGNMENT, Part, Wem};

fn section(out: &mut Vec<u8>, tag: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(tag);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
}

fn pad_to(out: &mut Vec<u8>, alignment: usize) {
    out.resize(out.len().next_multiple_of(alignment), 0);
}

/// `DIDX` and `DATA` for clips whose `DATA` body starts at `base` in the file: each clip at a
/// multiple of the alignment counted from the file's start, the last one unpadded.
fn media_sections(bank: &Bank, base: usize) -> (Vec<u8>, Vec<u8>) {
    let mut index = Vec::new();
    let mut data = Vec::new();
    for w in &bank.media {
        let at = (base + data.len()).next_multiple_of(bank.alignment.max(1));
        data.resize(at - base, 0);
        index.extend_from_slice(&w.id.to_le_bytes());
        index.extend_from_slice(&(data.len() as u32).to_le_bytes());
        index.extend_from_slice(&(w.bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(&w.bytes);
    }
    (index, data)
}

pub(crate) fn write_bnk(bank: &Bank) -> Vec<u8> {
    let mut hirc = (bank.objects.len() as u32).to_le_bytes().to_vec();
    for o in &bank.objects {
        hirc.push(o.kind);
        hirc.extend_from_slice(&((o.body.len() + 4) as u32).to_le_bytes());
        hirc.extend_from_slice(&o.id.to_le_bytes());
        hirc.extend_from_slice(&o.body);
    }
    let mut out = Vec::new();
    // `DATA` follows `DIDX` in every bank seen; its start is known once `DIDX` is placed.
    let mut data = Vec::new();
    for part in &bank.parts {
        match part {
            Part::Header => section(&mut out, b"BKHD", &bank.header),
            Part::Index if !bank.media.is_empty() => {
                let base = out.len() + 8 + bank.media.len() * 12 + 8;
                let (index, d) = media_sections(bank, base);
                data = d;
                section(&mut out, b"DIDX", &index);
            }
            Part::Data if !bank.media.is_empty() => section(&mut out, b"DATA", &data),
            Part::Index | Part::Data => {}
            Part::Hirc => section(&mut out, b"HIRC", &hirc),
            Part::Other(tag, body) => section(&mut out, tag, body),
        }
    }
    out
}

pub(crate) fn write_wpk(media: &[Wem], version: u32) -> Vec<u8> {
    let mut out = b"r3d2".to_vec();
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&(media.len() as u32).to_le_bytes());
    let offsets_at = out.len();
    out.resize(offsets_at + media.len() * 4, 0);
    let mut entries = Vec::new();
    for (i, w) in media.iter().enumerate() {
        pad_to(&mut out, PACKAGE_ALIGNMENT);
        let at = out.len();
        out[offsets_at + i * 4..offsets_at + i * 4 + 4].copy_from_slice(&(at as u32).to_le_bytes());
        entries.push(at);
        // The data's offset and size, filled in once the clips are placed.
        out.extend_from_slice(&[0; 8]);
        let name: Vec<u16> = format!("{}.wem", w.id).encode_utf16().collect();
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend(name.into_iter().flat_map(u16::to_le_bytes));
    }
    for (at, w) in entries.iter().zip(media) {
        pad_to(&mut out, PACKAGE_ALIGNMENT);
        let data_at = out.len() as u32;
        out[*at..*at + 4].copy_from_slice(&data_at.to_le_bytes());
        out[*at + 4..*at + 8].copy_from_slice(&(w.bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&w.bytes);
    }
    out
}
