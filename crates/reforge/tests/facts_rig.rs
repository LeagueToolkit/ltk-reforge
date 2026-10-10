//! A unit's rig against the game's skins: the mesh and its submeshes, the skeleton, the clips
//! at a slot, and the typed questions the game answers them from (#ADR-0080). The game is
//! `reforge_testkit::skins`, with rig files that read where a test needs them.

use reforge::facts::rig::GraphFrom;
use reforge::facts::{
    Aspect, Facts, FactsError, FactsFound, Outcome, RigPart, RigUnreadable, Source,
    SubmeshesCopied, Visibility, Why,
};
use reforge::game::{Build, Game, GameError, GameSource, GameView, MAX_CHAIN, SkinClass};
use reforge::history::NoHistory;
use reforge::model::entity::FileId;
use reforge::model::{Slot, WadHash, WadName, name_hash, path_hash};
use reforge_testkit::bins::{Skin, bin, blob, entry, graph, hiding, linked_bin, showing, text};
use reforge_testkit::facts::{archive, env, mod_skin, own_skin};
use reforge_testkit::skins::{self, BASE_MESH, BASE_SKEL, S3_CLIPS, S3_MESH, S3_SKEL};
use reforge_testkit::{BrokenHistory, FactRun, MemoryGame, MemoryHistory, rigs};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

const BODY: &[[f32; 3]] = &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
const CAPE: &[[f32; 3]] = &[
    [5.0, 5.0, 5.0],
    [6.0, 5.0, 5.0],
    [5.0, 6.0, 5.0],
    [5.0, 5.0, 6.0],
];
const HAT: &[[f32; 3]] = &[[9.0, 9.0, 9.0], [9.5, 9.0, 9.0]];

/// The game with meshes and skeletons that read: the base's body, and skin 3's body and cape
/// on a skeleton with one joint more.
fn game() -> MemoryGame {
    let mut game = skins::game();
    game.insert(BASE_MESH, rigs::mesh(&[("body", BODY)], 0));
    game.insert(S3_MESH, rigs::mesh(&[("body", BODY), ("cape", CAPE)], 0));
    game.insert(BASE_SKEL, rigs::skeleton(&["Root", "Spine"]));
    game.insert(S3_SKEL, rigs::skeleton(&["Root", "Spine", "Cape"]));
    for clip in S3_CLIPS {
        game.insert(clip, rigs::clip(&["Root", "Cape"]));
    }
    game
}

/// A skin0 mod on a mesh and a skeleton of its own, with the entry's field changed by `edit`.
fn own_model(mesh: Vec<u8>, skeleton: Vec<u8>, hidden: Option<&str>) -> Vec<(String, Vec<u8>)> {
    let skin = own_skin("a", GRAPH0, RESOURCES0);
    let mut object = entry(SKIN0, &skin);
    if let Some(hidden) = hidden {
        let block = object.properties.get_mut(&name_hash("skinMeshProperties"));
        let ltk_meta::PropertyValueEnum::Embedded(block) =
            block.expect("the entry has a mesh block")
        else {
            panic!("the mesh block is an embed");
        };
        block
            .0
            .properties
            .insert(name_hash("initialSubmeshToHide"), text(hidden));
    }
    own_model_of(mesh, skeleton, vec![object, graph(GRAPH0, S3_CLIPS)])
}

/// The files of a skin0 mod on a mesh and a skeleton of its own, whose bin holds `objects`.
fn own_model_of(
    mesh: Vec<u8>,
    skeleton: Vec<u8>,
    objects: Vec<ltk_meta::BinObject>,
) -> Vec<(String, Vec<u8>)> {
    vec![
        ("assets/mod/a/mesh.skn".to_string(), mesh),
        ("assets/mod/a/skeleton.skl".to_string(), skeleton),
        (
            "assets/mod/a/body.tex".to_string(),
            blob("assets/mod/a/body.tex"),
        ),
        (skins::skin_bin("test", 0), bin(objects)),
    ]
}

/// The file the mod ships at a path.
fn shipped(run: &FactRun<'_>, path: &str) -> FileId {
    let mut files = run.fix.fixed.files.iter();
    let found = files.find(|(_, file)| file.hash == path_hash(path));
    found
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("the mod ships no file at {path}"))
}

fn check(run: &FactRun<'_>, slot: u16, aspect: Aspect) -> (Outcome, String) {
    let unit = run.unit("test", 0);
    let facts = run
        .facts
        .judge(run.fix.fixed.units[unit].character, unit)
        .expect("the unit is judged")
        .at(Slot(slot));
    let check = facts.checks.iter().find(|check| check.aspect == aspect);
    let check =
        check.unwrap_or_else(|| panic!("no {aspect:?} check at skin{slot}\n{}", run.dump()));
    (check.fit.outcome, check.fit.why.to_string())
}

// #MOD-ARTIFACT-0024: visible.
#[test]
fn a_submesh_copied_from_a_premium_skin_blocks_the_base() {
    let env = env(game(), NoHistory);
    // The author's own hat, with skin 3's cape copied in. The clips make skin 3 a candidate.
    let mesh = rigs::mesh(&[("hat", HAT), ("cape", CAPE)], 0);
    let run = FactRun::of(
        &env,
        &archive(&own_model(mesh, rigs::skeleton(&["Root", "Hat"]), None)),
    );
    let rig = run
        .facts
        .rigs
        .of(run.unit("test", 0))
        .expect("the unit has a rig");
    assert!(rig.mesh.file.own && rig.skeleton.file.own, "{}", run.dump());
    let cape = &rig.mesh.submeshes[1];
    assert_eq!(
        (cape.vertices, cape.on_base, cape.copied_from()),
        (4, 0.0, Some(Slot(3)))
    );
    assert_eq!(cape.visibility, Visibility::Visible);
    assert_eq!(rig.mesh.submeshes[0].copied_from(), None);

    let (outcome, why) = check(&run, 0, Aspect::Submeshes);
    assert_eq!(outcome, Outcome::DoesNotFit);
    assert_eq!(
        why,
        "visible submeshes copied from a premium skin: \"cape\" (skin 3)"
    );
    // At skin 3 the cape is the slot's own.
    assert_eq!(check(&run, 3, Aspect::Submeshes).0, Outcome::Fits);

    let copied: Vec<SubmeshesCopied> = run.found();
    assert_eq!(copied.len(), 1);
    assert_eq!(copied[0].submeshes.len(), 1);
    assert_eq!(
        (
            copied[0].submeshes[0].name.raw(),
            copied[0].submeshes[0].from
        ),
        ("cape", Slot(3))
    );
}

// #MOD-ARTIFACT-0024: hidden and never shown.
#[test]
fn a_copied_submesh_hidden_and_never_shown_fits_with_a_fix() {
    let env = env(game(), NoHistory);
    let mesh = rigs::mesh(&[("hat", HAT), ("cape", CAPE)], 0);
    let files = own_model(mesh, rigs::skeleton(&["Root", "Hat"]), Some("Cape"));
    let run = FactRun::of(&env, &archive(&files));
    let (outcome, why) = check(&run, 0, Aspect::Submeshes);
    assert_eq!(outcome, Outcome::FitsWithFix, "{}", run.dump());
    assert_eq!(
        why,
        "remove never-shown copied submeshes: \"cape\" (skin 3)"
    );
    let rig = run
        .facts
        .rigs
        .of(run.unit("test", 0))
        .expect("the unit has a rig");
    let visibility: Vec<Visibility> = rig.mesh.submeshes.iter().map(|s| s.visibility).collect();
    assert_eq!(visibility, [Visibility::Visible, Visibility::HiddenNever]);
}

// #MOD-ARTIFACT-0024: hidden and shown.
#[test]
fn a_hidden_submesh_that_an_event_of_the_graph_shows_is_told_from_one_never_shown() {
    let env = env(game(), NoHistory);
    let mesh = rigs::mesh(&[("hat", HAT), ("cape", CAPE)], 0);
    // The entry keeps skin 3's name, which makes skin 3 a candidate: the graph is the mod's
    // own once it holds the event.
    let skin = Skin {
        name: "TestSkin03".into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    let objects = vec![
        hiding(entry(SKIN0, &skin), "Hat Cape"),
        showing(graph(GRAPH0, S3_CLIPS), &["CAPE"]),
    ];
    let files = own_model_of(mesh, rigs::skeleton(&["Root", "Hat"]), objects);
    let run = FactRun::of(&env, &archive(&files));
    let rig = run
        .facts
        .rigs
        .of(run.unit("test", 0))
        .expect("the unit has a rig");
    let visibility: Vec<Visibility> = rig.mesh.submeshes.iter().map(|s| s.visibility).collect();
    assert_eq!(
        visibility,
        [Visibility::HiddenNever, Visibility::HiddenShown],
        "{}",
        run.dump()
    );
    // The hat is the author's: hidden or not, it is no copy.
    let copied: Vec<SubmeshesCopied> = run.found();
    assert_eq!(copied.len(), 1);
    let found = |submesh: &reforge::facts::CopiedSubmesh| {
        (submesh.name.raw().to_string(), submesh.visibility)
    };
    let found: Vec<_> = copied[0].submeshes.iter().map(found).collect();
    assert_eq!(found, [("cape".to_string(), Visibility::HiddenShown)]);
}

#[test]
fn a_file_the_mod_ships_is_its_own_unless_its_bytes_are_the_games() {
    let env = env(game(), NoHistory);
    // The author's mesh, and the base's skeleton byte for byte under the mod's path.
    let mesh = rigs::mesh(&[("hat", HAT)], 0);
    let files = own_model(mesh, rigs::skeleton(&["Root", "Spine"]), None);
    let run = FactRun::of(&env, &archive(&files));
    let rig = run
        .facts
        .rigs
        .of(run.unit("test", 0))
        .expect("the unit has a rig");
    let mesh = &rig.mesh.file;
    assert_eq!(
        (mesh.source, mesh.own, mesh.game_slots.len()),
        (Source::Mod(shipped(&run, "assets/mod/a/mesh.skn")), true, 0),
        "{}",
        run.dump()
    );
    let skeleton = &rig.skeleton.file;
    assert_eq!(
        (skeleton.source, skeleton.own),
        (
            Source::Mod(shipped(&run, "assets/mod/a/skeleton.skl")),
            false
        )
    );
    let slots: Vec<u16> = skeleton.game_slots.iter().map(|slot| slot.0).collect();
    assert_eq!(slots, [0, 7]);
    assert!(mesh.source.is_shipped() && !Source::Game.is_shipped());
    // The graph is in the mod's bin, and the clips it plays are the game's files.
    assert_eq!(rig.graph, Some(GraphFrom::Archive));
    assert!(
        rig.clips
            .iter()
            .all(|clip| clip.file.source == Source::Game)
    );
}

#[test]
fn an_own_skeleton_binds_the_mesh_or_does_not_fit() {
    let env = env(game(), NoHistory);
    // Every vertex binds to the joint at index 4, and the skeleton has two.
    let mesh = rigs::mesh(&[("hat", HAT)], 4);
    let run = FactRun::of(
        &env,
        &archive(&own_model(mesh, rigs::skeleton(&["Root", "Hat"]), None)),
    );
    let rig = run
        .facts
        .rigs
        .of(run.unit("test", 0))
        .expect("the unit has a rig");
    assert_eq!(
        (rig.mesh.max_blend, rig.skeleton.influences),
        (Some(4), Some(2)),
        "{}",
        run.dump()
    );
    let (outcome, why) = check(&run, 0, Aspect::Skeleton);
    assert_eq!(
        (outcome, why.as_str()),
        (
            Outcome::DoesNotFit,
            "the mesh binds joints the skeleton does not have"
        )
    );

    // A copy of the base's skeleton under the mod's own path is the base's, by its bytes.
    let mesh = rigs::mesh(&[("hat", HAT)], 1);
    let run = FactRun::of(
        &env,
        &archive(&own_model(mesh, rigs::skeleton(&["Root", "Spine"]), None)),
    );
    assert_eq!(
        check(&run, 0, Aspect::Skeleton),
        (Outcome::Fits, "the slot's own skeleton".to_string())
    );
    let other = "skin 0/7's skeleton, other joints than skin 3's".to_string();
    assert_eq!(
        check(&run, 3, Aspect::Skeleton),
        (Outcome::DoesNotFit, other)
    );
}

#[test]
fn rig_files_that_do_not_read_are_reported_and_not_judged() {
    let env = env(game(), NoHistory);
    let files = own_model(blob("a mesh"), blob("a skeleton"), None);
    let run = FactRun::of(&env, &archive(&files));
    let unreadable: Vec<RigUnreadable> = run.found();
    assert_eq!(unreadable.len(), 1);
    assert_eq!(unreadable[0].parts, [RigPart::Mesh, RigPart::Skeleton]);
    assert_eq!(
        check(&run, 0, Aspect::Mesh).0,
        Outcome::Fits,
        "an own mesh, whatever it holds"
    );
    assert!(
        run.facts
            .rigs
            .of(run.unit("test", 0))
            .is_some_and(|rig| rig.mesh.submeshes.is_empty())
    );
}

// #MOD-ARTIFACT-0025: an own model on a premium skin's whole animation set.
#[test]
fn borrowed_clips_are_sorted_by_how_they_meet_the_skeleton() {
    let env = env(game(), NoHistory);
    let mesh = rigs::mesh(&[("hat", HAT)], 0);
    let run = FactRun::of(
        &env,
        &archive(&own_model(mesh, rigs::skeleton(&["Root", "Hat"]), None)),
    );
    let unit = run.unit("test", 0);
    let clips = run.facts.clips.of(unit).expect("the unit has clip facts");
    let at_base = &clips.at[&Slot(0)];
    assert_eq!(
        (at_base.total, at_base.fits, at_base.own),
        (2, 0, 0),
        "{}",
        run.dump()
    );
    assert!(at_base.own_rig && at_base.graph_found);
    let reused = &at_base.reused[0];
    assert_eq!(
        (reused.from, reused.clips, reused.same_joints),
        (Slot(3), 2, false)
    );
    // #GAME-BEHAVIOR-0005: each clip moves `Root`, which the skeleton has, and `Cape`, which
    // it lacks.
    assert_eq!(
        (reused.joints_fit, reused.joints_extra, reused.joints_off),
        (0, 2, 0)
    );
    assert_eq!(reused.extra_joints.get("Cape"), Some(&2));
    assert_eq!(at_base.mostly_from(), Some((Slot(3), 2)));
    // Skin 3 plays them itself.
    assert_eq!(clips.at[&Slot(3)].fits, 2);
    // A chroma counts as its parent, and is asked though it is no candidate.
    let judge = run
        .facts
        .judge(run.fix.fixed.units[unit].character, unit)
        .expect("the unit is judged");
    assert_eq!(judge.at(Slot(9)).clips.fits, 2);
    assert_eq!(
        judge.at(Slot(55)).checks[0].fit.why,
        Why::NoSuchSkin { slot: Slot(55) }
    );
    assert!(!judge.at(Slot(55)).game_has_bin);
}

// #MOD-ARTIFACT-0025: each kind of another skin's clip against the unit's skeleton.
#[test]
fn a_borrowed_clip_is_counted_by_where_its_tracks_fall() {
    let mut game = game();
    game.insert(S3_CLIPS[0], rigs::clip(&["Root"]));
    game.insert(S3_CLIPS[1], rigs::clip(&["Cape"]));
    let env = env(game, NoHistory);
    let mesh = rigs::mesh(&[("hat", HAT)], 0);
    let run = FactRun::of(
        &env,
        &archive(&own_model(mesh, rigs::skeleton(&["Root", "Hat"]), None)),
    );
    let clips = run.facts.clips.of(run.unit("test", 0));
    let at_base = &clips.expect("the unit has clip facts").at[&Slot(0)];
    let reused = &at_base.reused[0];
    assert_eq!((reused.from, reused.clips), (Slot(3), 2), "{}", run.dump());
    // #GAME-BEHAVIOR-0005: the clip on `Root` plays whole, and the one on `Cape` moves
    // nothing of the unit's model.
    assert_eq!(
        (
            reused.joints_fit,
            reused.joints_extra,
            reused.joints_off,
            reused.joints_unknown
        ),
        (1, 0, 1, 0)
    );
    assert!(reused.extra_joints.is_empty());
}

/// The mod's own model, whose graph plays an own clip that moves `Root`, one that moves only
/// a joint the skeleton lacks, one that does not read and one nothing holds.
fn own_clips() -> Vec<(String, Vec<u8>)> {
    let mesh = rigs::mesh(&[("hat", HAT)], 0);
    let clips = [
        "assets/mod/a/idle.anm",
        "assets/mod/a/tail.anm",
        "assets/mod/a/torn.anm",
        "assets/mod/a/absent.anm",
    ];
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        graph(GRAPH0, &clips),
    ];
    let mut files = own_model_of(mesh, rigs::skeleton(&["Root", "Hat"]), objects);
    files.push((clips[0].to_string(), rigs::clip(&["Root", "Tail"])));
    files.push((clips[1].to_string(), rigs::clip(&["Tail"])));
    files.push((clips[2].to_string(), blob(clips[2])));
    files
}

#[test]
fn the_rigs_a_snapshot_keeps_read_back_with_the_joints_of_each_clip_the_report_leaves_out() {
    let env = env(game(), NoHistory);
    let run = FactRun::of(&env, &archive(&own_clips()));
    let json = serde_json::to_string(&run.facts.found()).expect("the facts write");
    let found: FactsFound = serde_json::from_str(&json).expect("the facts read");
    let reopened = Facts::reopen(&run.fix.fixed, &env, found).expect("the game answers");
    let unit = run.unit("test", 0);
    let rig = run.facts.rigs.of(unit).expect("the unit has a rig");
    assert!(rig.clips.iter().any(|clip| clip.joints.is_some()));
    assert_eq!(reopened.rigs.of(unit), Some(rig));
}

// #MOD-ARTIFACT-0025: the mod's own clips.
#[test]
fn an_own_clip_fits_where_it_moves_a_joint_of_the_skeleton() {
    let env = env(game(), NoHistory);
    let run = FactRun::of(&env, &archive(&own_clips()));
    let facts = run.facts.clips.of(run.unit("test", 0));
    let at_base = &facts.expect("the unit has clip facts").at[&Slot(0)];
    // #GAME-BEHAVIOR-0005: a track on `Root` is enough, and a clip with none on a joint of
    // the skeleton moves nothing. The clip that does not read and the one nothing holds are
    // not judged.
    assert_eq!(
        (
            at_base.total,
            at_base.fits,
            at_base.own_off,
            at_base.unknown
        ),
        (4, 1, 1, 2),
        "{}",
        run.dump()
    );
    assert_eq!(at_base.own, 3);
    assert!(at_base.reused.is_empty() && at_base.own_work());
}

const OLD_MESH: &str = "assets/characters/test/skins/skin03/test_skin03.skins_test_skin03.skn";
const OLD_SKEL: &str = "assets/characters/test/skins/skin03/test_skin03.skins_test_skin03.skl";

#[test]
fn a_former_path_is_the_games_file_where_it_moved() {
    let skin = Skin {
        mesh: OLD_MESH.into(),
        skeleton: OLD_SKEL.into(),
        ..skins::skin3()
    };
    let files = [mod_skin(
        "test",
        3,
        "Characters/Test/Skins/Skin3",
        &skin,
        vec![],
    )];

    // The history knows the paths as the game's: the files are skin 3's, where it holds them.
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]);
    let history = history
        .with_path(OLD_MESH, 0..=0)
        .with_path(OLD_SKEL, 0..=0);
    let env = env(game(), history);
    let run = FactRun::of(&env, &archive(&files));
    let rig = run
        .facts
        .rigs
        .of(run.unit("test", 3))
        .expect("the unit has a rig");
    assert_eq!(
        (rig.mesh.file.source, rig.mesh.file.moved),
        (Source::Game, Some(path_hash(S3_MESH))),
        "{}",
        run.dump()
    );
    assert!(rig.mesh.file.game_slots.contains(&Slot(3)));
    assert_eq!(rig.skeleton.file.moved, Some(path_hash(S3_SKEL)));
    assert_eq!(rig.skeleton.joints.as_ref().map(Vec::len), Some(3));
    assert_eq!(rig.graph, Some(GraphFrom::Game));

    // Without a history nothing says the paths were the game's.
    let env = reforge_testkit::facts::env(game(), NoHistory);
    let run = FactRun::of(&env, &archive(&files));
    let rig = run
        .facts
        .rigs
        .of(run.unit("test", 3))
        .expect("the unit has a rig");
    assert_eq!(
        (rig.mesh.file.source, rig.skeleton.file.source),
        (Source::Missing, Source::Missing)
    );
}

#[test]
fn the_catalog_holds_each_skin_with_its_class_and_its_objects() {
    let view = GameView::new(Box::new(game()));
    let catalog = view.catalog("Test").expect("the game reads");
    let slots: Vec<u16> = catalog.skins.keys().map(|slot| slot.0).collect();
    assert_eq!(slots, [0, 3, 7, 9]);
    assert_eq!(
        catalog.skin(Slot(0)).map(|skin| skin.class),
        Some(SkinClass::Base)
    );
    for slot in [3, 7] {
        assert_eq!(
            catalog.skin(Slot(slot)).map(|skin| skin.class),
            Some(SkinClass::Premium),
            "skin{slot}"
        );
    }
    assert_eq!(
        catalog.skin(Slot(9)).map(|skin| skin.class),
        Some(SkinClass::Chroma(Slot(3)))
    );
    assert_eq!(
        (catalog.canon(Slot(9)), catalog.canon(Slot(7))),
        (Slot(3), Slot(7))
    );
    assert_eq!(catalog.chromas_of(Slot(3)), [Slot(9)]);
    assert!(catalog.has_bin(Slot(7)) && !catalog.has_bin(Slot(1)));
    let skin3 = catalog.skin(Slot(3)).expect("skin 3");
    assert_eq!(skin3.mesh, Some(path_hash(S3_MESH)));
    assert_eq!(skin3.clips.len(), 2);
    assert_eq!(skin3.name.as_deref(), Some("TestSkin03"));
    // The chroma's graph is its parent's, found among the objects read before it.
    assert_eq!(catalog.skin(Slot(9)).map(|skin| skin.clips.len()), Some(2));
    assert!(
        catalog
            .object(name_hash("Characters/Test/Animations/Skin3"))
            .is_some()
    );
    let slots = catalog.slots_of(name_hash("Characters/Test/Skins/Skin3/Resources"));
    assert_eq!(slots.map(|slots| slots.len()), Some(1));
    // A character the game has no skin of has an empty catalog.
    assert!(
        view.catalog("nobody")
            .expect("the game reads")
            .skins
            .is_empty()
    );
}

#[test]
fn a_skins_bins_are_read_breadth_first_up_to_the_cap() {
    let mut game = skins::game();
    let linked: Vec<String> = (0..MAX_CHAIN + 4)
        .map(|i| format!("data/characters/test/shared/part{i}.bin"))
        .collect();
    let links: Vec<&str> = linked.iter().map(String::as_str).collect();
    let skin = linked_bin(vec![entry(SKIN0, &skins::base_skin())], &links);
    game.insert(&skins::skin_bin("test", 0), skin);
    for (i, path) in linked.iter().enumerate() {
        game.insert(
            path,
            bin(vec![reforge_testkit::bins::resolver(&format!(
                "Shared/Part{i}"
            ))]),
        );
    }
    let view = GameView::new(Box::new(game));
    let catalog = view.catalog("test").expect("the game reads");
    assert_eq!(
        catalog.skin(Slot(0)).map(|skin| skin.chain.len()),
        Some(MAX_CHAIN)
    );
    assert!(catalog.object(name_hash("Shared/Part0")).is_some());
    assert!(
        catalog
            .object(name_hash(&format!("Shared/Part{}", MAX_CHAIN)))
            .is_none()
    );
    // The fix stage raises `catalog.capped` for it, as the first to read the catalog.
    assert_eq!(
        catalog.capped().iter().copied().collect::<Vec<_>>(),
        [Slot(0)]
    );
}

#[test]
fn a_skin_whose_bins_fit_the_cap_is_not_noted() {
    let mut game = skins::game();
    let linked: Vec<String> = (0..MAX_CHAIN - 1)
        .map(|i| format!("data/characters/test/shared/part{i}.bin"))
        .collect();
    let links: Vec<&str> = linked.iter().map(String::as_str).collect();
    let skin = linked_bin(vec![entry(SKIN0, &skins::base_skin())], &links);
    game.insert(&skins::skin_bin("test", 0), skin);
    for path in &linked {
        game.insert(path, bin(vec![]));
    }
    let view = GameView::new(Box::new(game));
    let catalog = view.catalog("test").expect("the game reads");
    assert_eq!(
        catalog.skin(Slot(0)).map(|skin| skin.chain.len()),
        Some(MAX_CHAIN)
    );
    assert!(catalog.capped().is_empty());
}

#[test]
fn the_game_answers_what_a_rig_file_holds() {
    let view = GameView::new(Box::new(game()));
    let skeleton = view
        .skeleton(path_hash(S3_SKEL))
        .expect("the game reads")
        .expect("a skeleton");
    assert_eq!(skeleton.joints, ["Root", "Spine", "Cape"]);
    assert_eq!(skeleton.influences, 3);
    let shape = view
        .mesh_shape(path_hash(S3_MESH))
        .expect("the game reads")
        .expect("a mesh");
    let names: Vec<&str> = shape
        .submeshes
        .iter()
        .map(|submesh| submesh.name.as_str())
        .collect();
    assert_eq!(names, ["body", "cape"]);
    assert_eq!((shape.all.len(), shape.max_blend), (7, Some(0)));
    assert!(
        shape.all.contains(&(80, 80, 96)),
        "positions are rounded to 1/16 unit"
    );
    let joints = view
        .clip_joints(path_hash(S3_CLIPS[0]))
        .expect("the game reads")
        .expect("a clip");
    assert_eq!(joints.len(), 2);
    // A file that is no rig file, and a path the game does not hold, answer nothing.
    assert!(
        view.skeleton(path_hash(skins::BASE_TEX))
            .expect("the game reads")
            .is_none()
    );
    assert!(
        view.mesh_shape(path_hash("assets/none.skn"))
            .expect("the game reads")
            .is_none()
    );
}

/// A game one of whose chunks is listed and does not read.
struct Broken(MemoryGame, WadHash);

impl Game for Broken {
    fn build(&self) -> Option<&Build> {
        self.0.build()
    }
    fn describe(&self) -> String {
        self.0.describe()
    }
    fn size(&self, path: WadHash) -> Option<u64> {
        self.0.size(path)
    }
    fn read(&self, path: WadHash) -> Result<Option<Vec<u8>>, GameError> {
        if path == self.1 {
            return Err(GameError::Open(std::io::Error::other("the disk failed")));
        }
        self.0.read(path)
    }
    fn wads(&self, path: WadHash) -> Vec<WadName> {
        self.0.wads(path)
    }
}

#[test]
fn a_game_chunk_that_does_not_read_is_a_fault() {
    let view = GameView::new(Box::new(Broken(game(), path_hash(S3_SKEL))));
    assert!(view.skeleton(path_hash(S3_SKEL)).is_err());
    let view = GameView::new(Box::new(Broken(
        game(),
        path_hash(&skins::skin_bin("test", 7)),
    )));
    assert!(view.catalog("test").is_err());

    // The facts of a mod judged against it end with the fault, and no fact.
    let broken = Broken(game(), path_hash(S3_SKEL));
    let sizes = broken.0.sizes();
    let env = reforge::env::Env::builder()
        .game(broken)
        .sizes(sizes)
        .build()
        .expect("a game was given");
    let files = [mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    let fix = reforge_testkit::FixRun::of(
        &env,
        &reforge::fixups::FixupSet::standard(),
        &archive(&files),
    );
    let speller = fix.speller();
    let provenance =
        reforge::load::FileProvenance::find(&fix.fixed, &reforge::load::against(&env, &speller))
            .expect("no file is the skeleton's size");
    let inputs = reforge::facts::FactInputs {
        env: &env,
        dating: &fix.dating,
        files: &provenance,
        names: &speller,
        reworks: &reforge::load::ReworkTable::standard(),
    };
    let gathered = reforge::facts::Facts::gather(&fix.fixed, &inputs);
    assert!(matches!(gathered, Err(FactsError::Game(_))));
}

#[test]
fn a_history_that_does_not_answer_is_a_fault() {
    // The mod is loaded and fixed against a history that answers, and its facts are gathered
    // against one that does not.
    let builds = ["16.18.1", "16.19.1"];
    let answering = env(game(), MemoryHistory::new(&builds));
    let files = [mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    let fix = reforge_testkit::FixRun::of(
        &answering,
        &reforge::fixups::FixupSet::standard(),
        &archive(&files),
    );
    let speller = fix.speller();
    let against = reforge::load::against(&answering, &speller);
    let provenance = reforge::load::FileProvenance::find(&fix.fixed, &against);
    let provenance = provenance.expect("the history answers");
    let broken = env(game(), BrokenHistory(MemoryHistory::new(&builds)));
    let inputs = reforge::facts::FactInputs {
        env: &broken,
        dating: &fix.dating,
        files: &provenance,
        names: &speller,
        reworks: &reforge::load::ReworkTable::standard(),
    };
    let gathered = reforge::facts::Facts::gather(&fix.fixed, &inputs);
    assert!(matches!(gathered, Err(FactsError::History(_))));
}
