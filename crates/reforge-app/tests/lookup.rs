//! The questions the History page asks: each the library's answer over a history in memory,
//! its hashes spelled through text tables made for the test.

use std::io::Cursor;

use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_meta::BinObject;
use ltk_meta::property::values;
use reforge::game::Names;
use reforge::history::{BinEntry, NoHistory};
use reforge::model::{BinHash, NormKey, ObjectKey, WadHash, name_hash, path_hash};
use reforge_app::error::AppError;
use reforge_app::lookup::{BuildView, EntryQuery, Lookup, MAX_NAMED_BY, PathQuery};
use reforge_testkit::history::{MemoryHistory, file_key};
use tempfile::TempDir;

const SKIN: &str = "data/characters/lux/skins/skin0.bin";
const LUX: &str = "data/characters/lux/lux.bin";
const SKIN_ENTRY: &str = "Characters/Lux/Skins/Skin0";
const RESOLVER: &str = "Characters/Lux/Skins/Skin0/Resources";
const SKIN_CLASS: &str = "SkinCharacterDataProperties";
const CHAMPION_WAD: &str = "DATA/FINAL/Champions/Lux.wad.client";
const LOCALE_WAD: &str = "DATA/FINAL/Champions/Lux.en_US.wad.client";
const SKIN_NAME_FIELD: &str = "championSkinName";

fn history() -> MemoryHistory {
    MemoryHistory::new(&["14.1.1", "14.2.1", "14.1.2", "14.3.1"])
}

/// The skin object naming the skin `skin_name`, as the index stores it: from its entry hash
/// on, with no size before it.
fn stored_skin(skin_name: &str) -> Vec<u8> {
    let object = BinObject::builder(name_hash(SKIN_ENTRY), name_hash(SKIN_CLASS))
        .property(name_hash(SKIN_NAME_FIELD), values::String::from(skin_name))
        .build();
    let mut out = Cursor::new(Vec::new());
    object.to_writer(&mut out).unwrap();
    out.into_inner().split_off(4)
}

/// Text tables that spell the test's paths, entries and classes.
fn names() -> (TempDir, Names) {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let lines = |names: &[&str], hash: &dyn Fn(&str) -> u64| -> String {
        names
            .iter()
            .map(|name| format!("{:x} {name}\n", hash(name)))
            .collect()
    };
    let paths = ["data/characters/lux/skins/skin1.bin", SKIN, LUX];
    fs::write(
        root.join("hashes.game.txt"),
        lines(&paths, &|path| path_hash(path).0),
    )
    .unwrap();
    let entry = |name: &str| u64::from(name_hash(name).0);
    fs::write(
        root.join("hashes.binentries.txt"),
        lines(&[SKIN_ENTRY, RESOLVER], &entry),
    )
    .unwrap();
    fs::write(
        root.join("hashes.bintypes.txt"),
        lines(&[SKIN_CLASS, "ResourceResolver"], &entry),
    )
    .unwrap();
    fs::write(
        root.join("hashes.binfields.txt"),
        lines(&[SKIN_NAME_FIELD], &entry),
    )
    .unwrap();
    let names = Names::builder().text_tables(&root).build().unwrap();
    (dir, names)
}

fn entry(name: &str, class: &str) -> BinEntry {
    BinEntry {
        entry: name_hash(name),
        class: name_hash(class),
        object: ObjectKey([0; 16]),
        norm: NormKey(0),
    }
}

#[test]
fn a_query_of_sixteen_hex_digits_is_a_path_hash_and_anything_else_a_path() {
    let hash = WadHash(0x0123_4567_89ab_cdef);
    assert_eq!(PathQuery::parse("0123456789abcdef"), PathQuery::Hash(hash));
    assert_eq!(
        PathQuery::parse("0x0123456789ABCDEF"),
        PathQuery::Hash(hash)
    );
    assert_eq!(
        PathQuery::parse(" 0X0123456789abcdef "),
        PathQuery::Hash(hash)
    );
    assert_eq!(
        PathQuery::parse("0123456789abcde"),
        PathQuery::Path("0123456789abcde".into())
    );
    assert_eq!(
        PathQuery::parse("0x0123456789abcdeg"),
        PathQuery::Path("0x0123456789abcdeg".into())
    );
    let path = PathQuery::parse(" DATA/Characters/Lux/Skins/Skin0.bin ");
    assert_eq!(path, PathQuery::Path(SKIN.into()));
    assert_eq!(path.hash(), path_hash(SKIN));
}

#[test]
fn no_history_is_refused_as_no_index() {
    let names = Names::default();
    assert!(matches!(
        Lookup::new(&NoHistory, &names),
        Err(AppError::NoIndex)
    ));
}

#[test]
fn the_builds_come_in_position_order_with_the_head() {
    let history = history();
    let names = Names::default();
    let builds = Lookup::new(&history, &names).unwrap().builds();

    assert_eq!(builds.head, 3);
    let versions: Vec<&str> = builds.builds.iter().map(|b| b.version.as_str()).collect();
    assert_eq!(versions, ["14.1.1", "14.2.1", "14.1.2", "14.3.1"]);
    assert_eq!(
        builds.builds[2],
        BuildView {
            position: 2,
            version: "14.1.2".into(),
            commit: "c2".into(),
        }
    );
}

#[test]
fn a_bin_is_answered_with_its_wads_its_namers_its_links_and_its_entries_spelled() {
    let (_tables, names) = names();
    let history = history()
        .with_bin(
            (CHAMPION_WAD, SKIN),
            b"skin0 at first",
            &[LUX, "data/unknown.bin"],
            vec![
                entry(SKIN_ENTRY, SKIN_CLASS),
                entry(RESOLVER, "ResourceResolver"),
                entry("Unspelled", "Unspelled"),
            ],
            0..=3,
        )
        .with_content_in(LOCALE_WAD, SKIN, b"skin0 of a locale", 1..=1)
        .with_naming(SKIN, (SKIN_ENTRY, LUX), 1..=3);
    let lookup = Lookup::new(&history, &names).unwrap();

    let asked = format!("0x{:016x}", path_hash(SKIN).0);
    let found = lookup.lookup(&asked, Some(1)).unwrap();

    assert_eq!(found.path, SKIN);
    assert_eq!(found.hash, format!("{:016x}", path_hash(SKIN).0));
    let life = found.life.as_ref().unwrap();
    assert_eq!(
        (life.first.version.as_str(), life.last.version.as_str()),
        ("14.1.1", "14.3.1")
    );
    assert_eq!(life.builds, 4);
    assert_eq!(found.at.version, "14.2.1");
    let wads: Vec<&str> = found.placed.iter().map(|p| p.wad.as_str()).collect();
    assert_eq!(wads, [LOCALE_WAD, CHAMPION_WAD]);
    assert_eq!(
        found.placed[1].content,
        file_key(b"skin0 at first").to_string()
    );
    assert_eq!(found.named_by.len(), 1);
    assert_eq!(found.named_by[0].bin, LUX);
    assert_eq!(found.named_by[0].entry, SKIN_ENTRY);
    assert_eq!(
        found.named_by[0].entry_hash,
        format!("{:08x}", name_hash(SKIN_ENTRY).0)
    );
    assert!(!found.named_by_capped);

    let bin = found.bin.unwrap();
    assert_eq!(bin.wad, CHAMPION_WAD);
    let links: Vec<&str> = bin.links.iter().map(|l| l.path.as_str()).collect();
    let unknown = format!("{:016x}", path_hash("data/unknown.bin").0);
    assert_eq!(links, [LUX, unknown.as_str()]);
    let entries: Vec<(&str, &str)> = bin
        .entries
        .iter()
        .map(|e| (e.entry.as_str(), e.class.as_str()))
        .collect();
    let unspelled = format!("{:08x}", name_hash("Unspelled").0);
    assert_eq!(
        entries,
        [
            (unspelled.as_str(), unspelled.as_str()),
            (SKIN_ENTRY, SKIN_CLASS),
            (RESOLVER, "ResourceResolver"),
        ]
    );
}

#[test]
fn a_path_the_game_never_held_keeps_its_text_and_is_answered_at_the_head() {
    let (_tables, names) = names();
    let history = history();
    let lookup = Lookup::new(&history, &names).unwrap();

    let found = lookup
        .lookup("DATA/Characters/Nobody/Nobody.bin", None)
        .unwrap();

    assert_eq!(found.path, "data/characters/nobody/nobody.bin");
    assert_eq!(found.life, None);
    assert_eq!(found.kind, None);
    assert_eq!(found.at.position, 3);
    assert!(found.placed.is_empty() && found.named_by.is_empty() && found.bin.is_none());
}

#[test]
fn a_hash_no_table_spells_is_answered_as_its_hex() {
    let names = Names::default();
    let history = history().with_path("data/secret.bin", 0..=1);
    let lookup = Lookup::new(&history, &names).unwrap();
    let hash = format!("{:016x}", path_hash("data/secret.bin").0);

    let found = lookup.lookup(&hash, Some(0)).unwrap();

    assert_eq!(found.path, hash);
    assert_eq!(found.life.unwrap().builds, 2);
}

#[test]
fn the_objects_naming_a_path_are_capped_and_said_to_be() {
    let names = Names::default();
    let mut history = history();
    for n in 0..=MAX_NAMED_BY {
        history = history.with_naming(SKIN, (&format!("Entry{n}"), LUX), 0..=0);
    }
    let lookup = Lookup::new(&history, &names).unwrap();

    let found = lookup.lookup(SKIN, None).unwrap();

    assert_eq!(found.named_by.len(), MAX_NAMED_BY);
    assert!(found.named_by_capped);
}

#[test]
fn a_position_no_build_has_is_refused() {
    let names = Names::default();
    let history = history();
    let lookup = Lookup::new(&history, &names).unwrap();

    assert!(matches!(
        lookup.lookup(SKIN, Some(4)),
        Err(AppError::Internal { .. })
    ));
}

#[test]
fn paths_are_suggested_from_three_characters_on_lowercased_and_in_order() {
    let (_tables, names) = names();
    let history = history();
    let lookup = Lookup::new(&history, &names).unwrap();

    assert!(lookup.suggest("da").is_empty());
    assert_eq!(
        lookup.suggest("DATA/Characters/Lux/Skins/"),
        [SKIN, "data/characters/lux/skins/skin1.bin"]
    );
    assert_eq!(lookup.suggest("dat").len(), 3);
}

#[test]
fn a_query_of_eight_hex_digits_is_an_entry_hash_and_anything_else_a_name() {
    assert_eq!(
        EntryQuery::parse("0x0000ABCD"),
        EntryQuery::Hash(BinHash(0xabcd))
    );
    assert_eq!(
        EntryQuery::parse("deadbeef"),
        EntryQuery::Hash(BinHash(0xdead_beef))
    );
    let name = EntryQuery::parse(" Characters/Lux/Skins/Skin0 ");
    assert_eq!(name, EntryQuery::Name(SKIN_ENTRY.into()));
    assert_eq!(name.hash(), name_hash(SKIN_ENTRY));
}

#[test]
fn an_object_at_a_build_is_its_text_with_names_spelled() {
    let (_tables, names) = names();
    let history = history()
        .with_object_bytes(
            CHAMPION_WAD,
            (SKIN_ENTRY, SKIN_CLASS, SKIN),
            stored_skin("Lux"),
            0..=1,
        )
        .with_object_bytes(
            CHAMPION_WAD,
            (SKIN_ENTRY, SKIN_CLASS, SKIN),
            stored_skin("Luxanna"),
            2..=3,
        );
    let lookup = Lookup::new(&history, &names).unwrap();

    let first = lookup.object(SKIN, SKIN_ENTRY, 1).unwrap().unwrap();
    assert_eq!(first.class, SKIN_CLASS);
    assert_eq!(first.wad, CHAMPION_WAD);
    assert_eq!(first.life.last.version, "14.2.1");
    assert!(
        first
            .text
            .starts_with(&format!("{SKIN_ENTRY} = {SKIN_CLASS} {{"))
    );
    assert!(first.text.contains(SKIN_NAME_FIELD) && first.text.contains("\"Lux\""));

    let bin = format!("{:016x}", path_hash(SKIN).0);
    let entry = format!("0x{:08x}", name_hash(SKIN_ENTRY).0);
    let later = lookup.object(&bin, &entry, 3).unwrap().unwrap();
    assert!(later.text.contains("\"Luxanna\""));
}

#[test]
fn a_build_that_holds_no_such_object_answers_none() {
    let names = Names::default();
    let history = history().with_object_bytes(
        CHAMPION_WAD,
        (SKIN_ENTRY, SKIN_CLASS, SKIN),
        stored_skin("Lux"),
        1..=1,
    );
    let lookup = Lookup::new(&history, &names).unwrap();

    assert_eq!(lookup.object(SKIN, SKIN_ENTRY, 0).unwrap(), None);
    assert_eq!(lookup.object(SKIN, RESOLVER, 1).unwrap(), None);
}

#[test]
fn bytes_that_are_no_object_are_refused_with_the_object_named() {
    let (_tables, names) = names();
    let mut cut = stored_skin("Lux");
    cut.truncate(cut.len() - 2);
    let history =
        history().with_object_bytes(CHAMPION_WAD, (SKIN_ENTRY, SKIN_CLASS, SKIN), cut, 0..=3);
    let lookup = Lookup::new(&history, &names).unwrap();

    match lookup.object(SKIN, SKIN_ENTRY, 0) {
        Err(AppError::HistoryObjectUnreadable { bin, entry, .. }) => {
            assert_eq!((bin.as_str(), entry.as_str()), (SKIN, SKIN_ENTRY));
        }
        other => panic!("the cut object answered {other:?}"),
    }
}
