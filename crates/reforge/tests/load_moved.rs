//! `moved-assets`: a file at a path the game shipped once and does not ship, made where the game
//! ships that asset, found by the renames Riot made. Over a mod, a game and a history built
//! here.

use std::collections::BTreeMap;

use fs_err as fs;
use reforge::archive::file_kind;
use reforge::diag::{Channel, Diag, Diagnostics, Render, Stage};
use reforge::game::{NameBook, Names};
use reforge::history::{
    BuildRow, FileHit, HeldAt, History, HistoryError, Life, MeshHit, NamedBy, NoHistory, ObjectHit,
    PathHit, PlayedBy, SkeletonHit, Stamp, UnitHit, WemPlayed,
};
use reforge::load::files::{FileFixCx, FileFixup, one_record};
use reforge::load::legacy::{Conversion, ConvertError, LegacyFormats};
use reforge::load::moved::{
    CODES, FilesMoved, MovedAssets, MovesFailed, RENAMES, Rename, Step, candidates,
};
use reforge::model::entity::{
    FileId, FileKind, FileNode, FileSource, FileUse, MadeBy, ModMeta, ModWad, WadId, WadLayout,
    WadRole,
};
use reforge::model::{
    ChunkPath, ClipKey, ContentId, FileKey, Mod, NoNames, NormKey, ObjectKey, Spelled, WadHash,
    WadName, path_hash,
};
use reforge_testkit::{MemoryGame, MemoryHistory};

const BASE: &str = "assets/characters/test/skins/base";
const BASE_MESH: &str = "assets/characters/test/skins/base/test.skn";
const BASE_SKEL: &str = "assets/characters/test/skins/base/test.skl";
const BASE_TEX: &str = "assets/characters/test/skins/base/test.tex";

/// A DDS by its magic.
fn dds() -> Vec<u8> {
    let mut out = b"DDS ".to_vec();
    out.extend(0..120u8);
    out
}

/// A property bin by its magic and version.
fn bin() -> Vec<u8> {
    let mut out = b"PROP".to_vec();
    out.extend(3u32.to_le_bytes());
    out.extend([0; 8]);
    out
}

/// A history of two builds in which the game shipped a file at each of `former` at the first.
fn history(former: &[&str]) -> MemoryHistory {
    let history = MemoryHistory::new(&["14.1.1", "16.19.1"]);
    former
        .iter()
        .fold(history, |history, path| history.with_path(path, 0..=0))
}

/// A mod, the game it is judged against, and the run's names and log.
struct Fixture {
    module: Mod,
    wad: WadId,
    game: MemoryGame,
    book: NameBook,
    log: Diagnostics,
}

impl Fixture {
    /// A mod of one empty champion WAD, and a game shipping the base's mesh, skeleton and
    /// texture.
    fn new() -> Self {
        let mut module = Mod::new(ModMeta {
            file_name: Spelled::new("moved.fantome"),
            sha256: ContentId::of(b"moved"),
            mod_id: "000000000000".into(),
            name: None,
            author: None,
            version: None,
            description: None,
        });
        let name = WadName::parse("Test.wad.client").unwrap();
        let wad = module.wads.push(ModWad {
            role: WadRole::Champion {
                champion: name.stem().to_string(),
            },
            name,
            layout: WadLayout::Loose,
            files: Vec::new(),
            packed_locale: None,
        });
        let mut game = MemoryGame::new("Test.wad.client", "16.19.1");
        for path in [BASE_MESH, BASE_SKEL, BASE_TEX] {
            game.insert(path, format!("the game's {path}").into_bytes());
        }
        Self {
            module,
            wad,
            game,
            book: NameBook::new(),
            log: Diagnostics::new(),
        }
    }

    /// A mod of `files`, moved against a history in which the game shipped a file at each of
    /// `former`.
    fn moved(files: &[(&str, &[u8])], former: &[&str]) -> Self {
        let mut f = Self::new();
        for (path, bytes) in files {
            f.file(path, bytes);
        }
        f.run(&MovedAssets, &history(former));
        f
    }

    /// Adds a file the archive ships, of the kind its bytes are.
    fn file(&mut self, path: &str, bytes: &[u8]) -> FileId {
        let path = ChunkPath::parse(path).unwrap();
        self.module
            .push_file(FileNode {
                wad: self.wad,
                hash: path.hash(),
                path: Some(path),
                size: bytes.len() as u64,
                content: ContentId::of(bytes),
                kind: file_kind(bytes),
                bytes: bytes.into(),
                used: FileUse::Unused,
                source: FileSource::Archive,
            })
            .expect("the path is free")
    }

    /// Runs a fixup as `load` runs it.
    fn run(&mut self, fixup: &dyn FileFixup, history: &dyn History) {
        let mut stage = self.log.at(Stage::Load);
        let mut sink = stage.by(fixup.id(), one_record(fixup));
        // A history that does not answer is the caller's fault to handle: nothing is made.
        let _ = fixup.fix(&mut FileFixCx {
            module: &mut self.module,
            game: &self.game,
            history,
            names: &Names::default(),
            book: &mut self.book,
            sink: &mut sink,
        });
    }

    /// The file at a path.
    fn at(&self, path: &str) -> Option<(FileId, &FileNode)> {
        let id = self.module.file_at(self.wad, path_hash(path))?;
        Some((id, &self.module.files[id]))
    }

    /// The file a fixup made at a path, and the file it was made from.
    fn made(&self, path: &str) -> Option<(&FileNode, FileId)> {
        let (_, file) = self.at(path)?;
        match file.source {
            FileSource::Made { from, by } => {
                assert_eq!(by, MadeBy::MovedAsset);
                Some((file, from))
            }
            _ => None,
        }
    }

    fn id_of(&self, path: &str) -> FileId {
        self.at(path)
            .unwrap_or_else(|| panic!("no file at {path}"))
            .0
    }

    fn codes(&self) -> Vec<&'static str> {
        self.log.iter().map(|d| d.code.name).collect()
    }

    /// The message of every diagnostic, with the mod's files spelled.
    fn messages(&self) -> Vec<String> {
        let cx = Render {
            names: &NoNames,
            module: Some(&self.module),
        };
        self.log.iter().map(|d| d.message(&cx)).collect()
    }
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

/// The path one rename gives, by the rename's name.
fn renamed(rename: &str, path: &str) -> Option<String> {
    let rename = Rename::named(rename).unwrap_or_else(|| panic!("no rename {rename}"));
    let to = rename.apply(&ChunkPath::parse(path).unwrap())?;
    Some(to.as_str().to_string())
}

fn candidate_paths(path: &str) -> Vec<String> {
    candidates(&ChunkPath::parse(path).unwrap())
        .into_iter()
        .map(|c| c.path.as_str().to_string())
        .collect()
}

#[test]
fn the_renames_are_listed_in_the_order_they_are_tried() {
    let listed: Vec<(&str, Vec<String>)> = RENAMES
        .iter()
        .map(|r| (r.name, r.records.iter().map(ToString::to_string).collect()))
        .collect();
    let records = |codes: &[&str]| codes.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(
        listed,
        [
            ("variant-tag", records(&["#GAME-CHANGE-0009"])),
            ("resolution-prefix-2x", records(&["#GAME-CHANGE-0010"])),
            ("resolution-prefix-4x", records(&["#GAME-CHANGE-0010"])),
            (
                "variant-tag-and-prefix-2x",
                records(&["#GAME-CHANGE-0009", "#GAME-CHANGE-0010"])
            ),
            (
                "variant-tag-and-prefix-4x",
                records(&["#GAME-CHANGE-0009", "#GAME-CHANGE-0010"])
            ),
            ("base-suffix", records(&["#GAME-CHANGE-0011"])),
            ("shared-bin", records(&["#GAME-CHANGE-0012"])),
        ]
    );
    assert_eq!(
        Rename::named("variant-tag-and-prefix-4x").unwrap().steps,
        [Step::DropVariantTag, Step::DropPrefix("4x_")]
    );
    assert!(Rename::named("backup").is_none());
}

#[test]
fn a_variant_tag_is_dropped() {
    let rule = |path| renamed("variant-tag", path);
    assert_eq!(
        rule("assets/characters/yunara/skins/base/yunara_base.yunara.skl").as_deref(),
        Some("assets/characters/yunara/skins/base/yunara_base.skl")
    );
    assert_eq!(
        rule("assets/characters/gwen/skins/skin38/gwen_skin38_weapons_tx_cm.chroma_gwen_battlequeens.tex")
            .as_deref(),
        Some("assets/characters/gwen/skins/skin38/gwen_skin38_weapons_tx_cm.tex")
    );
    // Every part between the first dot and the extension is the tag.
    assert_eq!(rule("a/b.one.two.dds").as_deref(), Some("a/b.dds"));
    assert_eq!(rule("b.tag.skn").as_deref(), Some("b.skn"));
    assert_eq!(rule("a/b.dds"), None);
    assert_eq!(rule("a.tag/b"), None);
}

#[test]
fn a_resolution_prefix_is_dropped() {
    assert_eq!(
        renamed("resolution-prefix-4x", "a/4x_test_tx_cm.dds").as_deref(),
        Some("a/test_tx_cm.dds")
    );
    assert_eq!(
        renamed("resolution-prefix-2x", "a/2x_test_tx_cm.tex").as_deref(),
        Some("a/test_tx_cm.tex")
    );
    assert_eq!(renamed("resolution-prefix-4x", "a/2x_test.dds"), None);
    assert_eq!(renamed("resolution-prefix-2x", "a/4x_test.dds"), None);
    assert_eq!(renamed("resolution-prefix-4x", "4x_a/test.dds"), None);
    // What is left is no file name.
    assert_eq!(renamed("resolution-prefix-4x", "a/4x_"), None);
}

#[test]
fn a_variant_tag_and_a_resolution_prefix_are_dropped_together() {
    assert_eq!(
        renamed("variant-tag-and-prefix-4x", "a/4x_test.rework.dds").as_deref(),
        Some("a/test.dds")
    );
    assert_eq!(
        renamed("variant-tag-and-prefix-2x", "a/2x_test.rework.dds").as_deref(),
        Some("a/test.dds")
    );
    assert_eq!(renamed("variant-tag-and-prefix-4x", "a/4x_test.dds"), None);
    assert_eq!(
        renamed("variant-tag-and-prefix-4x", "a/test.rework.dds"),
        None
    );
}

#[test]
fn a_base_mesh_and_skeleton_take_a_base_suffix() {
    let rule = |path| renamed("base-suffix", path);
    assert_eq!(
        rule("assets/characters/vi/skins/base/vi.skn").as_deref(),
        Some("assets/characters/vi/skins/base/vi_base.skn")
    );
    assert_eq!(
        rule("assets/characters/vi/skins/base/vi.skl").as_deref(),
        Some("assets/characters/vi/skins/base/vi_base.skl")
    );
    // #GAME-CHANGE-0011: no other file of the folder.
    assert_eq!(rule("assets/characters/vi/skins/base/vi.dds"), None);
    assert_eq!(rule("assets/characters/vi/skins/base/vi_tx_cm.tex"), None);
    assert_eq!(rule("assets/characters/vi/skins/base/vi.bin"), None);
    assert_eq!(rule("assets/characters/vi/skins/base/vi"), None);
    // Not a name that has the suffix or holds a tag, and not a skin's own folder.
    assert_eq!(rule("assets/characters/vi/skins/base/vi_base.skn"), None);
    assert_eq!(rule("assets/characters/vi/skins/base/vi.rework.skn"), None);
    assert_eq!(rule("assets/characters/vi/skins/skin03/vi.skn"), None);
    assert_eq!(
        rule("assets/characters/vi/skins/base/particles/vi.scb"),
        None
    );
    assert_eq!(rule("skins/base/vi.skn"), None);
}

#[test]
fn a_shared_skin_bin_moves_into_the_characters_folder() {
    let rule = |path| renamed("shared-bin", path);
    assert_eq!(
        rule("DATA/Darius_Skins_Skin0_Skins_Skin1_Skins_Skin5.bin").as_deref(),
        Some("data/characters/darius/darius_multi_skins_skin0_skins_skin1_skins_skin5.bin")
    );
    assert_eq!(rule("data/characters/darius_skins_skin0.bin"), None);
    assert_eq!(rule("data/darius_skins_skin0.txt"), None);
    assert_eq!(rule("data/_skins_skin0.bin"), None);
    assert_eq!(rule("data/da-rius_skins_skin0.bin"), None);
    assert_eq!(rule("data/darius.bin"), None);
}

#[test]
fn the_candidates_of_a_path_are_each_rename_as_the_twin_and_then_as_it_is() {
    assert_eq!(
        candidate_paths("ASSETS/Characters/Test/Skins/Base/4x_Test.Rework.dds"),
        [
            "assets/characters/test/skins/base/4x_test.tex",
            "assets/characters/test/skins/base/4x_test.dds",
            "assets/characters/test/skins/base/test.rework.tex",
            "assets/characters/test/skins/base/test.rework.dds",
            "assets/characters/test/skins/base/test.tex",
            "assets/characters/test/skins/base/test.dds",
        ]
    );
    let all = candidates(
        &ChunkPath::parse("assets/characters/test/skins/base/4x_test.rework.dds").unwrap(),
    );
    let found: Vec<(&str, Option<&str>, usize)> = all
        .iter()
        .map(|c| (c.rename.name, c.twin_of.map(|f| f.dropped), c.rank))
        .collect();
    assert_eq!(
        found,
        [
            ("variant-tag", Some("dds"), 0),
            ("variant-tag", None, 1),
            ("resolution-prefix-4x", Some("dds"), 4),
            ("resolution-prefix-4x", None, 5),
            ("variant-tag-and-prefix-4x", Some("dds"), 8),
            ("variant-tag-and-prefix-4x", None, 9),
        ]
    );
    // A base mesh and skeleton took a `_base` suffix; a skin's own folder did not.
    assert_eq!(
        candidate_paths("assets/characters/test/skins/base/test.skn"),
        ["assets/characters/test/skins/base/test_base.skn"]
    );
    assert!(candidate_paths("assets/characters/test/skins/base/test_base.skn").is_empty());
    assert!(candidate_paths("assets/characters/test/skins/base/test.dds").is_empty());
    assert!(candidate_paths("assets/characters/test/skins/skin03/test.skn").is_empty());
    // A bin Riot shares between skins moved into the character's folder.
    assert_eq!(
        candidate_paths("DATA/Test_Skins_Skin0_Skins_Skin3.bin"),
        ["data/characters/test/test_multi_skins_skin0_skins_skin3.bin"]
    );
}

#[test]
fn a_variant_tag_riot_dropped_is_followed() {
    // The mesh as the game once named it. The game ships it untagged.
    let old = format!("{BASE}/test.test_rework.skn");
    let f = Fixture::moved(&[(&old, b"the mod's own mesh")], &[&old]);
    let (mesh, from) = f
        .made(BASE_MESH)
        .expect("the mesh is made where the game ships it");
    assert_eq!(from, f.id_of(&old));
    assert_eq!(
        mesh.kind,
        FileKind::Other,
        "the bytes as they were, of the kind they are"
    );
    assert_eq!(&*mesh.bytes, b"the mod's own mesh");
    assert_eq!(mesh.content, ContentId::of(b"the mod's own mesh"));
    assert_eq!(mesh.used, FileUse::Unused);
    assert_eq!(mesh.path.as_ref().unwrap().as_str(), BASE_MESH);
    // The original stays, and the run's names learn the path made.
    assert_eq!(f.module.files.len(), 2);
    assert_eq!(f.book.path(path_hash(BASE_MESH)), Some(BASE_MESH));

    assert_eq!(f.codes(), ["file.moved"]);
    let found = one::<FilesMoved>(&f.log);
    assert_eq!(found.wad, f.wad);
    assert!(found.passed_over.is_empty());
    let moved = &found.files[0];
    assert_eq!((moved.from, moved.made), (from, f.id_of(BASE_MESH)));
    assert_eq!(
        (moved.rename.name, moved.how),
        ("variant-tag", Conversion::AsIs)
    );
    let diagnostic = f.log.iter().next().unwrap();
    assert_eq!(diagnostic.by.as_deref(), Some("moved-assets"));
    assert_eq!(diagnostic.record, None);
    assert_eq!(
        diagnostic.data()["files"][0],
        serde_json::json!({
            "from": "%f0", "made": "%f1", "rename": "variant-tag", "how": "as-is",
            "records": ["#GAME-CHANGE-0009"]
        })
    );
}

#[test]
fn a_resolution_prefix_is_dropped_and_the_texture_renamed() {
    let old = format!("{BASE}/4x_test.dds");
    let f = Fixture::moved(&[(&old, &dds())], &[&old]);
    let (tex, _) = f.made(BASE_TEX).expect("the texture is made");
    assert_eq!(tex.kind, FileKind::Dds);
    assert_eq!(&*tex.bytes, dds().as_slice());
    let moved = &one::<FilesMoved>(&f.log).files[0];
    assert_eq!(
        (moved.rename.name, moved.how),
        ("resolution-prefix-4x", Conversion::DdsAsTex)
    );
    // The rename's game change, then the one that dropped the format.
    let records: Vec<String> = moved.records.iter().map(ToString::to_string).collect();
    assert_eq!(records, ["#GAME-CHANGE-0010", "#GAME-CHANGE-0007"]);
    assert_eq!(
        f.messages(),
        [
            "1 file at paths the game shipped at earlier builds, made where it ships the asset \
          (1 by resolution-prefix-4x): 1 .dds as .tex, unchanged"
        ]
    );
}

#[test]
fn the_best_name_wins_and_the_rest_are_passed_over() {
    // #ADR-0038: of both reduced copies of one texture, the half-size one is made and the
    // quarter-size one passed over.
    let (four, two) = (format!("{BASE}/4x_test.dds"), format!("{BASE}/2x_test.dds"));
    let files: [(&str, &[u8]); 2] = [(&four, b"DDS the 4x one"), (&two, &dds())];
    let f = Fixture::moved(&files, &[&four, &two]);
    let (_, from) = f.made(BASE_TEX).expect("made");
    assert_eq!(from, f.id_of(&two));
    let found = one::<FilesMoved>(&f.log);
    assert_eq!(found.passed_over, [f.id_of(&four)]);
    assert_eq!(found.files.len(), 1);
    assert!(f.messages()[0].ends_with(
        "(1 by resolution-prefix-2x): 1 .dds as .tex, unchanged; 1 passed over, the asset \
         already had a file of the mod"
    ));
}

#[test]
fn of_two_files_with_one_rename_the_first_is_made() {
    let (one_tag, other_tag) = (format!("{BASE}/test.a.skn"), format!("{BASE}/test.b.skn"));
    let files: [(&str, &[u8]); 2] = [(&other_tag, b"the b mesh"), (&one_tag, b"the a mesh")];
    let f = Fixture::moved(&files, &[&one_tag, &other_tag]);
    let (mesh, from) = f.made(BASE_MESH).expect("made");
    assert_eq!(
        (from, &*mesh.bytes),
        (f.id_of(&other_tag), &b"the b mesh"[..])
    );
    assert_eq!(one::<FilesMoved>(&f.log).passed_over, [f.id_of(&one_tag)]);
}

#[test]
fn a_file_that_is_the_games_own_at_its_path_gives_way_to_the_mods_own_bytes() {
    // #ADR-0023: two files under one tag. The first is what the game shipped at that path,
    // a stale copy at a former path, and the second the mod's own texture.
    let (one_tag, other_tag) = (format!("{BASE}/test.a.skn"), format!("{BASE}/test.b.skn"));
    let files: [(&str, &[u8]); 2] = [(&one_tag, b"the game's a mesh"), (&other_tag, b"own")];
    let mut f = Fixture::new();
    for (path, bytes) in files {
        f.file(path, bytes);
    }
    let history = history(&[&one_tag, &other_tag]);
    let history = history.with_content(&one_tag, b"the game's a mesh", 0..=0);
    f.run(&MovedAssets, &history);
    let (mesh, from) = f.made(BASE_MESH).expect("made");
    assert_eq!((from, &*mesh.bytes), (f.id_of(&other_tag), &b"own"[..]));
    assert_eq!(one::<FilesMoved>(&f.log).passed_over, [f.id_of(&one_tag)]);

    // Alone, the game's own file at the former path is still made the asset's.
    let mut f = Fixture::new();
    f.file(&one_tag, b"the game's a mesh");
    f.run(&MovedAssets, &history);
    let (_, from) = f.made(BASE_MESH).expect("made");
    assert_eq!(from, f.id_of(&one_tag));
}

#[test]
fn the_mods_own_file_at_the_path_wins() {
    let old = format!("{BASE}/4x_test.dds");
    let files: [(&str, &[u8]); 2] = [(&old, &dds()), (BASE_TEX, b"the mod's own texture")];
    let f = Fixture::moved(&files, &[&old]);
    assert!(f.made(BASE_TEX).is_none());
    assert!(f.log.is_empty(), "{:?}", f.codes());
    assert_eq!(f.module.files.len(), 2);
}

#[test]
fn a_path_the_game_never_had_is_not_moved() {
    // An author's own `.backup` copy: never the game's, so it does not take over the mesh.
    let old = format!("{BASE}/test.backup.skn");
    let files: [(&str, &[u8]); 2] = [(&old, b"a backup"), (BASE_SKEL, b"the mod's own skeleton")];
    let f = Fixture::moved(&files, &[]);
    assert!(f.made(BASE_MESH).is_none());
    assert!(f.log.is_empty(), "{:?}", f.codes());

    // With no history nothing is known of former paths at all.
    let mut f = Fixture::new();
    f.file(&old, b"a backup");
    f.run(&MovedAssets, &NoHistory);
    assert!(f.made(BASE_MESH).is_none());
    assert!(f.log.is_empty(), "{:?}", f.codes());
}

#[test]
fn a_file_at_a_path_the_game_ships_is_not_moved() {
    // The game holds the tagged path too: the file overrides it there.
    let old = format!("{BASE}/test.test_rework.skn");
    let mut f = Fixture::new();
    f.game.insert(&old, b"the game's tagged mesh".to_vec());
    f.file(&old, b"the mod's own mesh");
    f.run(&MovedAssets, &history(&[&old]));
    assert!(f.made(BASE_MESH).is_none());
    assert!(f.log.is_empty(), "{:?}", f.codes());
}

#[test]
fn the_first_candidate_the_game_ships_is_the_path() {
    // The game ships the name without its tag and the name without tag and prefix. The first
    // is the nearer rename.
    let old = format!("{BASE}/4x_test.rework.dds");
    let untagged = format!("{BASE}/4x_test.tex");
    let mut f = Fixture::new();
    f.game.insert(&untagged, b"the game's 4x".to_vec());
    f.file(&old, &dds());
    f.run(&MovedAssets, &history(&[&old]));
    assert!(f.made(&untagged).is_some());
    assert!(f.made(BASE_TEX).is_none());
    assert_eq!(
        one::<FilesMoved>(&f.log).files[0].rename.name,
        "variant-tag"
    );
}

#[test]
fn a_base_mesh_is_made_with_the_suffix_the_game_ships_it_under() {
    let (old, new) = (format!("{BASE}/vi.skn"), format!("{BASE}/vi_base.skn"));
    let mut f = Fixture::new();
    f.game.insert(&new, b"the game's mesh".to_vec());
    f.file(&old, b"the mod's own mesh");
    f.run(&MovedAssets, &history(&[&old]));
    assert!(f.made(&new).is_some());
    let moved = &one::<FilesMoved>(&f.log).files[0];
    assert_eq!(moved.rename.name, "base-suffix");
    assert_eq!(moved.records[0].to_string(), "#GAME-CHANGE-0011");
}

#[test]
fn a_base_texture_is_not_made_under_a_base_suffix() {
    // #GAME-CHANGE-0011: the game renamed the mesh and the skeleton. A texture it once held in
    // the base folder is no earlier name of the one it ships with the suffix.
    let (old, new) = (format!("{BASE}/vi.dds"), format!("{BASE}/vi_base.tex"));
    let mut f = Fixture::new();
    f.game.insert(&new, b"the game's other texture".to_vec());
    f.game
        .insert(&format!("{BASE}/vi_base.dds"), b"and its dds".to_vec());
    f.file(&old, &dds());
    f.run(&MovedAssets, &history(&[&old]));
    assert!(f.at(&new).is_none());
    assert_eq!(f.module.files.len(), 1);
    assert!(f.log.is_empty(), "{:?}", f.codes());
    // The file is covered by no file at the suffixed name.
    let path = ChunkPath::parse(&old).unwrap();
    assert!(MovedAssets.destinations(&path, &f.game).is_empty());

    // The skeleton beside it is made.
    let (old, new) = (format!("{BASE}/vi.skl"), format!("{BASE}/vi_base.skl"));
    f.game.insert(&new, b"the game's skeleton".to_vec());
    f.file(&old, b"the mod's own skeleton");
    f.run(&MovedAssets, &history(&[&old]));
    assert!(f.made(&new).is_some());
    assert_eq!(
        one::<FilesMoved>(&f.log).files[0].rename.name,
        "base-suffix"
    );
}

#[test]
fn a_shared_bin_is_made_in_the_characters_folder_and_other_bytes_are_not() {
    let old = "data/test_skins_skin0_skins_skin3.bin";
    let new = "data/characters/test/test_multi_skins_skin0_skins_skin3.bin";
    let mut f = Fixture::new();
    f.game.insert(new, b"the game's shared bin".to_vec());
    f.file(old, &bin());
    f.run(&MovedAssets, &history(&[old]));
    let (made, _) = f.made(new).expect("the bin is made");
    assert_eq!(made.kind, FileKind::Bin);
    let moved = &one::<FilesMoved>(&f.log).files[0];
    assert_eq!(
        (moved.rename.name, moved.how),
        ("shared-bin", Conversion::AsIs)
    );
    assert_eq!(moved.records[0].to_string(), "#GAME-CHANGE-0012");

    // A bin path is loaded as a bin: other bytes there are no bin the game moved.
    let mut f = Fixture::new();
    f.game.insert(new, b"the game's shared bin".to_vec());
    f.file(old, b"a note left in the data folder");
    f.run(&MovedAssets, &history(&[old]));
    assert!(f.made(new).is_none());
    assert!(f.log.is_empty(), "{:?}", f.codes());
}

#[test]
fn bytes_that_are_no_texture_do_not_take_a_texture_path() {
    let old = format!("{BASE}/4x_test.dds");
    let f = Fixture::moved(&[(&old, b"a note named like a texture")], &[&old]);
    assert!(f.made(BASE_TEX).is_none());
    assert!(f.log.is_empty(), "{:?}", f.codes());
}

#[test]
fn a_moved_mesh_that_does_not_read_stays_and_is_reported() {
    let (old, new) = (format!("{BASE}/4x_bomb.sco"), format!("{BASE}/bomb.scb"));
    let mut f = Fixture::new();
    f.game.insert(&new, b"the game's bomb".to_vec());
    let broken = f.file(&old, b"[ObjectBegin]\nName= bomb\n\x1b[31mno mesh\n");
    assert_eq!(f.module.files[broken].kind, FileKind::Sco);
    f.run(&MovedAssets, &history(&[&old]));
    assert!(f.made(&new).is_none());
    assert_eq!(f.codes(), ["file.move-failed"]);
    let found = one::<MovesFailed>(&f.log);
    assert_eq!((found.wad, found.files.len()), (f.wad, 1));
    assert_eq!(found.files[0].file, broken);
    assert!(matches!(found.files[0].why, ConvertError::Read { .. }));
    let message = &f.messages()[0];
    assert!(
        message.starts_with(&format!(
            "1 file at former paths that do not convert, left where they are: {old} \
             (not a readable SCO: "
        )),
        "{message}"
    );
    assert!(!message.contains('\u{1b}'), "{message:?}");
}

#[test]
fn a_file_a_twin_was_made_of_is_not_moved_too() {
    // The game ships the twin of the prefixed name and the name without the prefix. The twin
    // is the file's asset.
    let old = format!("{BASE}/4x_test.dds");
    let twin = format!("{BASE}/4x_test.tex");
    let mut f = Fixture::new();
    f.game.insert(&twin, b"the game's 4x".to_vec());
    f.file(&old, &dds());
    let history = history(&[&old]);
    f.run(&LegacyFormats, &history);
    f.run(&MovedAssets, &history);
    assert_eq!(f.codes(), ["file.format-converted"]);
    assert!(f.at(&twin).is_some());
    assert!(f.at(BASE_TEX).is_none());
}

#[test]
fn a_second_run_makes_nothing() {
    let (mesh, four, two) = (
        format!("{BASE}/test.test_rework.skn"),
        format!("{BASE}/4x_test.dds"),
        format!("{BASE}/2x_test.dds"),
    );
    let files: [(&str, &[u8]); 3] = [
        (&mesh, b"the mod's own mesh"),
        (&four, &dds()),
        (&two, b"DDS the 2x one"),
    ];
    let mut f = Fixture::moved(&files, &[&mesh, &four, &two]);
    // The files are made in the order of their paths.
    let made: Vec<&str> = f
        .module
        .files
        .values()
        .skip(3)
        .map(|file| file.path.as_ref().unwrap().as_str())
        .collect();
    assert_eq!(made, [BASE_MESH, BASE_TEX]);
    let (files, diagnostics) = (f.module.files.len(), f.log.len());
    assert_eq!((files, diagnostics), (5, 1));
    f.run(&MovedAssets, &history(&[&mesh, &four, &two]));
    assert_eq!((f.module.files.len(), f.log.len()), (files, diagnostics));
}

/// A history whose index does not answer for a path.
struct BrokenHistory(MemoryHistory);

impl History for BrokenHistory {
    fn stamp(&self) -> Option<&Stamp> {
        self.0.stamp()
    }
    fn builds(&self) -> &[BuildRow] {
        self.0.builds()
    }
    fn file(&self, key: FileKey) -> Result<Option<FileHit>, HistoryError> {
        self.0.file(key)
    }
    fn path(&self, _: WadHash) -> Result<Option<PathHit>, HistoryError> {
        Err(HistoryError::Io(std::io::Error::other("the disk failed")))
    }
    fn held(&self, path: WadHash, key: FileKey) -> Result<Option<Life>, HistoryError> {
        self.0.held(path, key)
    }
    fn paths_of(&self, key: FileKey) -> Result<Vec<HeldAt>, HistoryError> {
        self.0.paths_of(key)
    }
    fn object(&self, key: ObjectKey) -> Result<Option<ObjectHit>, HistoryError> {
        self.0.object(key)
    }
    fn objects_by_norm(&self, norm: NormKey) -> Result<Vec<ObjectHit>, HistoryError> {
        self.0.objects_by_norm(norm)
    }
    fn clip(&self, key: ClipKey) -> Result<Option<Life>, HistoryError> {
        self.0.clip(key)
    }
    fn named_by(&self, path: WadHash) -> Result<Vec<NamedBy>, HistoryError> {
        self.0.named_by(path)
    }
    fn units_loading(&self, bank: WadHash) -> Result<Vec<UnitHit>, HistoryError> {
        self.0.units_loading(bank)
    }
    fn event_wems(&self, bank: WadHash, event: u32) -> Result<Vec<WemPlayed>, HistoryError> {
        self.0.event_wems(bank, event)
    }
    fn events_playing(&self, wem: u32) -> Result<Vec<PlayedBy>, HistoryError> {
        self.0.events_playing(wem)
    }
    fn skeletons(&self, path: WadHash) -> Result<Vec<SkeletonHit>, HistoryError> {
        self.0.skeletons(path)
    }
    fn meshes(&self, path: WadHash) -> Result<Vec<MeshHit>, HistoryError> {
        self.0.meshes(path)
    }
}

#[test]
fn a_history_that_does_not_answer_moves_nothing() {
    let old = format!("{BASE}/test.test_rework.skn");
    let mut f = Fixture::new();
    f.file(&old, b"the mod's own mesh");
    let broken = BrokenHistory(history(&[&old]));
    f.run(&MovedAssets, &broken);
    assert!(f.made(BASE_MESH).is_none());
    assert!(f.log.is_empty(), "{:?}", f.codes());

    let mut stage = f.log.at(Stage::Load);
    let result = MovedAssets.try_fix(&mut FileFixCx {
        module: &mut f.module,
        game: &f.game,
        history: &broken,
        names: &Names::default(),
        book: &mut f.book,
        sink: &mut stage,
    });
    assert!(matches!(result, Err(HistoryError::Io(_))));
    assert_eq!(f.module.files.len(), 1);
}

#[test]
fn the_codes_are_the_ones_the_codes_table_lists() {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design/CODES.md");
    let text = fs::read_to_string(file).unwrap();
    let documented: BTreeMap<&str, &str> = text
        .lines()
        .filter_map(|line| {
            let mut cells = line.strip_prefix("| `")?.split('|');
            let code = cells.next()?.trim().trim_end_matches('`');
            Some((code, cells.next()?.trim()))
        })
        .collect();
    assert_eq!(CODES, [FilesMoved::CODE, MovesFailed::CODE]);
    for code in CODES {
        assert_eq!(
            documented.get(code.name).copied(),
            Some(code.severity.as_str()),
            "{code}"
        );
        assert_eq!(
            code.channel,
            Channel::Output,
            "{code}: what a file fixup made"
        );
    }
}
