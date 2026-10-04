//! What reading an archive allocates is bounded by the bytes it is given
//! (`docs/design/ARCHITECTURE.md`, "Untrusted input"). A failed allocation aborts the process,
//! so the bound is measured: this test binary counts the largest allocation asked for.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use reforge::archive::{Archive, Caps, PackedError, WadUnreadable};
use reforge::diag::{Diagnostics, Stage};
use reforge_testkit::archive::{STORED, WAD_TABLE_AT, packed_wad, put_u32, zip};

/// The system's allocator, with the largest size asked of it.
struct Counting;

static LARGEST: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call goes to the system's allocator unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's contract is the system allocator's.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, at: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract is the system allocator's.
        unsafe { System.dealloc(at, layout) }
    }

    // The system zeroes pages without touching them: the default would write every byte.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        // SAFETY: the caller's contract is the system allocator's.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, at: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        LARGEST.fetch_max(size, Ordering::Relaxed);
        // SAFETY: the caller's contract is the system allocator's.
        unsafe { System.realloc(at, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const MIB: usize = 1 << 20;

/// Reads an archive, and gives its log with the largest allocation the read asked for.
fn read(bytes: &[u8]) -> (Diagnostics, usize) {
    let mut log = Diagnostics::new();
    LARGEST.store(0, Ordering::Relaxed);
    let read = Archive::read(
        bytes,
        "synthetic.fantome",
        &Caps::default(),
        &mut log.at(Stage::Read),
    );
    let largest = LARGEST.load(Ordering::Relaxed);
    assert!(read.is_ok(), "the archive reads");
    (log, largest)
}

#[test]
#[ignore = "ltk_wad's mount allocates the compressed size a chunk declares, 256 MiB here, \
            before reforge checks the size against the WAD's bytes"]
fn a_packed_wad_that_declares_a_chunk_larger_than_its_bytes_allocates_no_more_than_it_holds() {
    let good = packed_wad(&[("data/a.bin", b"some bytes of a chunk")]);
    let (_, largest) = read(&zip(&[("WAD/Lux.wad.client", &good, STORED)]));
    assert!(
        largest < MIB,
        "a WAD of {} bytes asked for {largest}",
        good.len()
    );

    // `ltk_wad` 0.5.8 looks for the table of subchunks as it mounts: a chunk whose inflated
    // size is 16 bytes for each subchunk the WAD declares is read whole to see whether it is
    // that table.
    let mut crafted = good.clone();
    let (compressed_size_at, uncompressed_size_at) = (WAD_TABLE_AT + 12, WAD_TABLE_AT + 16);
    let compression_at = WAD_TABLE_AT + 20;
    put_u32(&mut crafted, compressed_size_at, 256 * MIB as u32);
    put_u32(&mut crafted, uncompressed_size_at, 16);
    // In subchunks, one of them, the first at 0.
    put_u32(&mut crafted, compression_at, 0x14);
    let (log, largest) = read(&zip(&[("WAD/Lux.wad.client", &crafted, STORED)]));

    // The WAD is refused either way: the chunk ends past the WAD's bytes.
    let unreadable: Vec<&WadUnreadable> = log.of().collect();
    assert_eq!(unreadable.len(), 1);
    assert!(
        matches!(
            unreadable[0].entries[0].why,
            PackedError::ChunkPastEnd { .. }
        ),
        "{:?}",
        unreadable[0].entries[0].why
    );
    assert!(
        largest < MIB,
        "a WAD of {} bytes asked for an allocation of {largest} bytes",
        crafted.len()
    );
}
