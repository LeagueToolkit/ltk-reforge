//! The archive read from its file, entry by entry (#ADR-0100): what it gives is what the read
//! of the same bytes in memory gives, snapshot and blobs, into a store in memory or on disk.

use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::archive::{self, Caps, RawSnapshot};
use reforge::diag::{Diagnostics, Stage};
use reforge::model::entity::WadLayout;
use reforge::model::{ByteView, ContentId};
use reforge::store::{BlobStore, IngestStore, MemoryStore};

use reforge_testkit::archive::{DEFLATED, Field, STORED, packed_wad, rewrite, zip};

/// The name the archive is read under, in memory and on disk.
const NAME: &str = "mod.fantome";

/// A scratch directory, removed when dropped.
struct Scratch {
    _dir: tempfile::TempDir,
    root: Utf8PathBuf,
}

fn scratch() -> Scratch {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("a UTF-8 path");
    Scratch { _dir: dir, root }
}

/// The archive read from its bytes in memory, and the blobs of the store it was read into.
fn from_bytes(bytes: &[u8]) -> (RawSnapshot, BTreeMap<ContentId, ByteView>) {
    let mut log = Diagnostics::new();
    let store = MemoryStore::new();
    let raw = archive::read(
        bytes,
        NAME,
        &Caps::default(),
        &store,
        &mut log.at(Stage::Read),
    );
    (raw.expect("the archive reads"), store.into_blobs())
}

/// The archive written to a file in the scratch directory and read from there into a store.
fn from_file(scratch: &Scratch, bytes: &[u8], store: &dyn BlobStore) -> RawSnapshot {
    let path = scratch.root.join(NAME);
    fs::write(&path, bytes).unwrap();
    let mut log = Diagnostics::new();
    let raw = archive::read_file(&path, &Caps::default(), store, &mut log.at(Stage::Read));
    raw.expect("the archive reads")
}

#[test]
fn a_stored_wad_read_from_its_file_gives_what_its_bytes_give() {
    // #MOD-ARTIFACT-0006, #MOD-ARTIFACT-0009: a packed WAD stored as it is, one stored with a
    // wrong checksum, loose chunks and the extras.
    let lux = packed_wad(&[("data/a.bin", b"packed a"), ("data/b.bin", b"packed b")]);
    let ahri = packed_wad(&[("data/c.bin", b"packed c")]);
    let mut bytes = zip(&[
        ("META/info.json", br#"{"Name":"A Mod"}"#, DEFLATED),
        ("WAD/Lux.wad.client", &lux, STORED),
        ("WAD/Lux.wad.client/data/d.bin", b"loose d", DEFLATED),
        ("WAD/Ahri.wad.client", &ahri, STORED),
        ("META/image.png", b"the image", STORED),
    ]);
    rewrite(&mut bytes, "WAD/Ahri.wad.client", Field::Crc(0xdead_beef));

    let scratch = scratch();
    let store = MemoryStore::new();
    let raw = from_file(&scratch, &bytes, &store);
    assert_eq!((raw.clone(), store.into_blobs()), from_bytes(&bytes));
    assert_eq!(raw.meta.sha256, ContentId::of(&bytes));
    let layouts: Vec<WadLayout> = raw.wads.iter().map(|wad| wad.layout).collect();
    assert_eq!(layouts, [WadLayout::Packed, WadLayout::Packed]);
    assert_eq!(raw.wads[1].files.len(), 3);
    let codes: Vec<&str> = raw.findings.iter().map(|f| f.code.as_str()).collect();
    assert_eq!(codes, ["archive.stored-crc-wrong"]);
}

#[test]
fn a_deflated_wad_read_into_a_store_on_disk_is_inflated_once_into_a_scratch_file() {
    // #ADR-0100: the WAD inflates into a scratch file in the store's directory and is mounted
    // from there; its chunks are the blobs, and the file goes when they are read. A store in
    // memory holds no WAD either.
    let lux = packed_wad(&[("data/a.bin", b"packed a"), ("data/b.bin", b"packed b")]);
    let bytes = zip(&[
        ("WAD/Lux.wad.client", &lux, DEFLATED),
        ("WAD/Zed.wad.client/data/z.bin", b"loose z", DEFLATED),
    ]);
    let (expected, blobs) = from_bytes(&bytes);
    assert!(!blobs.contains_key(&ContentId::of(&lux)));

    let scratch = scratch();
    let dir = scratch.root.join("store");
    let store = IngestStore::new(&dir);
    let raw = from_file(&scratch, &bytes, &store);
    assert_eq!(raw, expected);
    assert_eq!(raw.wads[0].layout, WadLayout::Packed);
    for (id, blob) in &blobs {
        assert_eq!(store.get(*id).as_deref(), Some(&blob[..]), "{id:?}");
    }
    assert!(!store.contains(ContentId::of(&lux)), "the WAD is no blob");
    let scratch_files: Vec<_> = fs::read_dir(dir.join("scratch")).unwrap().collect();
    assert!(scratch_files.is_empty(), "no scratch file is left");
}
