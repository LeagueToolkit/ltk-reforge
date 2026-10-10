//! The archive reader over zips built here: every layout, the caps, the names it refuses, what
//! it skips and what it notes, and the raw snapshot it gives with the bytes in the store.

use std::collections::BTreeMap;
use std::ops::Deref;

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::archive::{
    self, BudgetExceeded, CODES, Caps, ChunkDuplicates, CodecsUnsupported, EntriesCorrupt,
    EntriesTooLarge, EntriesUnreadable, InfoUnreadable, NamesUnsafe, NotZip, PackedError,
    RAW_VERSION, RawFile, RawImage, RawSnapshot, RepackDuplicates, StoredCrcWrong, TooManyEntries,
    WadUnreadable, file_kind,
};
use reforge::diag::{Channel, Diag, Diagnostics, Halted, Render, Severity, Stage, Subject};
use reforge::model::entity::{FileKind, MAX_IMAGE_BYTES, WadLayout};
use reforge::model::{ByteView, ContentId, PathError, Spelled, WadHash, path_hash};
use reforge::store::{BlobStore, MemoryStore};

use reforge_testkit::archive::{
    DEFLATED, Field, STORED, WAD_COUNT_AT, WAD_TABLE_AT, get_u32, packed_wad, put_u32, rewrite, zip,
};

/// An archive read into a store of its own.
#[derive(Debug)]
struct Read {
    raw: RawSnapshot,
    store: MemoryStore,
}

impl Deref for Read {
    type Target = RawSnapshot;

    fn deref(&self) -> &RawSnapshot {
        &self.raw
    }
}

impl Read {
    /// The file of a WAD at a path hash.
    fn file(&self, wad: usize, hash: WadHash) -> &RawFile {
        self.raw.wads[wad]
            .files
            .get(&hash)
            .unwrap_or_else(|| panic!("no file at {hash:?} in WAD {wad}"))
    }

    /// The first file of a WAD, in path-hash order.
    fn first(&self, wad: usize) -> &RawFile {
        self.raw.wads[wad].files.values().next().expect("a file")
    }

    /// A file's bytes, from the store.
    fn bytes(&self, file: &RawFile) -> ByteView {
        self.store
            .get(file.content)
            .unwrap_or_else(|| panic!("the store holds {:?}", file.content))
    }

    /// The bytes of a WAD's file at a path.
    fn bytes_at(&self, wad: usize, path: &str) -> ByteView {
        self.bytes(self.file(wad, path_hash(path)))
    }
}

fn read_with(bytes: &[u8], caps: &Caps) -> (Result<Read, Halted>, Diagnostics) {
    let mut log = Diagnostics::new();
    let store = MemoryStore::new();
    let result = archive::read(bytes, "mod.fantome", caps, &store, &mut log.at(Stage::Read));
    (result.map(|raw| Read { raw, store }), log)
}

fn read(bytes: &[u8]) -> (Read, Diagnostics) {
    let (result, log) = read_with(bytes, &Caps::default());
    let archive =
        result.unwrap_or_else(|halted| panic!("the read ended: {halted}, {:?}", log.get(halted.0)));
    (archive, log)
}

/// The one finding of a type.
fn one<T: Diag>(log: &Diagnostics) -> &T {
    let mut found = log.of::<T>();
    let first = found
        .next()
        .unwrap_or_else(|| panic!("no {} was raised", T::CODE));
    assert_eq!(found.count(), 0, "{} was raised more than once", T::CODE);
    first
}

fn codes(log: &Diagnostics) -> Vec<&'static str> {
    log.iter().map(|d| d.code.name).collect()
}

fn spelled(names: &[Spelled]) -> Vec<&str> {
    names.iter().map(Spelled::raw).collect()
}

const SKIN: &str = "data/characters/lux/skins/skin0.bin";

#[test]
fn chunks_are_found_by_path_and_by_hash() {
    // #MOD-ARTIFACT-0007: two chunks named by their hash, one with an extension.
    // #MOD-ARTIFACT-0008: a third whose extension is a number.
    let bytes = zip(&[
        (
            "META/info.json",
            br#"{"Name":"Lux Mod","Author":"someone","Version":"1.2"}"#,
            STORED,
        ),
        (
            "WAD/Lux.wad.client/DATA/Characters/Lux/Skins/Skin0.bin",
            b"PROP-ish",
            DEFLATED,
        ),
        ("WAD/Lux.wad.client/0123456789abcdef", b"hashed", STORED),
        (
            "WAD/Lux.wad.client/0123456789abcdef.1",
            b"duplicate kept by a repack",
            STORED,
        ),
        (
            "WAD/Lux.wad.client/fedcba9876543210.dds",
            b"hashed too",
            STORED,
        ),
        ("WAD/Lux.wad.client.pyntex.json", b"{}", STORED),
        ("RAW/readme.txt", b"ignored", STORED),
    ]);
    let (archive, log) = read(&bytes);
    assert_eq!(archive.meta.name, Some(Spelled::new("Lux Mod")));
    assert_eq!(archive.meta.author, Some(Spelled::new("someone")));
    assert_eq!(archive.meta.version, Some(Spelled::new("1.2")));
    assert_eq!(archive.meta.description, None);
    assert_eq!(archive.wads.len(), 1, "the pyntex sidecar is not a WAD");
    let wad = &archive.wads[0];
    assert_eq!(wad.name.as_str(), "Lux.wad.client");
    assert_eq!(wad.layout, WadLayout::Loose);
    assert_eq!(wad.files.len(), 3);

    let by_path = archive.file(0, path_hash(SKIN));
    assert_eq!(by_path.path.as_ref().map(|p| p.as_str()), Some(SKIN));
    assert_eq!(&*archive.bytes(by_path), b"PROP-ish");
    assert_eq!(by_path.content, ContentId::of(b"PROP-ish"));

    let by_hash = archive.file(0, WadHash(0x0123_4567_89ab_cdef));
    assert_eq!(&*archive.bytes(by_hash), b"hashed");
    assert_eq!(by_hash.path, None);
    let with_extension = archive.file(0, WadHash(0xfedc_ba98_7654_3210));
    assert_eq!(&*archive.bytes(with_extension), b"hashed too");

    let repack = one::<RepackDuplicates>(&log);
    assert_eq!(repack.wad.raw(), "Lux.wad.client");
    assert_eq!(
        spelled(&repack.entries),
        ["WAD/Lux.wad.client/0123456789abcdef.1"]
    );
    assert_eq!(codes(&log), ["archive.repack-duplicate"]);
}

#[test]
fn the_read_puts_every_entrys_bytes_in_the_store_once_and_the_snapshot_names_them() {
    // #ADR-0093, #ADR-0094: two chunks of one content are one blob, and the extras' first
    // bytes and the image are blobs too.
    let bytes = zip(&[
        ("WAD/Lux.wad.client/data/a.bin", b"the same bytes", STORED),
        ("WAD/Lux.wad.client/data/b.bin", b"the same bytes", DEFLATED),
        ("WAD/Ahri.wad.client/data/c.bin", b"other bytes", STORED),
        ("META/image.png", b"the image", STORED),
        ("RAW/readme.txt", b"a readme", STORED),
    ]);
    let (archive, log) = read(&bytes);
    assert!(log.is_empty(), "{:?}", codes(&log));
    assert_eq!(archive.version, RAW_VERSION);
    let same = ContentId::of(b"the same bytes");
    assert_eq!(archive.file(1, path_hash("data/a.bin")).content, same);
    assert_eq!(archive.file(1, path_hash("data/b.bin")).content, same);
    let mut ids = vec![
        same,
        ContentId::of(b"other bytes"),
        ContentId::of(b"the image"),
        ContentId::of(b"a readme"),
    ];
    ids.sort();
    assert_eq!(archive.store.ids(), ids, "one blob per distinct content");
    assert_eq!(
        archive.image,
        Some(RawImage::Held(ContentId::of(b"the image")))
    );
    let heads: Vec<ContentId> = archive.extras.iter().map(|e| e.head).collect();
    assert_eq!(
        heads,
        [ContentId::of(b"the image"), ContentId::of(b"a readme")]
    );
    assert_eq!(
        archive.store.get(same).as_deref(),
        Some(&b"the same bytes"[..])
    );
}

#[test]
fn a_name_is_a_hash_directly_under_its_wad_and_a_duplicate_by_a_whole_number() {
    // #MOD-ARTIFACT-0007: sixteen hex digits inside a folder are a path.
    // #MOD-ARTIFACT-0008: the number is everything after the first dot.
    let in_folder = "data/0123456789abcdef.bin";
    let bytes = zip(&[
        (
            "WAD/Lux.wad.client/data/0123456789abcdef.bin",
            b"at a path",
            STORED,
        ),
        ("WAD/Lux.wad.client/aaaaaaaaaaaaaaaa.bin.1", b"one", STORED),
        ("WAD/Lux.wad.client/bbbbbbbbbbbbbbbb.1.bin", b"two", STORED),
    ]);
    let (archive, log) = read(&bytes);
    let mut chunks: Vec<(WadHash, Option<&str>)> = archive.wads[0]
        .files
        .iter()
        .map(|(hash, file)| (*hash, file.path.as_ref().map(|p| p.as_str())))
        .collect();
    chunks.sort_by_key(|(_, path)| path.is_none());
    assert_eq!(chunks[0], (path_hash(in_folder), Some(in_folder)));
    assert_eq!(
        chunks[1..],
        [
            (WadHash(0xaaaa_aaaa_aaaa_aaaa), None),
            (WadHash(0xbbbb_bbbb_bbbb_bbbb), None),
        ]
    );
    assert!(log.is_empty(), "{:?}", codes(&log));
}

#[test]
fn entries_outside_the_wads_are_kept_as_a_name_a_size_and_first_bytes() {
    let long = vec![b'x'; 200];
    let mut bytes = zip(&[
        ("RAW/readme.txt", b"ignored", STORED),
        ("WAD/Lux.wad.client.pyntex.json", b"{}", DEFLATED),
        ("WAD/notes/todo.txt", &long, DEFLATED),
        ("tool.exe", b"MZ and more", STORED),
        ("docs/", b"", STORED),
        ("WAD/Lux.wad.client/data/", b"", STORED),
    ]);
    rewrite(&mut bytes, "tool.exe", Field::Method(14));
    let (archive, log) = read(&bytes);
    assert!(archive.wads.is_empty());
    let extras: Vec<(&str, u64, Vec<u8>)> = archive
        .extras
        .iter()
        .map(|e| {
            let head = archive.store.get(e.head).expect("the head is held");
            (e.name.raw(), e.size, head.to_vec())
        })
        .collect();
    assert_eq!(
        extras,
        [
            ("RAW/readme.txt", 7, b"ignored".to_vec()),
            ("WAD/Lux.wad.client.pyntex.json", 2, b"{}".to_vec()),
            ("WAD/notes/todo.txt", 200, long[..64].to_vec()),
            ("tool.exe", 11, Vec::new()),
        ],
        "a directory is no entry, and a codec that is not read leaves the name"
    );
    let opened = archive
        .open_extras(&archive.store)
        .expect("the store answers");
    let heads: Vec<&[u8]> = opened.iter().map(|e| e.head.as_slice()).collect();
    assert_eq!(heads[0], b"ignored");
    assert_eq!(heads[3], b"");
    assert!(log.is_empty(), "{:?}", codes(&log));
}

#[test]
fn the_mods_image_is_held_whole_and_no_other_extra_is() {
    let wad = ("WAD/Lux.wad.client/data/x.bin", &b"one"[..], STORED);
    let bytes = zip(&[
        wad,
        ("meta/IMAGE.png", b"the image", DEFLATED),
        ("META/image.png", b"a second image", STORED),
        ("META/image.jpg", b"another name", STORED),
    ]);
    let (archive, log) = read(&bytes);
    let id = ContentId::of(b"the image");
    assert_eq!(
        archive.image,
        Some(RawImage::Held(id)),
        "the first entry at the name, in any case"
    );
    assert_eq!(archive.store.get(id).as_deref(), Some(&b"the image"[..]));
    assert_eq!(
        archive.extras[1].head,
        ContentId::of(b"a second image"),
        "the second entry at the name is an extra, by its first bytes alone"
    );
    assert_eq!(archive.extras.len(), 3, "the image is an extra as before");
    assert!(log.is_empty(), "{:?}", codes(&log));

    let (archive, _) = read(&zip(&[wad, ("META/readme.md", b"text", STORED)]));
    assert_eq!(archive.image, None);
}

#[test]
fn an_image_over_the_cap_or_that_does_not_read_is_not_held_and_raises_nothing() {
    let wad = ("WAD/Lux.wad.client/data/x.bin", &b"one"[..], STORED);
    let large = vec![0u8; MAX_IMAGE_BYTES as usize + 1];
    let (archive, log) = read(&zip(&[wad, ("META/image.png", &large, DEFLATED)]));
    assert_eq!(archive.image, Some(RawImage::TooLarge(MAX_IMAGE_BYTES + 1)));
    assert!(log.is_empty(), "{:?}", codes(&log));

    let mut bytes = zip(&[wad, ("META/image.png", b"the image", STORED)]);
    rewrite(&mut bytes, "META/image.png", Field::Method(14));
    let (archive, log) = read(&bytes);
    assert_eq!(archive.image, Some(RawImage::Unreadable));
    assert!(log.is_empty(), "{:?}", codes(&log));
}

#[test]
fn a_name_is_read_in_any_case_and_with_either_separator() {
    let bytes = zip(&[
        ("meta/INFO.JSON", br#"{"Name":"Lux"}"#, STORED),
        ("wad\\Lux.wad.client\\DATA\\Sub\\X.BIN", b"one", STORED),
        ("/Wad/lux.WAD.client/data/y.bin", b"two", STORED),
    ]);
    let (archive, log) = read(&bytes);
    assert_eq!(archive.meta.name, Some(Spelled::new("Lux")));
    assert_eq!(archive.wads.len(), 1, "one WAD in any case of its name");
    let wad = &archive.wads[0];
    assert_eq!(wad.name.as_str(), "Lux.wad.client", "as first spelled");
    let mut paths: Vec<&str> = wad
        .files
        .values()
        .map(|f| f.path.as_ref().unwrap().as_str())
        .collect();
    paths.sort_unstable();
    assert_eq!(paths, ["data/sub/x.bin", "data/y.bin"]);
    assert!(archive.extras.is_empty());
    assert!(log.is_empty());
}

#[test]
fn wads_come_in_name_order_and_chunks_in_hash_order() {
    let bytes = zip(&[
        ("WAD/b.wad.client/data/1.bin", b"1", STORED),
        ("WAD/A.wad.client/data/3.bin", b"3", STORED),
        ("WAD/A.wad.client/data/2.bin", b"2", STORED),
        ("WAD/A.wad.client/data/1.bin", b"1", STORED),
    ]);
    let (archive, _) = read(&bytes);
    let names: Vec<&str> = archive.wads.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(names, ["A.wad.client", "b.wad.client"]);
    let hashes: Vec<WadHash> = archive.wads[0].files.keys().copied().collect();
    let mut sorted = hashes.clone();
    sorted.sort();
    assert_eq!(hashes, sorted);
    assert_eq!(hashes.len(), 3);
}

#[test]
fn a_packed_wad_is_mounted_from_memory() {
    // #MOD-ARTIFACT-0009
    let wad = packed_wad(&[(SKIN, b"inside a packed wad"), ("data/b.bin", b"second")]);
    let bytes = zip(&[("WAD/Lux.wad.client", &wad, STORED)]);
    let (archive, log) = read(&bytes);
    let lux = &archive.wads[0];
    assert_eq!(lux.layout, WadLayout::Packed);
    assert_eq!(lux.files.len(), 2);
    let skin = archive.file(0, path_hash(SKIN));
    assert_eq!(&*archive.bytes(skin), b"inside a packed wad");
    assert_eq!(skin.content, ContentId::of(b"inside a packed wad"));
    assert_eq!(skin.path, None, "a packed WAD spells no path");
    assert!(log.is_empty());
}

#[test]
fn a_chunk_has_the_kind_its_first_bytes_say() {
    let bin = b"PROP\x03\0\0\0\0\0\0\0";
    let patch = b"PTCH\x01\0\0\0\0\0\0\0";
    let wad = packed_wad(&[("data/packed.bin", bin)]);
    let bytes = zip(&[
        ("WAD/Lux.wad.client/data/a.bin", bin, STORED),
        ("WAD/Lux.wad.client/data/b.bin", patch, STORED),
        ("WAD/Lux.wad.client/data/c.txt", b"plain words", STORED),
        ("WAD/Ahri.wad.client", &wad, STORED),
    ]);
    let (archive, _) = read(&bytes);
    let kind = |wad: usize, path: &str| file_kind(&archive.bytes_at(wad, path));
    assert_eq!(kind(0, "data/packed.bin"), FileKind::Bin);
    assert_eq!(kind(1, "data/a.bin"), FileKind::Bin);
    assert_eq!(kind(1, "data/b.bin"), FileKind::Ptch);
    assert_eq!(kind(1, "data/c.txt"), FileKind::Other);
    assert_eq!(file_kind(b""), FileKind::Other);
}

#[test]
fn the_mod_id_is_the_first_twelve_hex_digits_of_the_archives_sha256() {
    let bytes = zip(&[("WAD/Lux.wad.client/data/x.bin", b"x", STORED)]);
    let (archive, _) = read(&bytes);
    let digest = ContentId::of(&bytes);
    assert_eq!(archive.meta.sha256, digest);
    assert_eq!(archive.meta.mod_id, digest.hex()[..12]);
    assert_eq!(archive.meta.mod_id.len(), 12);
    assert_eq!(archive.meta.file_name.raw(), "mod.fantome");
}

#[test]
fn info_is_read_past_a_byte_order_mark_and_in_any_case_of_its_keys() {
    let info = "\u{feff}{\"nAmE\": \"  Lux Mod \", \"AUTHOR\": \"someone\", \"version\": \"1.2\", \
                \"Description\": \"\"}";
    let bytes = zip(&[("META/info.json", info.as_bytes(), DEFLATED)]);
    let (archive, log) = read(&bytes);
    assert_eq!(archive.meta.name, Some(Spelled::new("Lux Mod")));
    assert_eq!(archive.meta.author, Some(Spelled::new("someone")));
    assert_eq!(archive.meta.version, Some(Spelled::new("1.2")));
    assert_eq!(archive.meta.description, None, "an empty field is none");
    assert!(log.is_empty());
}

#[test]
fn info_that_does_not_parse_is_noted_and_the_archive_is_read() {
    let bytes = zip(&[
        ("META/info.json", b"{not json", STORED),
        ("WAD/Lux.wad.client/data/x.bin", b"x", STORED),
    ]);
    let (archive, log) = read(&bytes);
    assert_eq!(archive.meta.name, None);
    assert_eq!(archive.wads[0].files.len(), 1);
    let unreadable = one::<InfoUnreadable>(&log);
    assert_eq!(unreadable.entry.raw(), "META/info.json");
    assert!(!unreadable.reason.is_empty());
    assert_eq!(log.worst(), Some(Severity::Note));
}

#[test]
fn a_name_that_is_no_path_inside_a_wad_is_skipped() {
    let bytes = zip(&[
        ("WAD/X.wad.client/../x.dds", b"a", STORED),
        ("WAD/X.wad.client/data//x.dds", b"b", STORED),
        ("WAD/X.wad.client/C:/x.dds", b"c", STORED),
        ("WAD/X.wad.client/data/nul.dds", b"d", STORED),
        ("WAD/X.wad.client/data/ok.dds", b"e", STORED),
        ("WAD/C:x.wad/data/y.dds", b"f", STORED),
        ("WAD/C:x.wad", b"g", STORED),
        ("WAD/con.wad.client/data/z.dds", b"h", STORED),
    ]);
    let (archive, log) = read(&bytes);
    assert_eq!(archive.wads.len(), 1);
    assert_eq!(archive.wads[0].name.as_str(), "X.wad.client");
    assert_eq!(archive.wads[0].files.len(), 1);
    assert_eq!(&*archive.bytes(archive.first(0)), b"e");
    assert!(archive.extras.is_empty(), "a refused name is no extra");

    let found: Vec<&NamesUnsafe> = log.of::<NamesUnsafe>().collect();
    assert_eq!(found.len(), 3, "one finding per WAD");
    assert_eq!(found[0].wad.raw(), "X.wad.client");
    let entries: Vec<&str> = found[0].entries.iter().map(|e| e.entry.raw()).collect();
    assert_eq!(
        entries,
        [
            "WAD/X.wad.client/../x.dds",
            "WAD/X.wad.client/data//x.dds",
            "WAD/X.wad.client/C:/x.dds",
            "WAD/X.wad.client/data/nul.dds",
        ]
    );
    let why: Vec<&PathError> = found[0].entries.iter().map(|e| &e.why).collect();
    assert_eq!(
        why,
        [
            &PathError::DotPart("..".into()),
            &PathError::EmptyPart,
            &PathError::Character(':'),
            &PathError::Device("nul.dds".into()),
        ]
    );
    assert_eq!(found[1].wad.raw(), "C:x.wad");
    assert_eq!(found[1].entries.len(), 2, "its chunk and its packed form");
    assert!(
        found[1]
            .entries
            .iter()
            .all(|e| e.why == PathError::Character(':'))
    );
    assert_eq!(found[2].wad.raw(), "con.wad.client");
    assert_eq!(
        found[2].entries[0].why,
        PathError::Device("con.wad.client".into())
    );
    assert_eq!(log.worst(), Some(Severity::Error));
}

#[test]
fn an_entry_declaring_more_than_the_cap_is_skipped() {
    let big = vec![7u8; 4096];
    let bytes = zip(&[
        ("WAD/Lux.wad.client/data/big.bin", &big, DEFLATED),
        ("WAD/Lux.wad.client/data/small.bin", b"ok", STORED),
    ]);
    let caps = Caps {
        entry_bytes: 1024,
        ..Caps::default()
    };
    let (archive, log) = read_with(&bytes, &caps);
    let archive = archive.unwrap();
    assert_eq!(archive.wads[0].files.len(), 1);
    let large = one::<EntriesTooLarge>(&log);
    assert_eq!(large.within.raw(), "Lux.wad.client");
    assert_eq!(large.cap, 1024);
    assert_eq!(large.entries.len(), 1);
    assert_eq!(
        large.entries[0].entry.raw(),
        "WAD/Lux.wad.client/data/big.bin"
    );
    assert_eq!(large.entries[0].declared, 4096);
}

#[test]
fn an_entry_inflating_past_what_it_declares_is_skipped() {
    let big = vec![7u8; 4096];
    let mut bytes = zip(&[("WAD/Lux.wad.client/data/big.bin", &big, DEFLATED)]);
    rewrite(
        &mut bytes,
        "WAD/Lux.wad.client/data/big.bin",
        Field::Size(10),
    );
    let caps = Caps {
        entry_bytes: 1024,
        ..Caps::default()
    };
    let (archive, log) = read_with(&bytes, &caps);
    assert!(archive.unwrap().wads.is_empty());
    let large = one::<EntriesTooLarge>(&log);
    assert_eq!(large.entries[0].declared, 10);
}

#[test]
fn more_entries_than_the_cap_end_the_read() {
    let bytes = zip(&[
        ("WAD/Lux.wad.client/a.bin", b"a", STORED),
        ("WAD/Lux.wad.client/b.bin", b"b", STORED),
    ]);
    let caps = Caps {
        entries: 1,
        ..Caps::default()
    };
    let (result, log) = read_with(&bytes, &caps);
    assert_eq!(result.err(), log.halted());
    assert_eq!(
        one::<TooManyEntries>(&log),
        &TooManyEntries { entries: 2, cap: 1 }
    );
}

#[test]
fn the_inflate_budget_ends_the_read_and_what_was_found_before_is_kept() {
    let chunk = vec![1u8; 600];
    let bytes = zip(&[
        ("WAD/Lux.wad.client/0123456789abcdef.1", b"kept", STORED),
        ("WAD/Lux.wad.client/a.bin", &chunk, STORED),
        ("WAD/Lux.wad.client/b.bin", &chunk, DEFLATED),
        ("WAD/Lux.wad.client/c.bin", &chunk, STORED),
    ]);
    let caps = Caps {
        total_bytes: 1000,
        ..Caps::default()
    };
    let (result, log) = read_with(&bytes, &caps);
    let halted = result.unwrap_err();
    assert_eq!(log.halted(), Some(halted));
    assert_eq!(
        one::<BudgetExceeded>(&log),
        &BudgetExceeded::Inflated {
            cap: 1000,
            entry: Spelled::new("WAD/Lux.wad.client/b.bin"),
        }
    );
    assert_eq!(
        codes(&log),
        ["archive.repack-duplicate", "archive.budget-exceeded"]
    );
    assert_eq!(log.get(halted.0).unwrap().subject, Subject::Run);
}

#[test]
fn a_read_that_ends_early_puts_nothing_in_the_store() {
    // #ADR-0093: the bytes go into the store when the read is whole, so a halted read
    // leaves nothing behind.
    let chunk = vec![1u8; 600];
    let bytes = zip(&[
        ("WAD/Lux.wad.client/a.bin", &chunk, STORED),
        ("WAD/Lux.wad.client/b.bin", &chunk, DEFLATED),
    ]);
    let caps = Caps {
        total_bytes: 1000,
        ..Caps::default()
    };
    let mut log = Diagnostics::new();
    let store = MemoryStore::new();
    let result = archive::read(
        &bytes,
        "mod.fantome",
        &caps,
        &store,
        &mut log.at(Stage::Read),
    );
    assert!(result.is_err());
    assert!(store.is_empty());
}

#[test]
fn a_packed_chunk_is_charged_by_its_declared_size_before_it_is_decompressed() {
    let zeros = vec![0u8; 600];
    let wad = packed_wad(&[("data/a.bin", &zeros), ("data/b.bin", &zeros)]);
    assert!(wad.len() < 500, "600 zeros compress: {} bytes", wad.len());
    let bytes = zip(&[("WAD/Lux.wad.client", &wad, STORED)]);
    let caps = Caps {
        total_bytes: 1500,
        ..Caps::default()
    };
    let (result, log) = read_with(&bytes, &caps);
    assert!(result.is_err());
    assert_eq!(
        one::<BudgetExceeded>(&log),
        &BudgetExceeded::Inflated {
            cap: 1500,
            entry: Spelled::new("WAD/Lux.wad.client"),
        }
    );
    let enough = Caps {
        total_bytes: wad.len() as u64 + 1200,
        ..Caps::default()
    };
    let (result, log) = read_with(&bytes, &enough);
    assert_eq!(result.unwrap().wads[0].files.len(), 2);
    assert!(log.is_empty());
}

#[test]
fn a_packed_wad_over_a_cap_is_not_read() {
    let chunk = vec![3u8; 2000];
    let wad = packed_wad(&[("data/a.bin", &chunk), ("data/b.bin", b"small")]);
    let bytes = zip(&[
        ("WAD/Lux.wad.client", &wad, STORED),
        ("WAD/Ahri.wad.client/data/x.bin", b"x", STORED),
    ]);

    let few_chunks = Caps {
        packed_chunks: 1,
        ..Caps::default()
    };
    let (archive, log) = read_with(&bytes, &few_chunks);
    let archive = archive.unwrap();
    assert_eq!(archive.wads.len(), 1, "the archive is read on without it");
    assert_eq!(archive.wads[0].name.as_str(), "Ahri.wad.client");
    let unreadable = one::<WadUnreadable>(&log);
    assert_eq!(unreadable.wad.raw(), "Lux.wad.client");
    assert_eq!(unreadable.entries[0].entry.raw(), "WAD/Lux.wad.client");
    assert_eq!(
        unreadable.entries[0].why,
        PackedError::TooManyChunks { chunks: 2, cap: 1 }
    );

    let small_chunks = Caps {
        entry_bytes: 1000,
        ..Caps::default()
    };
    let (archive, log) = read_with(&bytes, &small_chunks);
    assert_eq!(archive.unwrap().wads.len(), 1);
    assert_eq!(
        one::<WadUnreadable>(&log).entries[0].why,
        PackedError::ChunkTooLarge {
            chunk: path_hash("data/a.bin"),
            bytes: 2000,
            cap: 1000
        }
    );
}

#[test]
fn a_packed_wad_that_does_not_mount_is_skipped() {
    let bytes = zip(&[
        ("WAD/Lux.wad.client", b"RW\x03\x04 and nothing else", STORED),
        ("WAD/Ahri.wad.client", b"not a WAD at all", DEFLATED),
        ("WAD/Zed.wad.client", b"RW\x01\x00 an old version", STORED),
        ("WAD/Jax.wad.client/data/x.bin", b"x", STORED),
    ]);
    let (archive, log) = read(&bytes);
    assert_eq!(archive.wads.len(), 1);
    assert_eq!(archive.wads[0].name.as_str(), "Jax.wad.client");
    let found: Vec<&WadUnreadable> = log.of::<WadUnreadable>().collect();
    let wads: Vec<&str> = found.iter().map(|f| f.wad.raw()).collect();
    assert_eq!(
        wads,
        ["Lux.wad.client", "Ahri.wad.client", "Zed.wad.client"]
    );
    assert!(matches!(found[0].entries[0].why, PackedError::Mount { .. }));
    assert!(matches!(found[1].entries[0].why, PackedError::Mount { .. }));
    assert_eq!(found[2].entries[0].why, PackedError::Version { major: 1 });
}

#[test]
fn a_packed_wad_whose_table_lies_is_refused_before_memory_is_reserved_for_it() {
    let good = packed_wad(&[("data/a.bin", b"some bytes of a chunk")]);

    let mut many = good.clone();
    put_u32(&mut many, WAD_COUNT_AT, i32::MAX as u32);
    let mut past_end = good.clone();
    let compressed_size_at = WAD_TABLE_AT + 8 + 4;
    put_u32(&mut past_end, compressed_size_at, good.len() as u32 + 1);
    let mut damaged = good.clone();
    let data_at = get_u32(&good, WAD_TABLE_AT + 8) as usize;
    let data_len = get_u32(&good, compressed_size_at) as usize;
    damaged[data_at..data_at + data_len].fill(0xff);

    let bytes = zip(&[
        ("WAD/Many.wad.client", &many, STORED),
        ("WAD/PastEnd.wad.client", &past_end, STORED),
        ("WAD/Damaged.wad.client", &damaged, STORED),
        ("WAD/Good.wad.client", &good, STORED),
    ]);
    let (archive, log) = read(&bytes);
    assert_eq!(archive.wads.len(), 1);
    assert_eq!(archive.wads[0].name.as_str(), "Good.wad.client");
    let why: BTreeMap<&str, &PackedError> = log
        .of::<WadUnreadable>()
        .map(|f| (f.wad.raw(), &f.entries[0].why))
        .collect();
    assert_eq!(
        why["Many.wad.client"],
        &PackedError::TableLargerThanData {
            chunks: i32::MAX as u64,
            bytes: good.len() as u64,
            fit: (good.len() - WAD_TABLE_AT) as u64 / 32,
        }
    );
    assert_eq!(
        why["PastEnd.wad.client"],
        &PackedError::ChunkPastEnd {
            chunk: path_hash("data/a.bin")
        }
    );
    assert!(
        matches!(
            why["Damaged.wad.client"],
            PackedError::ChunkUnreadable { chunk, .. } if *chunk == path_hash("data/a.bin")
        ),
        "{:?}",
        why["Damaged.wad.client"]
    );
}

#[test]
fn a_packed_wad_the_reader_panics_on_is_skipped_and_the_archive_is_read_on() {
    // `ltk_wad` 0.5.8 looks for the table of subchunks as it mounts. A chunk stored in
    // subchunks that declares no subchunk and no inflated byte is read as that table, and the
    // bytes before its first Zstandard frame are copied into a buffer of no length.
    let good = packed_wad(&[("data/a.bin", b"some bytes of a chunk")]);
    let mut crafted = good.clone();
    let (data_at, compressed_size_at) = (WAD_TABLE_AT + 8, WAD_TABLE_AT + 12);
    let (uncompressed_size_at, compression_at) = (WAD_TABLE_AT + 16, WAD_TABLE_AT + 20);
    // One byte before the frame, so the frame is not the chunk's first byte.
    put_u32(&mut crafted, data_at, get_u32(&good, data_at) - 1);
    let compressed = get_u32(&good, compressed_size_at) + 1;
    put_u32(&mut crafted, compressed_size_at, compressed);
    put_u32(&mut crafted, uncompressed_size_at, 0);
    // Compression 4, in subchunks, with no subchunk and the first of them at 0.
    put_u32(&mut crafted, compression_at, 4);

    let bytes = zip(&[
        ("WAD/Lux.wad.client", &crafted, STORED),
        ("WAD/Ahri.wad.client/data/x.bin", b"x", STORED),
    ]);
    let (archive, log) = read(&bytes);
    assert_eq!(archive.wads.len(), 1);
    assert_eq!(archive.wads[0].name.as_str(), "Ahri.wad.client");
    let unreadable = one::<WadUnreadable>(&log);
    assert_eq!(unreadable.wad.raw(), "Lux.wad.client");
    let PackedError::Panicked { reason } = &unreadable.entries[0].why else {
        panic!(
            "the panic was not caught as an error: {:?}",
            unreadable.entries[0].why
        );
    };
    assert!(reason.contains("out of range"), "{reason}");
    assert_eq!(codes(&log), ["archive.wad-unreadable"]);
}

#[test]
fn a_stored_entry_with_a_wrong_checksum_is_read_and_noted() {
    // #MOD-ARTIFACT-0006
    let name = "WAD/Lux.wad.client/data/x.bin";
    let mut bytes = zip(&[(name, b"stored bytes", STORED)]);
    rewrite(&mut bytes, name, Field::Crc(0xdead_beef));
    let (archive, log) = read(&bytes);
    assert_eq!(&*archive.bytes(archive.first(0)), b"stored bytes");
    let wrong = one::<StoredCrcWrong>(&log);
    assert_eq!(wrong.within.raw(), "Lux.wad.client");
    assert_eq!(spelled(&wrong.entries), [name]);
    assert_eq!(log.worst(), Some(Severity::Note));
}

#[test]
fn a_packed_wad_stored_with_a_wrong_checksum_is_mounted_and_noted() {
    // #MOD-ARTIFACT-0006: every entry seen with it holds a WAD packed whole, and the content
    // tells the artifact from damage.
    let wad = packed_wad(&[(SKIN, b"inside a packed wad")]);
    let (intact, damaged) = ("WAD/Lux.wad.client", "WAD/Ahri.wad.client");
    let mut bytes = zip(&[
        (intact, &wad, STORED),
        (damaged, b"RW\x03\x04 and nothing else", STORED),
    ]);
    rewrite(&mut bytes, intact, Field::Crc(0xdead_beef));
    rewrite(&mut bytes, damaged, Field::Crc(0xdead_beef));
    let (archive, log) = read(&bytes);

    assert_eq!(archive.wads.len(), 1);
    let lux = &archive.wads[0];
    assert_eq!(
        (lux.name.as_str(), lux.layout),
        ("Lux.wad.client", WadLayout::Packed)
    );
    assert_eq!(&*archive.bytes(archive.first(0)), b"inside a packed wad");
    let wrong: Vec<(&str, Vec<&str>)> = log
        .of::<StoredCrcWrong>()
        .map(|found| (found.within.raw(), spelled(&found.entries)))
        .collect();
    assert_eq!(
        wrong,
        [
            ("Lux.wad.client", vec![intact]),
            ("Ahri.wad.client", vec![damaged])
        ]
    );
    let unreadable = one::<WadUnreadable>(&log);
    assert_eq!(unreadable.wad.raw(), "Ahri.wad.client");
    assert!(matches!(
        unreadable.entries[0].why,
        PackedError::Mount { .. }
    ));
}

#[test]
fn a_compressed_entry_that_fails_its_checksum_is_skipped() {
    // #MOD-ARTIFACT-0006: a compressed entry that fails its checksum is not the artifact.
    let name = "WAD/Lux.wad.client/data/x.bin";
    let mut bytes = zip(&[
        (name, b"deflated bytes, deflated bytes", DEFLATED),
        ("WAD/Lux.wad.client/data/y.bin", b"fine", DEFLATED),
    ]);
    rewrite(&mut bytes, name, Field::Crc(0xdead_beef));
    let (archive, log) = read(&bytes);
    assert_eq!(archive.wads[0].files.len(), 1);
    assert_eq!(&*archive.bytes(archive.first(0)), b"fine");
    let corrupt = one::<EntriesCorrupt>(&log);
    assert_eq!(corrupt.within.raw(), "Lux.wad.client");
    assert_eq!(spelled(&corrupt.entries), [name]);
    assert_eq!(codes(&log), ["archive.entry-corrupt"]);
}

#[test]
fn an_entry_in_a_codec_other_than_deflate_or_stored_is_skipped() {
    let mut bytes = zip(&[
        ("WAD/Lux.wad.client/data/x.bin", b"as bzip2", STORED),
        ("WAD/Lux.wad.client/data/y.bin", b"as zstd", STORED),
        ("WAD/Lux.wad.client/data/z.bin", b"fine", STORED),
        ("META/info.json", b"{}", STORED),
    ]);
    rewrite(
        &mut bytes,
        "WAD/Lux.wad.client/data/x.bin",
        Field::Method(12),
    );
    rewrite(
        &mut bytes,
        "WAD/Lux.wad.client/data/y.bin",
        Field::Method(93),
    );
    rewrite(&mut bytes, "META/info.json", Field::Method(14));
    let (archive, log) = read(&bytes);
    assert_eq!(archive.wads[0].files.len(), 1);
    let found: Vec<&CodecsUnsupported> = log.of::<CodecsUnsupported>().collect();
    assert_eq!(
        found.len(),
        2,
        "one for the WAD, one for the entry under none"
    );
    assert_eq!(found[0].within.raw(), "Lux.wad.client");
    let methods: Vec<u16> = found[0].entries.iter().map(|e| e.method).collect();
    assert_eq!(methods, [12, 93]);
    assert_eq!(found[1].within.raw(), "META/info.json");
    assert_eq!(found[1].entries[0].method, 14);
}

#[test]
fn an_entry_that_does_not_read_is_skipped_under_its_own_code() {
    let name = "WAD/Lux.wad.client/data/x.bin";
    let mut bytes = zip(&[
        (name, &[0xff; 40], STORED),
        ("WAD/Lux.wad.client/data/y.bin", b"fine", STORED),
    ]);
    // Stored bytes read as a deflate stream: the stream is damaged, not its checksum.
    rewrite(&mut bytes, name, Field::Method(8));
    let (archive, log) = read(&bytes);
    assert_eq!(archive.wads[0].files.len(), 1);
    let unreadable = one::<EntriesUnreadable>(&log);
    assert_eq!(unreadable.within.raw(), "Lux.wad.client");
    assert_eq!(unreadable.entries[0].entry.raw(), name);
    assert!(!unreadable.entries[0].reason.is_empty());
    assert_eq!(codes(&log), ["archive.entry-unreadable"]);
}

#[test]
fn bytes_that_are_no_zip_end_the_read() {
    let (result, log) = read_with(b"definitely not a zip", &Caps::default());
    let halted = result.unwrap_err();
    assert_eq!(log.halted(), Some(halted));
    let not_zip = one::<NotZip>(&log);
    assert_eq!(not_zip.file.raw(), "mod.fantome");
    assert!(!not_zip.reason.is_empty());
    assert_eq!(log.worst(), Some(Severity::Fatal));
}

#[test]
fn an_archive_file_is_read_from_disk_under_the_cap_for_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .expect("the temporary directory is UTF-8");
    let path = root.join("lux.fantome");
    let bytes = zip(&[("WAD/Lux.wad.client/data/x.bin", b"x", STORED)]);
    fs::write(&path, &bytes).unwrap();

    let mut log = Diagnostics::new();
    let store = MemoryStore::new();
    let caps = Caps::default();
    let raw = archive::read_file(&path, &caps, &store, &mut log.at(Stage::Read)).unwrap();
    assert_eq!(raw.meta.file_name.raw(), "lux.fantome");
    assert_eq!(raw.meta.sha256, ContentId::of(&bytes));
    assert_eq!(raw.wads[0].files.len(), 1);
    assert_eq!(store.len(), 1);
    assert!(log.is_empty());

    let caps = Caps {
        archive_bytes: 10,
        ..Caps::default()
    };
    let over = BudgetExceeded::ArchiveFile {
        bytes: bytes.len() as u64,
        cap: 10,
    };
    let mut log = Diagnostics::new();
    let store = MemoryStore::new();
    assert!(archive::read_file(&path, &caps, &store, &mut log.at(Stage::Read)).is_err());
    assert_eq!(one::<BudgetExceeded>(&log), &over);
    let (result, log) = read_with(&bytes, &caps);
    assert!(result.is_err(), "bytes in memory are held to the same cap");
    assert_eq!(one::<BudgetExceeded>(&log), &over);

    let mut log = Diagnostics::new();
    let missing = root.join("missing.fantome");
    let caps = Caps::default();
    assert!(archive::read_file(&missing, &caps, &store, &mut log.at(Stage::Read)).is_err());
    assert_eq!(one::<NotZip>(&log).file.raw(), "missing.fantome");
}

#[test]
fn the_first_chunk_at_a_path_stays_and_the_later_ones_are_noted() {
    // #MOD-ARTIFACT-0007: an entry named by its path and one named by the hash of that path.
    let by_hash = format!("WAD/Lux.wad.client/{:016x}.bin", path_hash("data/y.bin").0);
    let bytes = zip(&[
        ("WAD/Lux.wad.client/data/x.bin", b"first", STORED),
        ("WAD/Lux.wad.client/DATA/X.BIN", b"second", STORED),
        (&by_hash, b"by hash", STORED),
        ("WAD/Lux.wad.client/data/y.bin", b"by path", STORED),
    ]);
    let (archive, log) = read(&bytes);
    let wad = &archive.wads[0];
    assert_eq!(wad.files.len(), 2);
    assert_eq!(&*archive.bytes_at(0, "data/x.bin"), b"first");
    assert_eq!(&*archive.bytes_at(0, "data/y.bin"), b"by hash");
    assert_eq!(
        archive
            .file(0, path_hash("data/y.bin"))
            .path
            .as_ref()
            .map(|p| p.as_str()),
        Some("data/y.bin"),
        "the later entry spells the path of the chunk that stays"
    );
    let duplicates = one::<ChunkDuplicates>(&log);
    assert_eq!(duplicates.wad.raw(), "Lux.wad.client");
    let skipped: Vec<(WadHash, &str)> = duplicates
        .chunks
        .iter()
        .map(|c| (c.hash, c.entry.raw()))
        .collect();
    assert_eq!(
        skipped,
        [
            (path_hash("data/x.bin"), "WAD/Lux.wad.client/DATA/X.BIN"),
            (path_hash("data/y.bin"), "WAD/Lux.wad.client/data/y.bin"),
        ]
    );
    assert_eq!(log.worst(), Some(Severity::Note));
}

#[test]
fn the_loose_and_the_packed_form_of_one_wad_are_one_wad() {
    // #MOD-ARTIFACT-0009: a WAD the archive holds both as a file and as a folder.
    let ahri = packed_wad(&[("data/a.bin", b"packed a"), ("data/b.bin", b"packed b")]);
    let zed = packed_wad(&[("data/a.bin", b"packed a"), ("data/b.bin", b"packed b")]);
    let jax = packed_wad(&[("data/a.bin", b"first packed")]);
    let jax_again = packed_wad(&[("data/a.bin", b"second packed"), ("data/c.bin", b"c")]);
    let bytes = zip(&[
        ("WAD/Ahri.wad.client", &ahri, STORED),
        ("WAD/ahri.wad.client/data/a.bin", b"loose a", STORED),
        ("WAD/ahri.wad.client/data/c.bin", b"loose c", STORED),
        ("WAD/Zed.wad.client/data/a.bin", b"loose a", STORED),
        ("WAD/Zed.wad.client", &zed, DEFLATED),
        ("WAD/Jax.wad.client", &jax, STORED),
        ("WAD/JAX.WAD.CLIENT", &jax_again, STORED),
    ]);
    let (archive, log) = read(&bytes);
    let names: Vec<&str> = archive.wads.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(
        names,
        ["Ahri.wad.client", "Jax.wad.client", "Zed.wad.client"]
    );
    let bytes_of = |wad: usize, path: &str| {
        let file = archive.file(wad, path_hash(path));
        (
            archive.bytes(file).to_vec(),
            file.path.as_ref().map(|p| p.as_str()),
        )
    };
    for wad in &archive.wads {
        assert_eq!(wad.layout, WadLayout::Packed, "{}", wad.name);
    }

    assert_eq!(archive.wads[0].files.len(), 3);
    assert_eq!(
        bytes_of(0, "data/a.bin"),
        (b"packed a".to_vec(), Some("data/a.bin")),
        "the packed chunk came first, and the loose entry spells its path"
    );
    assert_eq!(bytes_of(0, "data/b.bin"), (b"packed b".to_vec(), None));
    assert_eq!(
        bytes_of(0, "data/c.bin"),
        (b"loose c".to_vec(), Some("data/c.bin"))
    );

    assert_eq!(archive.wads[1].files.len(), 2);
    assert_eq!(bytes_of(1, "data/a.bin").0, b"first packed");
    assert_eq!(bytes_of(1, "data/c.bin").0, b"c");

    assert_eq!(archive.wads[2].files.len(), 2);
    assert_eq!(bytes_of(2, "data/a.bin").0, b"loose a");
    assert_eq!(bytes_of(2, "data/b.bin").0, b"packed b");

    let duplicates: Vec<(&str, Vec<&str>)> = log
        .of::<ChunkDuplicates>()
        .map(|f| {
            (
                f.wad.raw(),
                f.chunks.iter().map(|c| c.entry.raw()).collect(),
            )
        })
        .collect();
    assert_eq!(
        duplicates,
        [
            ("Ahri.wad.client", vec!["WAD/ahri.wad.client/data/a.bin"]),
            ("Zed.wad.client", vec!["WAD/Zed.wad.client"]),
            ("Jax.wad.client", vec!["WAD/JAX.WAD.CLIENT"]),
        ]
    );
    assert!(
        log.of::<ChunkDuplicates>()
            .all(|f| f.chunks[0].hash == path_hash("data/a.bin"))
    );
}

#[test]
fn a_code_is_raised_once_per_subject_with_every_instance_in_it() {
    // #MOD-ARTIFACT-0008: `.1` and `.2` are numbers, and `.7.bin` is not.
    let bytes = zip(&[
        ("WAD/Lux.wad.client/0123456789abcdef.1", b"1", STORED),
        ("WAD/Ahri.wad.client/0123456789abcdef.1", b"2", STORED),
        ("WAD/lux.wad.client/0123456789abcdef.2", b"3", STORED),
        ("WAD/Lux.wad.client/aaaaaaaaaaaaaaaa.7.bin", b"4", STORED),
    ]);
    let (archive, log) = read(&bytes);
    assert_eq!(
        archive.wads.len(),
        1,
        "a WAD of repack duplicates alone is none"
    );
    assert_eq!(
        archive.wads[0].files.keys().next(),
        Some(&WadHash(0xaaaa_aaaa_aaaa_aaaa))
    );
    let found: Vec<(&str, Vec<&str>)> = log
        .of::<RepackDuplicates>()
        .map(|f| (f.wad.raw(), spelled(&f.entries)))
        .collect();
    assert_eq!(
        found,
        [
            (
                "Lux.wad.client",
                vec![
                    "WAD/Lux.wad.client/0123456789abcdef.1",
                    "WAD/lux.wad.client/0123456789abcdef.2"
                ]
            ),
            (
                "Ahri.wad.client",
                vec!["WAD/Ahri.wad.client/0123456789abcdef.1"]
            ),
        ]
    );
    for diagnostic in log.iter() {
        assert_eq!(diagnostic.stage, Stage::Read);
        assert!(matches!(diagnostic.subject, Subject::Entry(_)));
    }
}

#[test]
fn text_of_the_archive_prints_cleaned() {
    let bytes = zip(&[
        (
            "META/info.json",
            b"{\"Name\": \"Lux\\u001b[31m\", \"Author\": \"some\\u202eone\"}",
            STORED,
        ),
        ("RAW/read\u{1b}me.txt", b"x", STORED),
        ("WAD/Lux.wad.client/data/a\u{1b}[2J.dds", b"x", STORED),
        ("WAD/Lu\u{7}x.wad.client/data/b.dds", b"x", STORED),
    ]);
    let mut log = Diagnostics::new();
    let archive = archive::read(
        &bytes,
        "mod\u{1b}.fantome",
        &Caps::default(),
        &MemoryStore::new(),
        &mut log.at(Stage::Read),
    )
    .unwrap();
    let name = archive.meta.name.as_ref().unwrap();
    assert_eq!(name.raw(), "Lux\u{1b}[31m");
    assert_eq!(name.to_string(), "Lux\\u{1b}[31m");
    assert_eq!(
        archive.meta.author.as_ref().unwrap().to_string(),
        "some\\u{202e}one"
    );
    assert_eq!(archive.meta.file_name.to_string(), "mod\\u{1b}.fantome");
    assert_eq!(archive.extras[0].name.to_string(), "RAW/read\\u{1b}me.txt");
    assert!(archive.wads.is_empty(), "a name with a control is no path");

    assert_eq!(log.of::<NamesUnsafe>().count(), 2);
    let cx = Render::plain();
    for diagnostic in log.iter() {
        let printed = format!(
            "{} {}",
            cx.subject_name(&diagnostic.subject),
            diagnostic.message(&cx)
        );
        assert!(
            !printed.contains(['\u{1b}', '\u{7}']),
            "a control reaches the terminal: {printed:?}"
        );
        assert!(printed.contains("\\u{"), "{printed:?}");
    }
}

#[test]
fn two_reads_of_one_archive_give_the_same_diagnostics_in_the_same_order() {
    let wad = packed_wad(&[("data/a.bin", b"packed")]);
    let mut bytes = zip(&[
        ("WAD/Lux.wad.client/data/a.bin", b"loose", STORED),
        ("WAD/Lux.wad.client", &wad, STORED),
        ("WAD/Lux.wad.client/0123456789abcdef.1", b"1", STORED),
        ("WAD/Ahri.wad.client/../x.bin", b"x", STORED),
        ("WAD/Ahri.wad.client/data/x.bin", b"x", STORED),
        ("META/info.json", b"[", STORED),
    ]);
    rewrite(
        &mut bytes,
        "WAD/Ahri.wad.client/data/x.bin",
        Field::Method(12),
    );
    let render = |log: &Diagnostics| -> Vec<serde_json::Value> {
        log.iter().map(|d| d.to_json(&Render::plain())).collect()
    };
    let (_, first) = read(&bytes);
    let (_, second) = read(&bytes);
    assert_eq!(render(&first), render(&second));
    assert_eq!(
        codes(&first),
        [
            "archive.codec-unsupported",
            "archive.name-unsafe",
            "archive.repack-duplicate",
            "archive.chunk-duplicate",
            "archive.info-unreadable",
        ]
    );
    let json = &render(&first)[3];
    assert_eq!(json["stage"], "read");
    assert_eq!(json["channel"], "input");
    assert_eq!(json["subject"]["kind"], "entry");
    assert_eq!(json["subject"]["id"], "Lux.wad.client");
    assert_eq!(
        json["data"]["chunks"][0]["hash"],
        format!("{:016x}", path_hash("data/a.bin").0)
    );
    assert_eq!(json["data"]["chunks"][0]["entry"], "WAD/Lux.wad.client");
}

/// The code and severity of every row of the tables in `CODES.md`.
fn documented() -> BTreeMap<String, String> {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design/CODES.md");
    let text = fs::read_to_string(file).unwrap();
    text.lines()
        .filter_map(|line| {
            let mut cells = line.strip_prefix("| `")?.split('|');
            let code = cells.next()?.trim().trim_end_matches('`');
            let severity = cells.next()?.trim();
            Some((code.to_string(), severity.to_string()))
        })
        .collect()
}

#[test]
fn the_codes_are_the_archive_table_of_the_codes_doc_less_the_one_load_raises() {
    let documented = documented();
    for code in CODES {
        assert_eq!(
            documented.get(code.name).map(String::as_str),
            Some(code.severity.as_str()),
            "{code}"
        );
        assert_eq!(code.channel, Channel::Input, "{code}");
    }
    let mut ours: Vec<&str> = CODES.iter().map(|code| code.name).collect();
    ours.sort_unstable();
    let theirs: Vec<&str> = documented
        .keys()
        .map(String::as_str)
        .filter(|code| code.starts_with("archive.") && *code != "archive.no-skin")
        .collect();
    assert_eq!(ours, theirs);
}
