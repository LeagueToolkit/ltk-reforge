//! A packed WAD (#MOD-ARTIFACT-0009): a whole WAD as one entry, mounted with `ltk_wad` over a
//! reader of its bytes, which seeks to each chunk and reads it alone. Its bytes are a mod's, so
//! every size its table declares is checked before it is used, and the reader runs where a
//! panic is caught.

use std::io::{self, Read, Seek, SeekFrom};
use std::panic::{AssertUnwindSafe, catch_unwind};

use ltk_wad::{Wad, WadChunk, WadChunkCompression};
use serde::Serialize;

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

/// The first bytes of a WAD of version 3: the magic and version, a 256-byte signature, an
/// 8-byte checksum and the count of its chunks. The table follows, 32 bytes a chunk.
const COUNT_AT: usize = 4 + 256 + 8;
const TABLE_AT: usize = COUNT_AT + 4;
const ENTRY_BYTES: u64 = 32;

/// A packed WAD, to be read under the archive's caps: a reader of its bytes, and their size.
pub(super) struct PackedWad<'a, R> {
    bytes: R,
    len: u64,
    caps: &'a Caps,
}

impl<'a, R: Read + Seek> PackedWad<'a, R> {
    /// A packed WAD's bytes, `len` of them, not read yet.
    pub(super) fn new(bytes: R, len: u64, caps: &'a Caps) -> Self {
        Self { bytes, len, caps }
    }

    /// Reads the WAD's chunks in path-hash order as `ltk_wad` gives them, each charged to the
    /// budget before it is decompressed and handed to `take` before the next is read, so that
    /// one chunk is held at a time (#ADR-0100). Gives what `take` made of each.
    pub(super) fn chunks<T>(
        mut self,
        budget: &mut Budget,
        take: impl FnMut(WadHash, &[u8]) -> T,
    ) -> Result<Vec<T>, PackedFailure> {
        self.check_header()?;
        guarded(|| self.read(budget, take))
    }

    /// Refuses a header that declares more chunks than the WAD's bytes can list, or more than
    /// the cap, before `ltk_wad` mounts it.
    fn check_header(&mut self) -> Result<(), PackedError> {
        // `Wad::mount` reserves memory for the count the header declares before it reads one
        // chunk. An allocation that fails aborts the process, which `catch_unwind` does not
        // catch. It reads the chunks of version 3 only, so the count is looked for there alone.
        let head = self.head().map_err(|e| PackedError::Mount {
            reason: e.to_string(),
        })?;
        let Some(&[b'R', b'W', major, _]) = head.first_chunk::<4>() else {
            return Ok(());
        };
        if major < 3 {
            return Err(PackedError::Version { major });
        }
        let declared = head
            .get(COUNT_AT..)
            .and_then(|rest| rest.first_chunk::<4>());
        let Some(chunks) = declared.map(|count| u64::from(u32::from_le_bytes(*count))) else {
            return Ok(());
        };
        if major != 3 {
            return Ok(());
        }
        // The impossible count comes first: the cap is set well above the game's own WADs, and
        // a count the bytes cannot list is wrong whatever the cap.
        let fit = (self.len - TABLE_AT as u64) / ENTRY_BYTES;
        if chunks > fit {
            return Err(PackedError::TableLargerThanData {
                chunks,
                bytes: self.len,
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

    /// The bytes up to the table, or fewer when the WAD is shorter, with the reader left at
    /// the start, where `Wad::mount` begins.
    fn head(&mut self) -> io::Result<Vec<u8>> {
        let mut head = Vec::with_capacity(TABLE_AT);
        self.bytes.seek(SeekFrom::Start(0))?;
        (&mut self.bytes)
            .take(TABLE_AT as u64)
            .read_to_end(&mut head)?;
        self.bytes.seek(SeekFrom::Start(0))?;
        Ok(head)
    }

    fn read<T>(
        self,
        budget: &mut Budget,
        mut take: impl FnMut(WadHash, &[u8]) -> T,
    ) -> Result<Vec<T>, PackedFailure> {
        let (len, caps) = (self.len, self.caps);
        let wad = Wad::mount(self.bytes).map_err(|e| PackedError::Mount {
            reason: e.to_string(),
        })?;
        let entries: Vec<WadChunk> = wad.chunks().iter().copied().collect();
        if entries.len() > caps.packed_chunks {
            return Err(PackedError::TooManyChunks {
                chunks: entries.len() as u64,
                cap: caps.packed_chunks,
            }
            .into());
        }
        let mut mounted = Mounted { wad, len, caps };
        entries
            .iter()
            .map(|entry| {
                let data = mounted.chunk(entry, budget)?;
                Ok(take(entry.path_hash(), &data))
            })
            .collect()
    }
}

/// A packed WAD that `ltk_wad` mounted, its chunks to be read one at a time.
struct Mounted<'a, R: Read + Seek> {
    wad: Wad<R>,
    len: u64,
    caps: &'a Caps,
}

impl<R: Read + Seek> Mounted<'_, R> {
    /// A chunk's bytes, decompressed.
    fn chunk(&mut self, entry: &WadChunk, budget: &mut Budget) -> Result<Box<[u8]>, PackedFailure> {
        let chunk = entry.path_hash();
        let bytes = declared_size(entry);
        if bytes > self.caps.chunk_bytes {
            return Err(PackedError::ChunkTooLarge {
                chunk,
                bytes,
                cap: self.caps.chunk_bytes,
            }
            .into());
        }
        // `ltk_wad` allocates the compressed size the entry declares before it reads.
        let end = (entry.data_offset() as u64).saturating_add(entry.compressed_size() as u64);
        if end > self.len {
            return Err(PackedError::ChunkPastEnd { chunk }.into());
        }
        budget.charge(bytes)?;
        let data =
            self.wad
                .load_chunk_decompressed(entry)
                .map_err(|e| PackedError::ChunkUnreadable {
                    chunk,
                    reason: e.to_string(),
                })?;
        // The declared size is charged above. What the reader gives beyond it is charged too.
        budget.charge((data.len() as u64).saturating_sub(bytes))?;
        Ok(data)
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
        let header = |bytes: &[u8]| {
            let len = bytes.len() as u64;
            PackedWad::new(io::Cursor::new(bytes), len, &caps).check_header()
        };
        assert_eq!(
            header(&bytes),
            Err(PackedError::TableLargerThanData {
                chunks: u64::from(u32::MAX),
                bytes: bytes.len() as u64,
                fit: 5
            })
        );
        bytes[268..272].copy_from_slice(&5u32.to_le_bytes());
        assert_eq!(
            header(&bytes),
            Err(PackedError::TooManyChunks { chunks: 5, cap: 4 })
        );
        bytes[268..272].copy_from_slice(&4u32.to_le_bytes());
        assert_eq!(header(&bytes), Ok(()));
        bytes[2] = 1;
        assert_eq!(header(&bytes), Err(PackedError::Version { major: 1 }));
        assert_eq!(header(b"RW"), Ok(()));
        assert_eq!(header(b"PK\x03\x04"), Ok(()));
    }
}
