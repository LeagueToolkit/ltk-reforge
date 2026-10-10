//! File provenance: whose bytes each file the client uses holds. The install is asked first, at
//! the file's path and at the chunks of its size in the champion's WAD, then the history.

use std::collections::BTreeMap;

use reforge::archive::file_kind;
use reforge::diag::{Channel, Diag, Diagnostics, Render, Severity, Stage, Subject};
use reforge::game::{Build, Game, GameError, GameView, SizeIndex};
use reforge::history::{History, HistoryError, Life, NoHistory};
use reforge::load::Fault;
use reforge::load::provenance::{
    Against, AtPath, BytesOrigin, CODES, FileProvenance, MAX_SAME_SIZE_READS, OlderGameFiles,
    SameSizeCapped, SkinFolder, is_vfx_path,
};
use reforge::model::entity::{
    FileId, FileNode, FileSource, FileUse, ModMeta, ModWad, WadId, WadLayout, WadRole,
};
use reforge::model::{
    BinHash, ByteView, ChunkPath, ContentId, Mod, NoNames, Slot, Spell, Spelled, WadHash, WadName,
    path_hash,
};
use reforge_testkit::{BrokenHistory, MemoryGame, MemoryHistory};

const WAD: &str = "Test.wad.client";

/// A game of one champion WAD, with its chunks by size and the names of its paths.
struct World {
    game: MemoryGame,
    sizes: SizeIndex,
    names: Table,
}

/// Spells the paths a test names.
#[derive(Default)]
struct Table(BTreeMap<WadHash, String>);

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

fn wad_name(name: &str) -> WadName {
    WadName::parse(name).expect("a WAD name")
}

impl World {
    fn new() -> Self {
        Self {
            game: MemoryGame::new(WAD, "16.19.300"),
            sizes: SizeIndex::new(),
            names: Table::default(),
        }
    }

    /// Puts a chunk in the champion's WAD, under a path the tables spell.
    fn ship(&mut self, path: &str, bytes: &[u8]) {
        self.ship_unnamed(path, bytes);
        self.name(path);
    }

    /// Puts a chunk in the champion's WAD, under a path no table spells.
    fn ship_unnamed(&mut self, path: &str, bytes: &[u8]) {
        self.game.insert(path, bytes.to_vec());
        self.sizes
            .add(&wad_name(WAD), path_hash(path), bytes.len() as u64);
    }

    /// Puts a chunk in another WAD of the install: the size index of the champion's WAD does
    /// not list it.
    fn ship_elsewhere(&mut self, path: &str, bytes: &[u8]) {
        self.game
            .insert_in(path, bytes.to_vec(), &["Map11.wad.client"]);
        self.name(path);
    }

    fn name(&mut self, path: &str) {
        self.names.0.insert(path_hash(path), path.to_string());
    }

    /// The game as a stage asks it.
    fn view(&self) -> GameView {
        GameView::new(Box::new(self.game.clone())).sizes(self.sizes.clone())
    }

    fn find(&self, m: &Mod, history: &dyn History) -> FileProvenance {
        let game = self.view();
        let against = Against {
            game: &game,
            history,
            names: &self.names,
        };
        FileProvenance::find(m, &against).expect("the history answers")
    }
}

fn empty_mod() -> Mod {
    Mod::new(ModMeta {
        file_name: Spelled::new("synthetic.fantome"),
        sha256: ContentId::of(b"synthetic"),
        mod_id: "000000000000".into(),
        name: None,
        author: None,
        version: None,
        description: None,
    })
}

fn wad(m: &mut Mod, name: &str, role: WadRole) -> WadId {
    m.wads.push(ModWad {
        name: wad_name(name),
        role,
        layout: WadLayout::Loose,
        files: Vec::new(),
        packed_locale: None,
    })
}

fn champion() -> WadRole {
    WadRole::Champion {
        champion: "test".into(),
    }
}

/// A mod of the champion's WAD with no file in it.
fn test_mod() -> (Mod, WadId) {
    let mut m = empty_mod();
    let wad = wad(&mut m, WAD, champion());
    (m, wad)
}

fn file_used(m: &mut Mod, wad: WadId, path: &str, bytes: &[u8], used: FileUse) -> FileId {
    m.push_file(FileNode {
        wad,
        hash: path_hash(path),
        path: Some(ChunkPath::parse(path).expect("a chunk path")),
        size: bytes.len() as u64,
        content: ContentId::of(bytes),
        kind: file_kind(bytes),
        bytes: ByteView::from(bytes),
        used,
        source: FileSource::Archive,
    })
    .expect("the path is free")
}

fn file(m: &mut Mod, wad: WadId, path: &str, bytes: &[u8]) -> FileId {
    file_used(m, wad, path, bytes, FileUse::Referenced)
}

/// Bytes that differ by tag, of one length.
fn blob(tag: &str) -> Vec<u8> {
    let mut bytes = tag.as_bytes().to_vec();
    bytes.resize(64, 0);
    bytes
}

fn builds() -> MemoryHistory {
    MemoryHistory::new(&["16.17.100", "16.18.200", "16.19.300"])
}

fn install(paths: &[&str]) -> BytesOrigin {
    BytesOrigin::Install {
        paths: paths.iter().map(|path| path_hash(path)).collect(),
    }
}

const BODY: &str = "assets/characters/test/skins/base/test_body.tex";

#[test]
fn a_file_nothing_loads_is_not_judged() {
    let mut world = World::new();
    world.ship(BODY, &blob("game body"));
    let (mut m, wad) = test_mod();
    let junk = file_used(
        &mut m,
        wad,
        "assets/mod/a/body_old.tex",
        &blob("old"),
        FileUse::Unused,
    );
    let unused_copy = file_used(&mut m, wad, BODY, &blob("game body"), FileUse::Unused);
    let used = file(&mut m, wad, "assets/mod/a/body.tex", &blob("own"));

    let facts = world.find(&m, &builds());
    assert!(facts.fact(junk).is_none());
    assert!(facts.fact(unused_copy).is_none());
    assert!(!facts.is_game(unused_copy));
    assert!(facts.game_paths(unused_copy).is_empty());
    assert_eq!(facts.at_path(unused_copy), None);
    let judged: Vec<FileId> = facts.iter().map(|(id, _)| id).collect();
    assert_eq!(judged, [used]);
}

#[test]
fn the_games_bytes_at_the_games_path_are_the_installs() {
    let mut world = World::new();
    world.ship(BODY, &blob("game body"));
    let (mut m, wad) = test_mod();
    let copy = file(&mut m, wad, BODY, &blob("game body"));

    let facts = world.find(&m, &builds());
    let fact = facts.fact(copy).expect("a used file is judged");
    assert_eq!(fact.at_path, AtPath::SameBytes);
    assert_eq!(fact.origin, install(&[BODY]));
    assert!(facts.is_game(copy));
    assert_eq!(facts.game_paths(copy), [path_hash(BODY)]);
    assert_eq!(facts.shipped(copy), None);
    assert!(facts.older_game().is_empty());
}

#[test]
fn other_bytes_at_a_game_path_are_the_mods_own() {
    let mut world = World::new();
    world.ship(BODY, &blob("game body"));
    let (mut m, wad) = test_mod();
    let own = file(&mut m, wad, BODY, &blob("repainted"));
    let longer = file(
        &mut m,
        wad,
        "assets/mod/a/body.tex",
        b"own bytes of another size",
    );

    let facts = world.find(&m, &builds());
    let fact = facts.fact(own).unwrap();
    assert_eq!(fact.at_path, AtPath::OtherBytes);
    assert_eq!(fact.origin, BytesOrigin::Own);
    assert!(!facts.is_game(own));
    assert!(facts.game_paths(own).is_empty());
    assert_eq!(facts.at_path(longer), Some(AtPath::NotGamePath));
    assert_eq!(facts.fact(longer).unwrap().origin, BytesOrigin::Own);
}

#[test]
fn the_games_bytes_under_another_path_are_found_among_the_chunks_of_their_size() {
    let mut world = World::new();
    world.ship(BODY, &blob("game body"));
    world.ship(
        "assets/characters/test/skins/base/other.tex",
        &blob("other"),
    );
    let (mut m, wad) = test_mod();
    let borrowed = file(&mut m, wad, "assets/mod/a/body.tex", &blob("game body"));

    let facts = world.find(&m, &builds());
    let fact = facts.fact(borrowed).unwrap();
    assert_eq!(fact.at_path, AtPath::NotGamePath);
    assert_eq!(fact.origin, install(&[BODY]));
    assert!(fact.is_game());
}

#[test]
fn equal_copies_are_ranked_by_how_long_their_path_is_to_live() {
    let mut world = World::new();
    let bytes = blob("shared texture");
    let base = "assets/characters/test/skins/base/shared.tex";
    let skin = "assets/characters/test/skins/skin3/shared.tex";
    let named = "assets/shared/particles/shared.tex";
    let unnamed = "assets/perks/shared.tex";
    world.ship_unnamed(unnamed, &bytes);
    world.ship(named, &bytes);
    world.ship(skin, &bytes);
    world.ship(base, &bytes);
    let (mut m, wad) = test_mod();
    let copy = file(&mut m, wad, skin, &bytes);

    let facts = world.find(&m, &builds());
    assert_eq!(facts.at_path(copy), Some(AtPath::SameBytes));
    assert_eq!(
        facts.game_paths(copy),
        [base, skin, named, unnamed].map(path_hash)
    );
    // The file's own path names its skin folder, whatever the best copy is.
    assert_eq!(
        facts.fact(copy).unwrap().skin_folder,
        Some(SkinFolder {
            character: "test".into(),
            slot: Slot(3)
        })
    );
}

#[test]
fn copies_of_one_rank_are_in_path_hash_order() {
    let mut world = World::new();
    let bytes = blob("shared texture");
    let paths = [
        "assets/shared/a.tex",
        "assets/shared/b.tex",
        "assets/shared/c.tex",
    ];
    for path in paths {
        world.ship(path, &bytes);
    }
    let (mut m, wad) = test_mod();
    let copy = file(&mut m, wad, "assets/mod/a/shared.tex", &bytes);

    let mut by_hash = paths.map(path_hash);
    by_hash.sort();
    assert_eq!(world.find(&m, &builds()).game_paths(copy), by_hash);
}

#[test]
fn bytes_only_the_history_holds_are_older_game_bytes() {
    let mut world = World::new();
    world.ship(BODY, &blob("body as it is"));
    let (mut m, wad) = test_mod();
    let old = blob("body as it was");
    let stale = file(&mut m, wad, BODY, &old);
    let moved = file(&mut m, wad, "assets/mod/a/cape.tex", &blob("cape"));
    file(&mut m, wad, "assets/mod/a/own.tex", &blob("own"));
    let history = builds().with_content(BODY, &old, 0..=1).with_content(
        "assets/characters/test/cape.tex",
        &blob("cape"),
        0..=0,
    );

    let facts = world.find(&m, &history);
    let fact = facts.fact(stale).unwrap();
    assert_eq!(fact.at_path, AtPath::OtherBytes);
    assert_eq!(
        fact.origin,
        BytesOrigin::History {
            life: Life::new(0, 1),
            path: path_hash(BODY),
            still_shipped: false
        }
    );
    assert!(facts.is_game(stale));
    assert!(facts.game_paths(stale).is_empty());
    assert_eq!(facts.shipped(stale), Some(Life::new(0, 1)));
    assert_eq!(facts.shipped(moved), Some(Life::new(0, 0)));

    let older = OlderGameFiles {
        wad,
        files: vec![stale, moved],
        bytes: 128,
        still_shipped: 0,
    };
    assert_eq!(facts.older_game(), std::slice::from_ref(&older));

    let mut log = Diagnostics::new();
    facts.report(&mut log.at(Stage::Load));
    assert_eq!(log.len(), 1);
    let raised = log.iter().next().unwrap();
    assert_eq!(raised.code, OlderGameFiles::CODE);
    assert_eq!(raised.code.name, "file.older-game");
    assert_eq!(
        (raised.channel(), raised.severity()),
        (Channel::Input, Severity::Warning)
    );
    assert_eq!(raised.subject, Subject::Wad(wad));
    assert_eq!(log.of::<OlderGameFiles>().next(), Some(&older));
    assert_eq!(
        raised.message(&Render::plain()),
        "2 files (0.0 MiB) of bytes the game shipped at earlier builds, which the install does \
         not hold"
    );
    assert_eq!(
        raised.data(),
        serde_json::json!({ "wad": "%w0", "files": ["%f0", "%f1"], "bytes": 128, "still_shipped": 0 })
    );
}

#[test]
fn older_game_bytes_are_reported_once_per_wad_in_wad_order() {
    let world = World::new();
    let mut m = empty_mod();
    let first = wad(&mut m, WAD, champion());
    let second = wad(&mut m, "Other.wad.client", WadRole::Other);
    let (a, b) = (blob("a"), blob("b"));
    let in_second = file(&mut m, second, "assets/b.tex", &b);
    let in_first = file(&mut m, first, "assets/a.tex", &a);
    let history = builds()
        .with_content("assets/a.tex", &a, 0..=0)
        .with_content("assets/b.tex", &b, 0..=0);

    let facts = world.find(&m, &history);
    let wads: Vec<(WadId, &[FileId])> = facts
        .older_game()
        .iter()
        .map(|of_wad| (of_wad.wad, &of_wad.files[..]))
        .collect();
    assert_eq!(wads, [(first, &[in_first][..]), (second, &[in_second][..])]);
}

#[test]
fn bytes_the_head_still_holds_and_the_install_does_not_are_counted_as_still_shipped() {
    // The install is behind the history's head: the head holds the bytes at a path the install
    // has other bytes at.
    let mut world = World::new();
    world.ship(BODY, &blob("body at the install's build"));
    let (mut m, wad) = test_mod();
    let ahead = blob("body at the head");
    let copy = file(&mut m, wad, "assets/mod/a/body.tex", &ahead);
    let history = builds().with_content(BODY, &ahead, 1..=2);

    let facts = world.find(&m, &history);
    assert_eq!(
        facts.fact(copy).unwrap().origin,
        BytesOrigin::History {
            life: Life::new(1, 2),
            path: path_hash(BODY),
            still_shipped: true
        }
    );
    assert_eq!(facts.older_game()[0].still_shipped, 1);
    let mut log = Diagnostics::new();
    facts.report(&mut log.at(Stage::Load));
    let message = log.iter().next().unwrap().message(&Render::plain());
    assert!(
        message.ends_with("not hold, 1 of them still shipped under another path"),
        "{message}"
    );
}

#[test]
fn the_history_names_a_path_outside_the_champions_wad_where_the_install_holds_the_bytes() {
    let mut world = World::new();
    let bytes = blob("a shared effect");
    let shared = "assets/shared/particles/glow.tex";
    let dropped = "assets/characters/test/skins/base/particles/glow.tex";
    world.ship_elsewhere(shared, &bytes);
    let (mut m, wad) = test_mod();
    let copy = file(&mut m, wad, dropped, &bytes);
    // The champion's own path left the game at build 1. The shared one lasts to the head.
    let history = builds()
        .with_content(dropped, &bytes, 0..=1)
        .with_content(shared, &bytes, 0..=2);

    let facts = world.find(&m, &history);
    let fact = facts.fact(copy).unwrap();
    assert_eq!(fact.at_path, AtPath::NotGamePath);
    assert_eq!(fact.origin, install(&[shared]));
    assert!(facts.older_game().is_empty());

    // Without a history the install is asked only in the champion's WAD.
    let alone = world.find(&m, &NoHistory);
    assert_eq!(alone.fact(copy).unwrap().origin, BytesOrigin::Own);
}

#[test]
fn a_path_whose_life_ended_before_the_head_is_not_read_from_the_install() {
    let mut world = World::new();
    let bytes = blob("a shared effect");
    let shared = "assets/shared/particles/glow.tex";
    // The install holds the bytes there, and the history says the path ended at build 1.
    world.ship_elsewhere(shared, &bytes);
    let (mut m, wad) = test_mod();
    let copy = file(&mut m, wad, "assets/mod/a/glow.tex", &bytes);
    let history = builds().with_content(shared, &bytes, 0..=1);

    let fact = world.find(&m, &history).fact(copy).cloned().unwrap();
    assert_eq!(fact.shipped(), Some(Life::new(0, 1)));
}

#[test]
fn without_a_history_older_game_bytes_look_like_the_mods_own() {
    let mut world = World::new();
    world.ship(BODY, &blob("body as it is"));
    let (mut m, wad) = test_mod();
    let stale = file(&mut m, wad, BODY, &blob("body as it was"));

    let facts = world.find(&m, &NoHistory);
    assert_eq!(facts.fact(stale).unwrap().origin, BytesOrigin::Own);
    assert!(facts.older_game().is_empty());
    let mut log = Diagnostics::new();
    facts.report(&mut log.at(Stage::Load));
    assert!(log.is_empty());
}

#[test]
fn a_skin_folder_and_effects_material_are_told_by_the_file_path_then_by_the_game_path() {
    let mut world = World::new();
    let wings = blob("wings of skin 7");
    let spark = blob("a spark of skin 2");
    world.ship("assets/characters/test/skins/skin07/wings.tex", &wings);
    world.ship(
        "assets/characters/test/skins/skin02/particles/spark.tex",
        &spark,
    );
    let (mut m, wad) = test_mod();
    let own_in_folder = file(
        &mut m,
        wad,
        "assets/characters/ally/skins/skin12/own.tex",
        &blob("own"),
    );
    let borrowed = file(&mut m, wad, "assets/mod/a/wings.tex", &wings);
    let effect = file(&mut m, wad, "assets/mod/a/spark.tex", &spark);
    let own_effect = file(&mut m, wad, "assets/mod/particles/own.tex", &blob("fx"));
    let plain = file(&mut m, wad, "assets/mod/a/plain.tex", &blob("plain"));
    let old = blob("an effect of skin 5, since removed");
    let older = file(&mut m, wad, "assets/mod/a/old.tex", &old);
    let old_path = "assets/characters/test/skins/skin05/particles/old.tex";
    world.name(old_path);
    let history = builds().with_content(old_path, &old, 0..=0);

    let facts = world.find(&m, &history);
    let folder = |file: FileId| {
        let fact = facts.fact(file).unwrap();
        let folder = fact.skin_folder.clone();
        (folder.map(|f| (f.character, f.slot.0)), fact.vfx)
    };
    assert_eq!(folder(own_in_folder), (Some(("ally".into(), 12)), false));
    assert_eq!(folder(borrowed), (Some(("test".into(), 7)), false));
    assert_eq!(folder(effect), (Some(("test".into(), 2)), true));
    assert_eq!(folder(own_effect), (None, true));
    assert_eq!(folder(plain), (None, false));
    assert_eq!(folder(older), (Some(("test".into(), 5)), true));
}

#[test]
fn a_path_only_a_table_spells_tells_no_skin_folder_and_no_effects_material() {
    // A chunk of a packed WAD has the path a table gives its hash. It is the mod's own file,
    // and the archive says nothing of whose folder it is in.
    let world = World::new();
    let (mut m, wad) = test_mod();
    let path = "assets/characters/test/skins/skin07/particles/own.tex";
    let spelled = file(&mut m, wad, path, &blob("own"));
    let other = "assets/characters/test/skins/skin08/particles/own.tex";
    let by_table = file(&mut m, wad, other, &blob("own too"));
    m.named_by_table.insert(by_table);
    assert_eq!(m.spelled_path(spelled).map(ChunkPath::as_str), Some(path));
    assert_eq!(m.spelled_path(by_table), None);

    let facts = world.find(&m, &builds());
    let folder = |file: FileId| {
        let fact = facts.fact(file).unwrap();
        (fact.skin_folder.clone().map(|f| f.slot.0), fact.vfx)
    };
    assert_eq!(folder(spelled), (Some(7), true));
    assert_eq!(folder(by_table), (None, false));
}

#[test]
fn a_game_path_no_table_names_tells_no_skin_folder() {
    let mut world = World::new();
    let wings = blob("wings of skin 7");
    world.ship_unnamed("assets/characters/test/skins/skin07/wings.tex", &wings);
    let (mut m, wad) = test_mod();
    let borrowed = file(&mut m, wad, "assets/mod/a/wings.tex", &wings);

    let facts = world.find(&m, &builds());
    let fact = facts.fact(borrowed).unwrap();
    assert!(fact.is_game());
    assert_eq!(fact.skin_folder, None);
}

#[test]
fn a_locale_wads_files_are_looked_for_in_the_champions_wad() {
    let mut world = World::new();
    world.ship(BODY, &blob("game body"));
    let mut m = empty_mod();
    let locale = wad(
        &mut m,
        "Test.en_US.wad.client",
        WadRole::Locale {
            champion: "test".into(),
            locale: "en_us".into(),
        },
    );
    let other = wad(&mut m, "Map11.wad.client", WadRole::Other);
    let in_locale = file(&mut m, locale, "assets/mod/a/body.tex", &blob("game body"));
    let in_other = file(&mut m, other, "assets/mod/b/body.tex", &blob("game body"));

    let facts = world.find(&m, &builds());
    assert_eq!(facts.fact(in_locale).unwrap().origin, install(&[BODY]));
    // A WAD of no champion is looked for under its own name, which this install does not have.
    assert_eq!(facts.fact(in_other).unwrap().origin, BytesOrigin::Own);
}

#[test]
fn no_more_chunks_of_a_files_size_are_read_than_the_cap_and_the_files_past_it_are_reported() {
    let mut world = World::new();
    let bytes = blob("the copy");
    for i in 0..MAX_SAME_SIZE_READS {
        world.ship(
            &format!("assets/filler/{i}.tex"),
            &blob(&format!("filler {i}")),
        );
    }
    world.ship("assets/characters/test/skins/base/last.tex", &bytes);
    let (mut m, wad) = test_mod();
    let copy = file(&mut m, wad, "assets/mod/a/copy.tex", &bytes);
    let facts = world.find(&m, &builds());
    assert_eq!(facts.fact(copy).unwrap().origin, BytesOrigin::Own);
    let mut log = Diagnostics::new();
    facts.report(&mut log.at(Stage::Load));
    let capped = SameSizeCapped {
        files: vec![copy],
        cap: MAX_SAME_SIZE_READS,
    };
    assert_eq!(log.of::<SameSizeCapped>().collect::<Vec<_>>(), [&capped]);

    // One filler fewer and the copy is within the cap.
    let mut world = World::new();
    for i in 1..MAX_SAME_SIZE_READS {
        world.ship(
            &format!("assets/filler/{i}.tex"),
            &blob(&format!("filler {i}")),
        );
    }
    world.ship("assets/characters/test/skins/base/last.tex", &bytes);
    let facts = world.find(&m, &builds());
    assert!(facts.is_game(copy));
    let mut log = Diagnostics::new();
    facts.report(&mut log.at(Stage::Load));
    assert_eq!(log.of::<SameSizeCapped>().count(), 0);
}

/// A game whose chunks are listed and do not read.
struct BrokenGame(MemoryGame);

impl Game for BrokenGame {
    fn build(&self) -> Option<&Build> {
        self.0.build()
    }
    fn describe(&self) -> String {
        "broken".into()
    }
    fn size(&self, path: WadHash) -> Option<u64> {
        self.0.size(path)
    }
    fn read(&self, _: WadHash) -> Result<Option<Vec<u8>>, GameError> {
        Err(GameError::Open(std::io::Error::other("the WAD is gone")))
    }
    fn wads(&self, path: WadHash) -> Vec<WadName> {
        self.0.wads(path)
    }
}

#[test]
fn a_game_chunk_that_does_not_read_is_a_fault() {
    let mut world = World::new();
    world.ship(BODY, &blob("game body"));
    let broken = GameView::new(Box::new(BrokenGame(world.game))).sizes(world.sizes);
    let (mut m, wad) = test_mod();
    let copy = file(&mut m, wad, BODY, &blob("game body"));

    let against = Against {
        game: &broken,
        history: &NoHistory,
        names: &NoNames,
    };
    // The chunk is there and does not read: the file is not judged as if the game lacked it.
    let found = FileProvenance::find(&m, &against);
    assert!(matches!(found, Err(Fault::Game(_))));
    assert_eq!(m.files[copy].size, blob("game body").len() as u64);
}

#[test]
fn a_history_that_does_not_answer_is_a_fault() {
    let mut world = World::new();
    world.ship(BODY, &blob("game body"));
    let (mut m, wad) = test_mod();
    let broken = BrokenHistory(builds());
    let game = world.view();
    let against = Against {
        game: &game,
        history: &broken,
        names: &NoNames,
    };

    // Bytes the install holds are found there, and the history is not asked.
    file(&mut m, wad, BODY, &blob("game body"));
    assert!(FileProvenance::find(&m, &against).is_ok());

    // Bytes it does not hold are the history's to place: they are not judged the mod's own.
    file(&mut m, wad, "assets/mod/a/old.tex", &blob("old"));
    let found = FileProvenance::find(&m, &against);
    assert!(matches!(found, Err(Fault::History(HistoryError::Io(_)))));
}

#[test]
fn the_facts_serialise_in_file_order_with_hashes_as_hex() {
    let mut world = World::new();
    world.ship(BODY, &blob("game body"));
    let (mut m, wad) = test_mod();
    file(&mut m, wad, BODY, &blob("game body"));
    let old = blob("old");
    file(&mut m, wad, "assets/mod/a/old.tex", &old);
    file_used(
        &mut m,
        wad,
        "assets/mod/a/unused.tex",
        &old,
        FileUse::Unused,
    );
    file(&mut m, wad, "assets/mod/particles/own.tex", &blob("own"));
    let history = builds().with_content("assets/old.tex", &old, 0..=1);

    let hex = |path: &str| format!("{:016x}", path_hash(path).0);
    let json = serde_json::to_value(world.find(&m, &history)).unwrap();
    assert_eq!(
        json,
        serde_json::json!([
            {
                "file": "%f0", "at_path": "same-bytes", "origin": "install",
                "paths": [hex(BODY)], "skin_folder": null, "vfx": false,
            },
            {
                "file": "%f1", "at_path": "not-game-path", "origin": "history",
                "life": { "first": 0, "last": 1 }, "path": hex("assets/old.tex"),
                "still_shipped": false, "skin_folder": null, "vfx": false,
            },
            {
                "file": "%f3", "at_path": "not-game-path", "origin": "own",
                "skin_folder": null, "vfx": true,
            },
        ])
    );
}

#[test]
fn a_particles_folder_is_told_in_any_case() {
    assert!(is_vfx_path(
        "ASSETS/Characters/Test/Skins/Base/Particles/x.tex"
    ));
    assert!(!is_vfx_path(
        "assets/characters/test/skins/base/particles.tex"
    ));
}

#[test]
fn file_provenance_raises_two_codes() {
    assert_eq!(CODES, [OlderGameFiles::CODE, SameSizeCapped::CODE]);
}
