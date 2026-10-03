//! Dating: the newest first build among the mod's game files and objects bounds the build it
//! was made at from below. Small pieces, the game's own bins and bytes the history does not hold
//! date nothing.

use std::sync::Arc;

use ltk_meta::BinObject;
use ltk_meta::property::values;
use reforge::archive::file_kind;
use reforge::history::{History, NoHistory, Position};
use reforge::load::dating::{
    CODES, Dated, Dating, DatingEvidence, MIN_FILE, MIN_PROPERTIES, Piece,
};
use reforge::model::entity::{
    BinNode, FileId, FileNode, FileSource, FileUse, MadeBy, ModMeta, ModWad, ObjectId, ObjectNode,
    WadId, WadLayout, WadRole,
};
use reforge::model::{
    ChunkPath, ContentId, Mod, NormKey, ObjectKey, Spelled, WadName, name_hash, path_hash,
};
use reforge_testkit::MemoryHistory;

/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";
const SKIN_BIN: &str = "data/characters/test/skins/skin0.bin";
const EMITTER: &str = "Characters/Test/Skins/Skin0/Particles/Test_Emitter";

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

/// Builds 50, 150 and 300, at positions 0, 1 and 2.
fn builds() -> MemoryHistory {
    MemoryHistory::new(&["1.0.50", "1.0.150", "16.19.300"])
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
        name: WadName::parse("Test.wad.client").expect("a WAD name"),
        role: WadRole::Champion {
            champion: "test".into(),
        },
        layout: WadLayout::Loose,
        files: Vec::new(),
        packed_locale: None,
    });
    (m, wad)
}

fn file(m: &mut Mod, wad: WadId, path: &str, bytes: &[u8], source: FileSource) -> FileId {
    m.push_file(FileNode {
        wad,
        hash: path_hash(path),
        path: Some(ChunkPath::parse(path).expect("a chunk path")),
        size: bytes.len() as u64,
        content: ContentId::of(bytes),
        kind: file_kind(bytes),
        bytes: Arc::from(bytes),
        used: FileUse::Referenced,
        source,
    })
    .expect("the path is free")
}

/// The digest of an object's bytes in a test: any bytes that differ by entry.
fn raw_of(entry: &str) -> ContentId {
    ContentId::of(entry.as_bytes())
}

/// Adds an object with this many properties to the bin of a file, making the bin when the file
/// has none.
fn object(m: &mut Mod, bin_file: FileId, entry: &str, properties: usize) -> ObjectId {
    let bin = m.bin_of(bin_file).unwrap_or_else(|| {
        m.push_bin(BinNode {
            file: bin_file,
            links: Vec::new(),
            objects: Vec::new(),
            error: None,
        })
    });
    let mut value = BinObject::builder(name_hash(entry), name_hash("TestEmitter"));
    for i in 0..properties {
        value = value.property(name_hash(&format!("field{i}")), values::F32::new(1.0));
    }
    let id = m.objects.push(ObjectNode {
        bin,
        entry: name_hash(entry),
        class: name_hash("TestEmitter"),
        raw: raw_of(entry),
        body: ContentId::of(b"body"),
        value: value.build(),
    });
    m.bins[bin].objects.push(id);
    id
}

/// A history in which the game held the object of an entry at these builds.
fn with_object(
    history: MemoryHistory,
    entry: &str,
    builds: std::ops::RangeInclusive<u16>,
) -> MemoryHistory {
    history.with_object(
        ObjectKey::from_sha256(&raw_of(entry).0),
        NormKey(1),
        (entry, "TestEmitter", SKIN_BIN),
        builds,
    )
}

#[test]
fn the_bound_is_the_build_the_newest_game_bytes_first_shipped_at() {
    let (mut m, wad) = empty_mod();
    let dated = file(&mut m, wad, DATED, &dated_bytes(), FileSource::Archive);
    let history = builds().with_content(DATED, &dated_bytes(), 1..=2);

    let dating = Dating::find(&m, &history).unwrap();
    assert_eq!(dating.build(), Some(150));
    assert_eq!(dating.position(), Some(Position(1)));
    let bound = dating.not_before.as_ref().expect("a bound");
    assert_eq!(bound.build.as_str(), "1.0.150");
    assert_eq!(dating.evidence, Some(Piece::File(dated)));
    assert_eq!((dating.dated, dating.at_bound), (1, 1));
}

#[test]
fn bytes_from_the_first_build_date_the_mod_there() {
    let (mut m, wad) = empty_mod();
    file(&mut m, wad, DATED, &dated_bytes(), FileSource::Archive);
    let history = builds().with_content(DATED, &dated_bytes(), 0..=2);

    assert_eq!(Dating::find(&m, &history).unwrap().build(), Some(50));
}

#[test]
fn the_newest_first_build_wins_over_older_pieces() {
    let (mut m, wad) = empty_mod();
    let old = vec![1; 6000];
    file(&mut m, wad, "assets/old.tex", &old, FileSource::Archive);
    let skin = file(&mut m, wad, SKIN_BIN, b"PROP", FileSource::Archive);
    let emitter = object(&mut m, skin, EMITTER, MIN_PROPERTIES);
    let other = object(&mut m, skin, "Other/Emitter", MIN_PROPERTIES);
    let history = builds().with_content("assets/old.tex", &old, 0..=1);
    let history = with_object(history, EMITTER, 2..=2);
    let history = with_object(history, "Other/Emitter", 2..=2);

    let evidence = DatingEvidence::gather(&m, &history).unwrap();
    assert_eq!(evidence.pieces.len(), 3);
    assert_eq!(
        evidence.pieces[1],
        Dated {
            piece: Piece::Object(emitter),
            first: Position(2)
        }
    );
    assert_eq!(evidence.newest(), Some(Position(2)));

    let dating = Dating::judge(&evidence, &history);
    assert_eq!(dating.build(), Some(300));
    // The first piece at the bound is the evidence named. Both objects are counted.
    assert_eq!(dating.evidence, Some(Piece::Object(emitter)));
    assert_ne!(dating.evidence, Some(Piece::Object(other)));
    assert_eq!((dating.dated, dating.at_bound), (3, 2));
}

#[test]
fn a_small_file_and_a_near_empty_object_date_nothing() {
    let (mut m, wad) = empty_mod();
    let small = vec![3; MIN_FILE as usize - 1];
    file(&mut m, wad, "assets/small.tex", &small, FileSource::Archive);
    let skin = file(&mut m, wad, SKIN_BIN, b"PROP", FileSource::Archive);
    object(&mut m, skin, EMITTER, MIN_PROPERTIES - 1);
    let history = builds().with_content("assets/small.tex", &small, 2..=2);
    let history = with_object(history, EMITTER, 2..=2);

    let dating = Dating::find(&m, &history).unwrap();
    assert_eq!(dating, Dating::default());
    assert_eq!(dating.build(), None);
}

#[test]
fn a_file_of_the_threshold_size_and_an_object_of_four_properties_count() {
    let (mut m, wad) = empty_mod();
    let edge = vec![3; MIN_FILE as usize];
    file(&mut m, wad, "assets/edge.tex", &edge, FileSource::Archive);
    let skin = file(&mut m, wad, SKIN_BIN, b"PROP", FileSource::Archive);
    object(&mut m, skin, EMITTER, MIN_PROPERTIES);
    let history = builds().with_content("assets/edge.tex", &edge, 1..=2);
    let history = with_object(history, EMITTER, 1..=2);

    let dating = Dating::find(&m, &history).unwrap();
    assert_eq!((dating.dated, dating.at_bound), (2, 2));
    assert_eq!(dating.build(), Some(150));
}

#[test]
fn the_games_bins_taken_for_a_binless_mod_date_nothing() {
    let (mut m, wad) = empty_mod();
    let bin_bytes = vec![9; 8000];
    let taken = file(&mut m, wad, SKIN_BIN, &bin_bytes, FileSource::Game);
    object(&mut m, taken, EMITTER, MIN_PROPERTIES);
    let history = builds().with_content(SKIN_BIN, &bin_bytes, 2..=2);
    let history = with_object(history, EMITTER, 2..=2);

    assert_eq!(Dating::find(&m, &history).unwrap(), Dating::default());
}

#[test]
fn own_bytes_date_nothing() {
    let (mut m, wad) = empty_mod();
    file(
        &mut m,
        wad,
        "assets/mod/own.tex",
        &[5; 9000],
        FileSource::Archive,
    );
    let skin = file(&mut m, wad, SKIN_BIN, b"PROP", FileSource::Archive);
    object(&mut m, skin, EMITTER, MIN_PROPERTIES);
    let history = builds().with_content(DATED, &dated_bytes(), 1..=2);

    let dating = Dating::find(&m, &history).unwrap();
    assert_eq!(dating.not_before, None);
    assert_eq!(dating.dated, 0);
}

#[test]
fn without_a_history_nothing_is_dated() {
    let (mut m, wad) = empty_mod();
    file(&mut m, wad, DATED, &dated_bytes(), FileSource::Archive);

    assert_eq!(Dating::find(&m, &NoHistory).unwrap(), Dating::default());
}

#[test]
fn a_file_a_fixup_made_dates_the_mod_like_any_file() {
    let (mut m, wad) = empty_mod();
    let from = file(
        &mut m,
        wad,
        "assets/old/place.tex",
        &[1; 10],
        FileSource::Archive,
    );
    let made = FileSource::Made {
        from,
        by: MadeBy::MovedAsset,
    };
    let moved = file(&mut m, wad, DATED, &dated_bytes(), made);
    let history = builds().with_content(DATED, &dated_bytes(), 1..=2);

    let dating = Dating::find(&m, &history).unwrap();
    assert_eq!(dating.evidence, Some(Piece::File(moved)));
    assert_eq!(dating.build(), Some(150));
}

#[test]
fn the_bound_follows_the_order_the_builds_arrived_in() {
    // A hotfix of 16.18 lands after the first build of 16.19: it is the later position.
    let history = MemoryHistory::new(&["16.18.200", "16.19.300", "16.18.250"]);
    let (mut m, wad) = empty_mod();
    let (a, b) = (vec![1; 5000], vec![2; 5000]);
    file(&mut m, wad, "assets/a.tex", &a, FileSource::Archive);
    file(&mut m, wad, "assets/b.tex", &b, FileSource::Archive);
    let history = history
        .with_content("assets/a.tex", &a, 1..=2)
        .with_content("assets/b.tex", &b, 2..=2);

    let dating = Dating::find(&m, &history).unwrap();
    assert_eq!(dating.position(), Some(Position(2)));
    assert_eq!(dating.build(), Some(250));
}

#[test]
fn a_second_manifest_of_a_version_dates_by_the_versions_number() {
    let history = MemoryHistory::new(&["14.8.5756478", "14.8.5756478__2"]);
    let (mut m, wad) = empty_mod();
    file(&mut m, wad, DATED, &dated_bytes(), FileSource::Archive);
    let history = history.with_content(DATED, &dated_bytes(), 1..=1);

    let dating = Dating::find(&m, &history).unwrap();
    assert_eq!(dating.build(), Some(5_756_478));
    assert_eq!(dating.not_before.unwrap().build.as_str(), "14.8.5756478__2");
}

#[test]
fn a_position_the_history_lists_no_build_at_gives_no_bound_and_keeps_the_evidence() {
    let (mut m, wad) = empty_mod();
    let dated = file(&mut m, wad, DATED, &dated_bytes(), FileSource::Archive);
    let history = builds().with_content(DATED, &dated_bytes(), 7..=7);
    assert!(history.build(Position(7)).is_none());

    let dating = Dating::find(&m, &history).unwrap();
    assert_eq!(dating.not_before, None);
    assert_eq!(dating.evidence, Some(Piece::File(dated)));
    assert_eq!((dating.dated, dating.at_bound), (1, 1));
}

#[test]
fn a_dating_serialises_with_its_pieces_by_id() {
    let (mut m, wad) = empty_mod();
    file(&mut m, wad, DATED, &dated_bytes(), FileSource::Archive);
    let history = builds().with_content(DATED, &dated_bytes(), 1..=2);

    let dating = Dating::find(&m, &history).unwrap();
    assert_eq!(
        serde_json::to_value(&dating).unwrap(),
        serde_json::json!({
            "not_before": { "at": 1, "build": "1.0.150" },
            "evidence": { "piece": "file", "id": "%f0" },
            "dated": 1,
            "at_bound": 1,
        })
    );
}

#[test]
fn dating_raises_no_diagnostic() {
    assert!(CODES.is_empty());
}
