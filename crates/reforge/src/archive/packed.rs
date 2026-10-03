//! A packed WAD (#MOD-ARTIFACT-0009): a whole WAD as one entry, mounted from memory with
//! `ltk_wad`. Its bytes are a mod's, so every size its table declares is checked before it is
//! used, and the reader runs where a panic is caught.

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ltk_wad::{Wad, WadChunk, WadChunkCompression};
use serde::Serialize;

use super::Chunk;
use super::caps::{Budget, Caps, OverBudget};
use super::findings::hash_hex;
use crate::model::{WadHash, panic_text};

/// Why a packed WAD does not read. One chunk that does not read refuses the whole WAD: a WAD
/// that is damaged is not carried in part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "why", rename_all = "kebab-case")]
pub enum PackedError {
    /// The WAD is of a version before 3, whose chunks `ltk_wad` does not read.
    #[error("WAD version {major} is not read")]
    Version {
        /// The major version the header declares.
        major: u8,
    },
    /// The WAD declares more chunks than its bytes hold a table for.
    #[error("{chunks} chunks declared, and the WAD's {bytes} bytes hold a table of {fit}")]
    TableLargerThanData {
        /// The chunks the header declares.
        chunks: u64,
        /// The bytes of the WAD.
        bytes: u64,
        /// The chunks a table in those bytes can list.
        fit: u64,
    },
    /// The WAD declares more chunks than the cap.
    #[error("{chunks} chunks, over the cap of {cap}")]
    TooManyChunks {
        /// The chunks the header declares.
        chunks: u64,
        /// The cap on the chunks of one packed WAD.
        cap: usize,
    },
    /// `ltk_wad` refused the header or the table of contents.
    #[error("does not mount: {reason}")]
    Mount {
        /// What failed.
        reason: String,
    },
    /// A chunk declares more bytes than the cap for one chunk.
    #[error("chunk {chunk:016x} is {bytes} bytes, over the cap of {cap}")]
    ChunkTooLarge {
        /// The chunk.
        #[serde(serialize_with = "hash_hex")]
        chunk: WadHash,
        /// The inflated bytes the chunk declares.
        bytes: u64,
        /// The cap on one chunk.
        cap: u64,
    },
    /// A chunk's bytes are declared to lie past the end of the WAD.
    #[error("chunk {chunk:016x} lies past the end of the WAD")]
    ChunkPastEnd {
        /// The chunk.
        #[serde(serialize_with = "hash_hex")]
        chunk: WadHash,
    },
    /// A chunk does not decompress.
    #[error("chunk {chunk:016x} does not decompress: {reason}")]
    ChunkUnreadable {
        /// The chunk.
        #[serde(serialize_with = "hash_hex")]
        chunk: WadHash,
        /// What failed.
        reason: String,
    },
    /// `ltk_wad` panicked on the bytes.
    #[error("the reader panicked: {reason}")]
    Panicked {
        /// The panic's message.
        reason: String,
    },
}

/// Why a packed WAD gives no chunks.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum PackedFailure {
    /// The WAD does not read. The archive is read on without it.
    #[error(transparent)]
    Unreadable(#[from] PackedError),
    /// Its chunks inflate past the archive's budget. Reading ends.
    #[error(transparent)]
    OverBudget(#[from] OverBudget),
}

/// Runs a reader of a mod's bytes where its panic is caught.
fn guarded<T>(read: impl FnOnce() -> Result<T, PackedFailure>) -> Result<T, PackedFailure> {
    catch_unwind(AssertUnwindSafe(read)).unwrap_or_else(|payload| {
        Err(PackedError::Panicked {
            reason: panic_text(&*payload),
        }
        .into())
    })
}

/// The bytes a chunk becomes, by what its entry declares. `ltk_wad` gives an uncompressed
/// chunk's stored bytes whatever uncompressed size the entry declares.
fn declared_size(entry: &WadChunk) -> u64 {
    match entry.compression_type() {
        WadChunkCompression::None => entry.compressed_size() as u64,
        _ => entry.uncompressed_size() as u64,
    }
}

/// The bytes of a packed WAD, to be read under the archive's caps.
pub(super) struct PackedWad<'a> {
    bytes: &'a [u8],
    caps: &'a Caps,
}

impl<'a> PackedWad<'a> {
    /// A packed WAD's bytes, not read yet.
    pub(super) fn new(bytes: &'a [u8], caps: &'a Caps) -> Self {
        Self { bytes, caps }
    }

    /// The WAD's chunks, decompressed, in path-hash order as `ltk_wad` gives them. Each is
    /// charged to the budget before it is decompressed.
    pub(super) fn chunks(&self, budget: &mut Budget) -> Result<Vec<Chunk>, PackedFailure> {
        self.check_header()?;
        guarded(|| self.read(budget))
    }

    /// Refuses a header that declares more chunks than the WAD's bytes can list, or more than
    /// the cap, before `ltk_wad` mounts it.
    fn check_header(&self) -> Result<(), PackedError> {
        // `Wad::mount` reserves memory for the count the header declares before it reads one
        // chunk. An allocation that fails aborts the process, which `catch_unwind` does not
        // catch. It reads the chunks of version 3 only, so the count is looked for there alone:
        // after the magic and version, a 256-byte signature and an 8-byte checksum. The table
        // follows the count, 32 bytes a chunk.
        const COUNT_AT: usize = 4 + 256 + 8;
        const TABLE_AT: usize = COUNT_AT + 4;
        const ENTRY_BYTES: u64 = 32;
        let Some(&[b'R', b'W', major, _]) = self.bytes.first_chunk::<4>() else {
            return Ok(());
        };
        if major < 3 {
            return Err(PackedError::Version { major });
        }
        let declared = self
            .bytes
            .get(COUNT_AT..)
            .and_then(|rest| rest.first_chunk::<4>());
        let Some(chunks) = declared.map(|count| u64::from(u32::from_le_bytes(*count))) else {
            return Ok(());
        };
        if major != 3 {
            return Ok(());
        }
        // The impossible count comes first: the game's own WADs hold up to some 200,000 chunks,
        // so the cap is high, and a count the bytes cannot list is wrong whatever the cap.
        let fit = (self.bytes.len() - TABLE_AT) as u64 / ENTRY_BYTES;
        if chunks > fit {
            return Err(PackedError::TableLargerThanData {
                chunks,
                bytes: self.bytes.len() as u64,
                fit,
            });
        }
        if chunks > self.caps.packed_chunks as u64 {
            return Err(PackedError::TooManyChunks {
                chunks,
                cap: self.caps.packed_chunks,
            });
        }
        Ok(())
    }

    fn read(&self, budget: &mut Budget) -> Result<Vec<Chunk>, PackedFailure> {
        let mut wad = Wad::mount(Cursor::new(self.bytes)).map_err(|e| PackedError::Mount {
            reason: e.to_string(),
        })?;
        let entries: Vec<WadChunk> = wad.chunks().iter().copied().collect();
        if entries.len() > self.caps.packed_chunks {
            return Err(PackedError::TooManyChunks {
                chunks: entries.len() as u64,
                cap: self.caps.packed_chunks,
            }
            .into());
        }
        entries
            .iter()
            .map(|entry| self.chunk(&mut wad, entry, budget))
            .collect()
    }

    fn chunk(
        &self,
        wad: &mut Wad<Cursor<&[u8]>>,
        entry: &WadChunk,
        budget: &mut Budget,
    ) -> Result<Chunk, PackedFailure> {
        let chunk = entry.path_hash();
        let bytes = declared_size(entry);
        if bytes > self.caps.entry_bytes {
            return Err(PackedError::ChunkTooLarge {
                chunk,
                bytes,
                cap: self.caps.entry_bytes,
            }
            .into());
        }
        // `ltk_wad` allocates the compressed size the entry declares before it reads.
        let end = (entry.data_offset() as u64).saturating_add(entry.compressed_size() as u64);
        if end > self.bytes.len() as u64 {
            return Err(PackedError::ChunkPastEnd { chunk }.into());
        }
        budget.charge(bytes)?;
        let data =
            wad.load_chunk_decompressed(entry)
                .map_err(|e| PackedError::ChunkUnreadable {
                    chunk,
                    reason: e.to_string(),
                })?;
        // The declared size is charged above. What the reader gives beyond it is charged too.
        budget.charge((data.len() as u64).saturating_sub(bytes))?;
        Ok(Chunk::new(chunk, None, &data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_of_the_reader_is_an_error_with_its_message() {
        let read = || -> Result<(), PackedFailure> {
            let empty: &[u8] = &[];
            let _ = empty[usize::from(empty.is_empty())];
            Ok(())
        };
        let Err(PackedFailure::Unreadable(PackedError::Panicked { reason })) = guarded(read) else {
            panic!("the panic was not caught as an error");
        };
        assert!(reason.contains("index out of bounds"), "{reason}");
    }

    #[test]
    fn a_header_declaring_chunks_it_cannot_list_or_past_the_cap_is_refused_before_it_mounts() {
        let caps = Caps {
            packed_chunks: 4,
            ..Caps::default()
        };
        // A header and room for a table of five chunks.
        let mut bytes = vec![0u8; 4 + 256 + 8 + 4 + 5 * 32];
        bytes[..4].copy_from_slice(&[b'R', b'W', 3, 4]);
        bytes[268..272].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            PackedWad::new(&bytes, &caps).check_header(),
            Err(PackedError::TableLargerThanData {
                chunks: u64::from(u32::MAX),
                bytes: bytes.len() as u64,
                fit: 5
            })
        );
        bytes[268..272].copy_from_slice(&5u32.to_le_bytes());
        assert_eq!(
            PackedWad::new(&bytes, &caps).check_header(),
            Err(PackedError::TooManyChunks { chunks: 5, cap: 4 })
        );
        bytes[268..272].copy_from_slice(&4u32.to_le_bytes());
        assert_eq!(PackedWad::new(&bytes, &caps).check_header(), Ok(()));
        bytes[2] = 1;
        assert_eq!(
            PackedWad::new(&bytes, &caps).check_header(),
            Err(PackedError::Version { major: 1 })
        );
        assert_eq!(PackedWad::new(b"RW", &caps).check_header(), Ok(()));
        assert_eq!(PackedWad::new(b"PK\x03\x04", &caps).check_header(), Ok(()));
    }
}
