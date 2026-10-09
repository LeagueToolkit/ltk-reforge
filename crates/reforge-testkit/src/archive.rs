//! Archives in memory, for tests: zips written with `zip`, packed WADs with `ltk_wad`, and the
//! header edits no writer makes.

use std::io::{Cursor, Write};

use ltk_wad::{WadBuilder, WadChunkBuilder, WadChunkCompression};
use reforge::model::path_hash;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

/// A stored entry.
pub const STORED: CompressionMethod = CompressionMethod::Stored;
/// A deflated entry.
pub const DEFLATED: CompressionMethod = CompressionMethod::Deflated;

/// The bytes of a zip with these entries, in this order.
pub fn zip(entries: &[(&str, &[u8], CompressionMethod)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes, method) in entries {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(*method),
            )
            .unwrap_or_else(|error| panic!("the entry {name:?} does not start: {error}"));
        writer.write_all(bytes).unwrap_or_else(|error| {
            panic!(
                "the {} bytes of {name:?} do not write: {error}",
                bytes.len()
            )
        });
    }
    writer
        .finish()
        .unwrap_or_else(|error| {
            panic!(
                "the zip of {} entries does not finish: {error}",
                entries.len()
            )
        })
        .into_inner()
}

/// The bytes of a WAD with these chunks, each compressed with Zstandard.
pub fn packed_wad(chunks: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = WadBuilder::default();
    for (path, _) in chunks {
        builder = builder.with_chunk(
            WadChunkBuilder::default()
                .with_path(path)
                .with_force_compression(WadChunkCompression::Zstd),
        );
    }
    let mut out = Cursor::new(Vec::new());
    builder
        .build_to_writer(&mut out, |hash, chunk| {
            let (_, bytes) = chunks
                .iter()
                .find(|(path, _)| path_hash(path) == hash)
                .unwrap_or_else(|| panic!("the builder asked for chunk {hash:016x}, not given"));
            chunk.write_all(bytes)?;
            Ok(())
        })
        .unwrap_or_else(|error| {
            panic!("the WAD of {} chunks does not build: {error}", chunks.len())
        });
    out.into_inner()
}

/// Where a version 3 WAD keeps its chunk count: after the magic and version, the signature and
/// the checksum.
pub const WAD_COUNT_AT: usize = 4 + 256 + 8;
/// Where a version 3 WAD's first table entry starts. An entry is the path hash, then the data
/// offset, the compressed size and the uncompressed size, four bytes each.
pub const WAD_TABLE_AT: usize = WAD_COUNT_AT + 4;

/// Writes a little-endian `u32` into bytes.
pub fn put_u32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Reads a little-endian `u32` from bytes.
pub fn get_u32(bytes: &[u8], at: usize) -> u32 {
    let word = bytes[at..at + 4]
        .try_into()
        .unwrap_or_else(|error| panic!("no four bytes at {at} of {}: {error}", bytes.len()));
    u32::from_le_bytes(word)
}

/// A field of an entry's zip headers, with the value a test writes there.
#[derive(Debug, Clone, Copy)]
pub enum Field {
    /// The compression method's number.
    Method(u16),
    /// The checksum.
    Crc(u32),
    /// The inflated size.
    Size(u32),
}

/// One of the two headers a zip keeps of an entry, by where its fields are.
struct Header {
    signature: [u8; 4],
    method_at: usize,
    crc_at: usize,
    size_at: usize,
    name_len_at: usize,
    name_at: usize,
}

const LOCAL: Header = Header {
    signature: *b"PK\x03\x04",
    method_at: 8,
    crc_at: 14,
    size_at: 22,
    name_len_at: 26,
    name_at: 30,
};

const CENTRAL: Header = Header {
    signature: *b"PK\x01\x02",
    method_at: 10,
    crc_at: 16,
    size_at: 24,
    name_len_at: 28,
    name_at: 46,
};

/// Rewrites a field in the local and the central header of the entry with this name.
pub fn rewrite(zip: &mut [u8], entry: &str, field: Field) {
    let mut rewritten = 0;
    for header in [LOCAL, CENTRAL] {
        for at in 0..zip.len().saturating_sub(header.name_at) {
            if zip[at..at + 4] != header.signature {
                continue;
            }
            let name_len = usize::from(u16::from_le_bytes([
                zip[at + header.name_len_at],
                zip[at + header.name_len_at + 1],
            ]));
            let name_at = at + header.name_at;
            if zip.get(name_at..name_at + name_len) != Some(entry.as_bytes()) {
                continue;
            }
            match field {
                Field::Method(method) => zip[at + header.method_at..at + header.method_at + 2]
                    .copy_from_slice(&method.to_le_bytes()),
                Field::Crc(crc) => put_u32(zip, at + header.crc_at, crc),
                Field::Size(size) => put_u32(zip, at + header.size_at, size),
            }
            rewritten += 1;
        }
    }
    assert_eq!(
        rewritten, 2,
        "{entry:?} has a local and a central header to rewrite"
    );
}
