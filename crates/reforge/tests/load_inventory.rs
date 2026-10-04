//! The inventory: what the archive carries by kind, and what of it is no game file, reported
//! once per finding. The archive's own metadata is not the mod's content.

use std::collections::BTreeMap;
use std::sync::Arc;

use reforge::archive::file_kind;
use reforge::diag::{Channel, Code, Diag, Diagnostics, Record, Render, Severity, Stage, Subject};
use reforge::load::inventory::{
    CODES, Executables, Finding, Inventory, Item, NestedArchives, OsArtifacts, Sources, Tally,
    TextBins, UnknownFiles,
};
use reforge::model::entity::{
    Extra, FileId, FileNode, FileSource, FileUse, MadeBy, ModMeta, ModWad, WadId, WadLayout,
    WadRole,
};
use reforge::model::{
    BinHash, ChunkPath, ContentId, Mod, NoNames, Spell, Spelled, WadHash, WadName, path_hash,
};
use reforge_magic::{Confidence, Kind};

const WAD: &str = "Test.wad.client";
const SKIN_BIN: &str = "data/characters/test/skins/skin0.bin";
const TEXT_BIN: &str = "data/characters/test/skins/skin1.bin";

/// The opening of a binary bin.
const BIN: &[u8] = b"PROP\x03\0\0\0\0\0\0\0\0\0\0\0";
/// The opening of a bin as ritobin's text.
const BIN_TEXT: &[u8] = b"#PROP_text\ntype: string = \"PROP\"\n";
const MAYA: &[u8] = b"//Maya ASCII 2020 scene\n";
const ZIP: &[u8] = b"PK\x03\x04\x14\0\0\0\x08\0";
const APPLE_DOUBLE: &[u8] = b"\x00\x05\x16\x07\x00\x02\x00\x00";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
/// Bytes no table knows, and no text.
const NOISE: &[u8] = &[0xAB; 32];

/// The opening of a Windows program: `MZ`, and where its PE header starts.
fn pe() -> Vec<u8> {
    let mut bytes = vec![0u8; 256];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    bytes
}

/// A mod of one champion WAD with no file in it.
fn empty_mod() -> (Mod, WadId) {
    let mut m = Mod::new(ModMeta {
        file_name: Spelled::new("synthetic.fantome"),
        sha256: ContentId::of(b"synthetic"),
        mod_id: "000000000000".into(),
        name: None,
        author: None,
        version: None,
        description: None,
    });
    let wad = m.wads.push(ModWad {
        name: WadName::parse(WAD).expect("a WAD name"),
        role: WadRole::Champion {
            champion: "test".into(),
        },
        layout: WadLayout::Loose,
        files: Vec::new(),
        packed_locale: None,
    });
    (m, wad)
}

fn node(wad: WadId, hash: WadHash, path: Option<&str>, bytes: &[u8]) -> FileNode {
    FileNode {
        wad,
        hash,
        path: path.map(|p| ChunkPath::parse(p).expect("a chunk path")),
        size: bytes.len() as u64,
        content: ContentId::of(bytes),
        kind: file_kind(bytes),
        bytes: Arc::from(bytes),
        used: FileUse::Unused,
        source: FileSource::Archive,
    }
}

/// A file the archive spells the path of.
fn file(m: &mut Mod, wad: WadId, path: &str, bytes: &[u8]) -> FileId {
    m.push_file(node(wad, path_hash(path), Some(path), bytes))
        .expect("the path is free")
}

/// A chunk the archive names by its hash.
fn hashed(m: &mut Mod, wad: WadId, path: &str, bytes: &[u8], used: FileUse) -> FileId {
    let mut file = node(wad, path_hash(path), None, bytes);
    file.used = used;
    m.push_file(file).expect("the path is free")
}

fn extra(m: &mut Mod, name: &str, size: u64, head: &[u8]) {
    m.extras.push(Extra {
        name: Spelled::new(name),
        size,
        head: head.to_vec(),
    });
}

/// Spells the paths a test names.
#[derive(Default)]
struct Table(BTreeMap<WadHash, String>);

impl Table {
    fn of(paths: &[&str]) -> Self {
        Self(
            paths
                .iter()
                .map(|p| (path_hash(p), (*p).to_string()))
                .collect(),
        )
    }
}

impl Spell for Table {
    fn path(&self, hash: WadHash) -> Option<String> {
        self.0.get(&hash).cloned()
    }
    fn entry(&self, _: BinHash) -> Option<String> {
        None
    }
    fn field(&self, _: BinHash) -> Option<String> {
        None
    }
    fn class(&self, _: BinHash) -> Option<String> {
        None
    }
}

fn raised(inventory: &Inventory) -> Diagnostics {
    let mut log = Diagnostics::new();
    inventory.report(&mut log.at(Stage::Load));
    log
}

/// The message of the one diagnostic of a code.
fn message(log: &Diagnostics, code: Code) -> String {
    let mut of_code = log.iter().filter(|d| d.code == code);
    let found = of_code.next().unwrap_or_else(|| panic!("{code} is raised"));
    assert!(of_code.next().is_none(), "{code} is raised once");
    found.message(&Render::plain())
}

/// A skin bin, a bin as text at a skin bin's path, a program, a Maya scene, and outside the
/// WAD the cover image, a macOS sidecar and a readme.
fn mixed_mod() -> Mod {
    let (mut m, wad) = empty_mod();
    file(&mut m, wad, SKIN_BIN, BIN);
    file(&mut m, wad, TEXT_BIN, BIN_TEXT);
    file(&mut m, wad, "tools/fix.exe", &pe());
    file(&mut m, wad, "source/test.ma", MAYA);
    extra(&mut m, "META/image.png", PNG.len() as u64, PNG);
    extra(&mut m, "__MACOSX/WAD/._skin0.bin", 8, APPLE_DOUBLE);
    extra(&mut m, "readme.txt", 16, b"Enjoy the skin!\n");
    m
}

#[test]
fn what_is_no_game_file_is_reported_by_finding() {
    let inventory = Inventory::take(&mixed_mod(), &NoNames);
    let log = raised(&inventory);

    let exe = log
        .iter()
        .find(|d| d.code.name == "inventory.executable")
        .expect("the executable is reported");
    assert_eq!(
        (exe.channel(), exe.severity()),
        (Channel::Input, Severity::Warning)
    );
    assert_eq!(exe.subject, Subject::Run);
    assert_eq!(
        exe.message(&Render::plain()),
        "1 executable, not examined (0.0 MiB; application/vnd.microsoft.portable-executable 1): \
         Test.wad.client/tools/fix.exe"
    );
    let text = message(&log, TextBins::CODE);
    assert!(text.contains("skin1.bin"), "{text}");
    let source = message(&log, Sources::CODE);
    assert!(
        source.contains("test.ma") && !source.contains("skin1.bin"),
        "{source}"
    );
    let mac = message(&log, OsArtifacts::CODE);
    assert!(mac.contains("archive/__MACOSX"), "{mac}");
    // The readme is text and the cover image is the archive's own: neither is a finding.
    for diagnostic in log.iter() {
        let message = diagnostic.message(&Render::plain());
        assert!(
            !message.contains("readme") && !message.contains("image.png"),
            "{message}"
        );
    }
    assert_eq!(log.len(), 4);
}

#[test]
fn every_file_and_entry_is_counted_by_its_kind() {
    let inventory = Inventory::take(&mixed_mod(), &NoNames);
    let kinds: Vec<(Kind, Tally)> = inventory.by_kind.iter().map(|(k, t)| (*k, *t)).collect();
    let tally = |files, bytes| Tally { files, bytes };
    assert_eq!(
        kinds,
        [
            (Kind::PropertyBin, tally(1, BIN.len() as u64)),
            (Kind::RitobinText, tally(1, BIN_TEXT.len() as u64)),
            (Kind::MayaAscii, tally(1, MAYA.len() as u64)),
            (Kind::Text, tally(1, 16)),
            (Kind::Pe, tally(1, 256)),
            (Kind::AppleDouble, tally(1, 8)),
        ]
    );
    // The cover image is the archive's own metadata.
    assert_eq!(inventory.tally(Kind::Png), Tally::default());
    assert_eq!(inventory.tally(Kind::Pe).files, 1);
}

#[test]
fn an_item_names_its_file_its_place_and_how_its_kind_was_known() {
    let m = mixed_mod();
    let inventory = Inventory::take(&m, &NoNames);
    let exe: Vec<&Item> = inventory.found(Finding::Executable).collect();
    let tool = m.files.ids().nth(2).expect("the third file");
    assert_eq!(
        exe,
        [&Item {
            file: Some(tool),
            place: Spelled::new(WAD),
            name: Spelled::new("tools/fix.exe"),
            size: 256,
            kind: Kind::Pe,
            confidence: Confidence::Magic,
            finding: Finding::Executable,
        }]
    );
    let mac: Vec<&Item> = inventory.found(Finding::OsArtifact).collect();
    assert_eq!(mac.len(), 1);
    assert_eq!(mac[0].file, None);
    assert_eq!(mac[0].place.raw(), "archive");
    assert_eq!(mac[0].confidence, Confidence::Name);
    // The files of the WADs come first, then the entries outside them.
    let order: Vec<Finding> = inventory.items.iter().map(|item| item.finding).collect();
    assert_eq!(
        order,
        [
            Finding::BinText,
            Finding::Executable,
            Finding::Source,
            Finding::OsArtifact
        ]
    );
}

#[test]
fn a_mod_of_game_files_and_a_readme_raises_nothing() {
    let (mut m, wad) = empty_mod();
    file(&mut m, wad, SKIN_BIN, BIN);
    // A WAD is an archive and a game file.
    file(&mut m, wad, "data/packed.wad.client", b"RW\x03\x04");
    extra(&mut m, "readme.txt", 16, b"Enjoy the skin!\n");
    extra(&mut m, "META/info.json", 2, b"{}");

    let inventory = Inventory::take(&m, &NoNames);
    assert!(inventory.items.is_empty());
    assert_eq!(inventory.tally(Kind::Wad).files, 1);
    assert_eq!(inventory.tally(Kind::Json), Tally::default());
    assert!(raised(&inventory).is_empty());
}

#[test]
fn a_bin_as_text_is_a_source_away_from_a_bin_path() {
    let (mut m, wad) = empty_mod();
    let working_copy = file(&mut m, wad, "source/skin0.py", BIN_TEXT);
    let json = b"{ \"entries\": {} }";
    let at_bin = file(&mut m, wad, "data/characters/test/test.bin", json);
    extra(&mut m, "skin0.bin", 33, BIN_TEXT);

    let inventory = Inventory::take(&m, &NoNames);
    let found = |finding| -> Vec<Option<FileId>> {
        let items = inventory.found(finding);
        items.map(|item| item.file).collect()
    };
    assert_eq!(found(Finding::BinText), [Some(at_bin)]);
    // An entry outside the WADs is at no path the client reads, whatever its name.
    assert_eq!(found(Finding::Source), [Some(working_copy), None]);
    assert_eq!(inventory.tally(Kind::RitobinJson).files, 1);
}

#[test]
fn a_bin_as_text_at_a_game_path_is_found_where_no_name_says_bin() {
    let (mut m, wad) = empty_mod();
    let path = "data/characters/test/skins/skin2.bin";
    let overriding = hashed(&mut m, wad, path, BIN_TEXT, FileUse::Override);
    let former = hashed(
        &mut m,
        wad,
        "data/former.bin",
        BIN_TEXT,
        FileUse::FormerPath,
    );
    let loose = hashed(&mut m, wad, "data/loose.bin", BIN_TEXT, FileUse::Referenced);

    let inventory = Inventory::take(&m, &NoNames);
    let files = |finding| -> Vec<Option<FileId>> {
        let items = inventory.found(finding);
        items.map(|item| item.file).collect()
    };
    assert_eq!(files(Finding::BinText), [Some(overriding), Some(former)]);
    assert_eq!(files(Finding::Source), [Some(loose)]);
    // No table spells the path, so the item is named by its hash.
    let named = inventory.found(Finding::BinText).next().unwrap();
    assert_eq!(named.name.raw(), format!("{:016x}", path_hash(path).0));

    // A table that spells the path says it is a bin's.
    let named = Inventory::take(&m, &Table::of(&["data/loose.bin"]));
    let items: Vec<&Item> = named.found(Finding::BinText).collect();
    assert_eq!(items.len(), 3);
    assert_eq!(items[2].name.raw(), "data/loose.bin");
}

#[test]
fn a_chunk_named_by_its_hash_is_told_by_the_name_a_table_spells() {
    let toc = "data/final/champions/test.wad.subchunktoc";
    let (mut m, wad) = empty_mod();
    hashed(&mut m, wad, toc, NOISE, FileUse::Unused);

    let unnamed = Inventory::take(&m, &NoNames);
    assert_eq!(unnamed.tally(Kind::Unknown).files, 1);
    assert_eq!(unnamed.found(Finding::Unknown).count(), 1);

    let named = Inventory::take(&m, &Table::of(&[toc]));
    assert_eq!(named.tally(Kind::SubchunkToc).files, 1);
    assert!(named.items.is_empty());
}

#[test]
fn an_archive_inside_the_mod_and_bytes_of_no_kind_are_notes() {
    let (mut m, wad) = empty_mod();
    file(&mut m, wad, "extras/sources.zip", ZIP);
    file(&mut m, wad, "assets/mystery.dat", NOISE);
    extra(&mut m, "backup.zip", 4096, ZIP);

    let inventory = Inventory::take(&m, &NoNames);
    let found: Vec<(&str, &str, Kind)> = inventory
        .found(Finding::NestedArchive)
        .map(|item| (item.place.raw(), item.name.raw(), item.kind))
        .collect();
    assert_eq!(
        found,
        [
            (WAD, "extras/sources.zip", Kind::Zip),
            ("archive", "backup.zip", Kind::Zip),
        ]
    );
    // A WAD is an archive too, and a game file: it is no finding.
    assert_eq!(Finding::of(Kind::Zip, false), Some(Finding::NestedArchive));
    assert_eq!(Finding::of(Kind::Wad, false), None);

    let log = raised(&inventory);
    let nested = log.of::<NestedArchives>().next().expect("the archives");
    assert_eq!(nested.items.len(), 2);
    assert!(
        nested
            .items
            .iter()
            .all(|item| item.finding == Finding::NestedArchive)
    );
    assert_eq!(NestedArchives::CODE.name, "inventory.nested-archive");
    assert_eq!(
        message(&log, NestedArchives::CODE),
        "2 archives inside the mod, not opened (0.0 MiB; application/zip 2): \
         Test.wad.client/extras/sources.zip, archive/backup.zip"
    );
    assert_eq!(
        message(&log, UnknownFiles::CODE),
        "1 file of no known kind (0.0 MiB; application/octet-stream 1): \
         Test.wad.client/assets/mystery.dat"
    );
    assert!(log.iter().all(|d| d.severity() == Severity::Note));
}

#[test]
fn a_file_a_fixup_made_is_not_counted_and_a_bin_taken_from_the_game_is() {
    let (mut m, wad) = empty_mod();
    let from = file(&mut m, wad, "tools/fix.exe", &pe());
    let mut made = node(
        wad,
        path_hash("tools/moved.exe"),
        Some("tools/moved.exe"),
        &pe(),
    );
    made.source = FileSource::Made {
        from,
        by: MadeBy::MovedAsset,
    };
    m.push_file(made).expect("the path is free");
    let mut taken = node(wad, path_hash(SKIN_BIN), Some(SKIN_BIN), BIN);
    taken.source = FileSource::Game;
    m.push_file(taken).expect("the path is free");

    let inventory = Inventory::take(&m, &NoNames);
    assert_eq!(inventory.tally(Kind::Pe).files, 1);
    assert_eq!(inventory.found(Finding::Executable).count(), 1);
    assert_eq!(inventory.tally(Kind::PropertyBin).files, 1);
}

#[test]
fn the_findings_are_raised_in_the_order_of_their_codes() {
    let (mut m, wad) = empty_mod();
    file(&mut m, wad, "assets/mystery.dat", NOISE);
    file(&mut m, wad, "source/test.ma", MAYA);
    file(&mut m, wad, "extras/sources.zip", ZIP);
    file(&mut m, wad, TEXT_BIN, BIN_TEXT);
    extra(&mut m, ".DS_Store", 8, b"\0\0\0\x01Bud1");
    extra(&mut m, "setup.exe", 256, &pe());

    let log = raised(&Inventory::take(&m, &NoNames));
    let codes: Vec<Code> = log.iter().map(|d| d.code).collect();
    assert_eq!(codes, CODES);
    assert_eq!(
        CODES.iter().map(|code| code.name).collect::<Vec<_>>(),
        [
            "inventory.executable",
            "inventory.bin-text",
            "inventory.nested-archive",
            "inventory.source",
            "inventory.os-artifact",
            "inventory.unknown",
        ]
    );
    let severities: Vec<Severity> = CODES.iter().map(|code| code.severity).collect();
    assert_eq!(
        severities,
        [
            Severity::Warning,
            Severity::Warning,
            Severity::Note,
            Severity::Note,
            Severity::Note,
            Severity::Note
        ]
    );
    assert!(CODES.iter().all(|code| code.channel == Channel::Input));
    assert!(log.iter().all(|d| d.subject == Subject::Run));
    // Every finding but the files of no known kind rests on the record of what mods carry.
    let records: Vec<Option<Record>> = log.iter().map(|d| d.record).collect();
    let carried = Some(Record::mod_artifact(16));
    assert_eq!(records, [carried, carried, carried, carried, carried, None]);
    assert_eq!(log.of::<Executables>().count(), 1);
}

#[test]
fn a_name_the_mod_spells_is_cleaned_in_the_message_and_kept_in_the_data() {
    let (mut m, _) = empty_mod();
    extra(&mut m, "run\u{1b}[31m.exe", 256, &pe());

    let log = raised(&Inventory::take(&m, &NoNames));
    let text = message(&log, Executables::CODE);
    assert!(text.ends_with("archive/run\\u{1b}[31m.exe"), "{text:?}");
    assert!(!text.contains('\u{1b}'));
    let data = log.iter().next().unwrap().data();
    assert_eq!(data["items"][0]["name"], "run\u{1b}[31m.exe");
}

#[test]
fn a_long_list_shows_its_first_items_and_counts_the_rest() {
    let (mut m, _) = empty_mod();
    for i in 0..11 {
        extra(&mut m, &format!("tool{i}.exe"), 1 << 20, &pe());
    }

    let log = raised(&Inventory::take(&m, &NoNames));
    let text = message(&log, Executables::CODE);
    assert!(
        text.starts_with("11 executables, not examined (11.0 MiB; "),
        "{text}"
    );
    assert!(text.ends_with("archive/tool7.exe and 3 more"), "{text}");
    assert_eq!(log.of::<Executables>().next().unwrap().items.len(), 11);
}

#[test]
fn sizes_an_archive_declares_do_not_overflow_the_count() {
    let (mut m, _) = empty_mod();
    extra(&mut m, "a.exe", u64::MAX, &pe());
    extra(&mut m, "b.exe", u64::MAX, &pe());

    let inventory = Inventory::take(&m, &NoNames);
    assert_eq!(
        inventory.tally(Kind::Pe),
        Tally {
            files: 2,
            bytes: u64::MAX
        }
    );
    let text = message(&raised(&inventory), Executables::CODE);
    assert!(text.starts_with("2 executables"), "{text}");
}

#[test]
fn an_inventory_serialises_by_kind_then_item() {
    let (mut m, wad) = empty_mod();
    file(&mut m, wad, SKIN_BIN, BIN);
    extra(&mut m, "setup.exe", 256, &pe());

    let inventory = Inventory::take(&m, &NoNames);
    assert_eq!(
        serde_json::to_value(&inventory).unwrap(),
        serde_json::json!({
            "by_kind": {
                "property-bin": { "files": 1, "bytes": 16 },
                "pe": { "files": 1, "bytes": 256 },
            },
            "items": [{
                "file": null,
                "place": "archive",
                "name": "setup.exe",
                "size": 256,
                "kind": "pe",
                "confidence": "magic",
                "finding": "executable",
            }],
        })
    );
    // Two inventories of one mod are equal.
    assert_eq!(inventory, Inventory::take(&m, &NoNames));
}
