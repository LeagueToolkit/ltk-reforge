//! The archive read from its file, entry by entry (#ADR-0100): what it gives is what the read
//! of the same bytes in memory gives, snapshot and blobs.

use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::archive::{self, Caps, RawSnapshot};
use reforge::diag::{Diagnostics, Stage};
use reforge::model::entity::WadLayout;
use reforge::model::{ByteView, ContentId};
use reforge::store::MemoryStore;

use reforge_testkit::archive::{DEFLATED, Field, STORED, packed_wad, rewrite, zip};

/// The name the archive is read under, in memory and on disk.
const NAME: &str = "mod.fantome";

/// A raw snapshot and the blobs of the store it was read into.
type Read = (RawSnapshot, BTreeMap<ContentId, ByteView>);

/// The archive read from its bytes in memory.
fn from_bytes(bytes: &[u8]) -> Read {
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

/// The archive written to a file and read from there.
fn from_file(bytes: &[u8]) -> Read {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("a UTF-8 path");
    let path = root.join(NAME);
    fs::write(&path, bytes).unwrap();
    let mut log = Diagnostics::new();
    let store = MemoryStore::new();
    let raw = archive::read_file(&path, &Caps::default(), &store, &mut log.at(Stage::Read));
    (raw.expect("the archive reads"), store.into_blobs())
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

    let (raw, blobs) = from_file(&bytes);
    assert_eq!((raw.clone(), blobs.clone()), from_bytes(&bytes));
    assert_eq!(raw.meta.sha256, ContentId::of(&bytes));
    let layouts: Vec<WadLayout> = raw.wads.iter().map(|wad| wad.layout).collect();
    assert_eq!(layouts, [WadLayout::Packed, WadLayout::Packed]);
    assert_eq!(raw.wads[1].files.len(), 3);
    let codes: Vec<&str> = raw.findings.iter().map(|f| f.code.as_str()).collect();
    assert_eq!(codes, ["archive.stored-crc-wrong"]);
    assert!(blobs.contains_key(&ContentId::of(b"packed c")));
}
