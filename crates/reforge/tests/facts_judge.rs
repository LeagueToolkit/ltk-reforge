//! The checks of a unit at a slot (#ADR-0013): each reason its mesh, the submeshes of an own
//! mesh and its skeleton fit the slot's rig, do not fit it, or are not judged. The game is
//! `reforge_testkit::skins`, with rig files that read.

use ltk_meta::BinObject;
use reforge::facts::{Aspect, Copied, Fit, Outcome, RigCheck, Why};
use reforge::history::NoHistory;
use reforge::model::{Slot, Spelled};
use reforge_testkit::bins::{Skin, bin, blob, entry, graph, hiding, particle, showing};
use reforge_testkit::facts::{env, own_skin};
use reforge_testkit::skins::{
    self, BASE_MESH, BASE_SKEL, PET_MESH, PET_SKEL, S3_CLIPS, S3_MESH, S3_SKEL, WAD,
};
use reforge_testkit::{FactRun, MemoryGame, ModArchive, rigs};

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
const WINGS: &[[f32; 3]] = &[[7.0, 7.0, 7.0], [8.0, 7.0, 7.0], [7.0, 8.0, 7.0]];
const HAT: &[[f32; 3]] = &[[9.0, 9.0, 9.0], [9.5, 9.0, 9.0]];

/// The game with meshes and skeletons that read: the base's body, skin 3's body, cape and
/// wings on a skeleton with one joint more, and the sub-character's skeleton.
fn game() -> MemoryGame {
    let mut game = skins::game();
    game.insert(BASE_MESH, rigs::mesh(&[("body", BODY)], 0));
    let skin3 = [("body", BODY), ("cape", CAPE), ("wings", WINGS)];
    game.insert(S3_MESH, rigs::mesh(&skin3, 0));
    game.insert(BASE_SKEL, rigs::skeleton(&["Root", "Spine"]));
    game.insert(S3_SKEL, rigs::skeleton(&["Root", "Spine", "Cape"]));
    game.insert(PET_SKEL, rigs::skeleton(&["Root", "Tail"]));
    game
}

/// A skin0 mod whose bin holds these objects and nothing else.
fn skin0(objects: Vec<BinObject>) -> ModArchive {
    ModArchive::new().file(WAD, &skins::skin_bin("test", 0), bin(objects))
}

/// A skin0 mod on a mesh and a skeleton of its own. Its entry hides the submeshes of `hidden`,
/// and its graph shows those of `shown`.
fn own_rig(mesh: Vec<u8>, skeleton: Vec<u8>, hidden: Option<&str>, shown: &[&str]) -> ModArchive {
    // The entry keeps skin 3's name: skin 3 is the unit's origin, and so a skin its submeshes
    // are compared with.
    let skin = Skin {
        name: "TestSkin03".into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    let mut object = entry(SKIN0, &skin);
    if let Some(hidden) = hidden {
        object = hiding(object, hidden);
    }
    let graph = showing(graph(GRAPH0, S3_CLIPS), shown);
    let files = [
        (skin.mesh.as_str(), mesh),
        (skin.skeleton.as_str(), skeleton),
        (skin.texture.as_str(), blob(&skin.texture)),
    ];
    skin0(vec![object, graph]).files(WAD, &files)
}

/// The author's own hat on a skeleton of two joints, which the hat binds to.
fn own_hat() -> ModArchive {
    let mesh = rigs::mesh(&[("hat", HAT)], 0);
    own_rig(mesh, rigs::skeleton(&["Root", "Hat"]), None, &[])
}

/// The checks of the mod's unit at a slot.
fn checks(run: &FactRun<'_>, slot: u16) -> Vec<RigCheck> {
    let unit = run.unit("test", 0);
    let judge = run.facts.judge(run.fix.fixed.units[unit].character, unit);
    let judge = judge.expect("the game has skins of the unit's character");
    judge.at(Slot(slot)).checks
}

/// The one check of an aspect of the mod's unit at a slot.
fn check(run: &FactRun<'_>, slot: u16, aspect: Aspect) -> Fit {
    let checks = checks(run, slot);
    let check = checks.iter().find(|check| check.aspect == aspect);
    let check =
        check.unwrap_or_else(|| panic!("no {aspect:?} check at skin{slot}\n{}", run.dump()));
    check.fit.clone()
}

fn is(aspect: Aspect, outcome: Outcome, why: Why) -> RigCheck {
    RigCheck {
        aspect,
        fit: Fit { outcome, why },
    }
}

fn cape() -> Vec<Copied> {
    vec![Copied {
        name: Spelled::new("cape"),
        from: Slot(3),
    }]
}

#[test]
fn a_slot_the_game_has_no_skin_at_is_ruled_out_before_the_rig_is_looked_at() {
    let env = env(game(), NoHistory);
    let run = FactRun::of(&env, &own_hat());
    let no_skin = Why::NoSuchSkin { slot: Slot(55) };
    assert_eq!(
        checks(&run, 55),
        [is(Aspect::Slot, Outcome::DoesNotFit, no_skin)],
        "{}",
        run.dump()
    );
}

#[test]
fn a_unit_with_no_skin_entry_is_not_judged() {
    let env = env(game(), NoHistory);
    let glow = particle(
        "Characters/Test/Skins/Skin0/Particles/Glow",
        "assets/mod/a/glow.tex",
    );
    let run = FactRun::of(&env, &skin0(vec![glow]));
    let unknown = [is(Aspect::Rig, Outcome::Unknown, Why::NoEntry)];
    assert_eq!(checks(&run, 0), unknown, "{}", run.dump());
    assert!(
        !unknown[0].fit.blocks(),
        "what nothing judges rules no slot out"
    );
    // The same at a slot the game has no skin of: nothing of the unit is compared with it.
    assert_eq!(checks(&run, 55), unknown);
    let candidates = run.facts.candidates.get(run.unit("test", 0));
    let candidates = candidates.expect("a unit with no entry has its candidate slots");
    assert_eq!(candidates.slots[&Slot(0)].checks, unknown);
}

#[test]
fn the_games_own_files_fit_the_skins_that_use_them_and_no_other() {
    let env = env(game(), NoHistory);
    let run = FactRun::of(&env, &skin0(vec![entry(SKIN0, &skins::base_skin())]));
    // Skin 7 is on the base's mesh and skeleton. A mesh of the game's has no submesh check.
    for slot in [0, 7] {
        assert_eq!(
            checks(&run, slot),
            [
                is(Aspect::Mesh, Outcome::Fits, Why::SlotMesh),
                is(Aspect::Skeleton, Outcome::Fits, Why::SlotSkeleton),
            ],
            "at skin{slot}\n{}",
            run.dump()
        );
    }
    let of = vec![Slot(0), Slot(7)];
    let other_mesh = Why::OtherSkinMesh { of: of.clone() };
    let other_skeleton = Why::OtherSkinSkeleton { of, slot: Slot(3) };
    assert_eq!(
        checks(&run, 3),
        [
            is(Aspect::Mesh, Outcome::DoesNotFit, other_mesh),
            is(Aspect::Skeleton, Outcome::DoesNotFit, other_skeleton),
        ]
    );
}

#[test]
fn a_premium_skins_files_fit_it_and_its_chroma_and_not_the_base() {
    let env = env(game(), NoHistory);
    let run = FactRun::of(&env, &skin0(vec![entry(SKIN0, &skins::skin3())]));
    // The mesh names its skins with a chroma counted as its parent. The skeleton names each.
    let other_mesh = Why::OtherSkinMesh { of: vec![Slot(3)] };
    let other_skeleton = Why::OtherSkinSkeleton {
        of: vec![Slot(3), Slot(9)],
        slot: Slot(0),
    };
    assert_eq!(
        checks(&run, 0),
        [
            is(Aspect::Mesh, Outcome::DoesNotFit, other_mesh),
            is(Aspect::Skeleton, Outcome::DoesNotFit, other_skeleton),
        ],
        "{}",
        run.dump()
    );
    for slot in [3, 9] {
        assert_eq!(
            checks(&run, slot),
            [
                is(Aspect::Mesh, Outcome::Fits, Why::SlotMesh),
                is(Aspect::Skeleton, Outcome::Fits, Why::SlotSkeleton),
            ],
            "at skin{slot}"
        );
    }
}

#[test]
fn files_nothing_holds_are_not_judged() {
    let env = env(game(), NoHistory);
    let skin = Skin {
        mesh: "assets/mod/a/none.skn".into(),
        skeleton: "assets/mod/a/none.skl".into(),
        ..skins::base_skin()
    };
    let run = FactRun::of(&env, &skin0(vec![entry(SKIN0, &skin)]));
    assert_eq!(
        checks(&run, 0),
        [
            is(Aspect::Mesh, Outcome::Unknown, Why::NoMesh),
            is(Aspect::Skeleton, Outcome::Unknown, Why::NoSkeleton),
        ],
        "{}",
        run.dump()
    );
}

#[test]
fn a_game_file_of_no_skin_of_the_character_is_not_judged() {
    let env = env(game(), NoHistory);
    let skin = Skin {
        mesh: PET_MESH.into(),
        skeleton: PET_SKEL.into(),
        ..skins::base_skin()
    };
    let run = FactRun::of(&env, &skin0(vec![entry(SKIN0, &skin)]));
    assert_eq!(
        checks(&run, 0),
        [
            is(Aspect::Mesh, Outcome::Unknown, Why::ForeignMesh),
            is(Aspect::Skeleton, Outcome::Unknown, Why::ForeignSkeleton),
        ],
        "{}",
        run.dump()
    );
}

#[test]
fn a_rig_of_the_mods_own_fits_any_slot() {
    let env = env(game(), NoHistory);
    let run = FactRun::of(&env, &own_hat());
    for slot in [0, 3] {
        assert_eq!(
            checks(&run, slot),
            [
                is(Aspect::Mesh, Outcome::Fits, Why::OwnMesh),
                is(Aspect::Submeshes, Outcome::Fits, Why::NoCopiedSubmesh),
                is(Aspect::Skeleton, Outcome::Fits, Why::OwnSkeleton),
            ],
            "at skin{slot}\n{}",
            run.dump()
        );
    }
}

#[test]
fn an_own_skeleton_of_the_slots_joints_is_taken_as_the_slots() {
    let env = env(game(), NoHistory);
    // The base's joints in the base's order, in a file of other bytes than the base's.
    let skeleton = rigs::skeleton_named("mine", &["Root", "Spine"]);
    let mesh = rigs::mesh(&[("hat", HAT)], 0);
    let run = FactRun::of(&env, &own_rig(mesh, skeleton, None, &[]));
    assert_eq!(
        check(&run, 0, Aspect::Skeleton),
        Fit {
            outcome: Outcome::Fits,
            why: Why::SlotJoints
        },
        "{}",
        run.dump()
    );
    // Skin 3 has one joint more.
    assert_eq!(check(&run, 3, Aspect::Skeleton).why, Why::OwnSkeleton);
}

#[test]
fn a_premium_skins_skeleton_file_does_not_fit_the_base_though_it_has_the_bases_joints() {
    let mut game = game();
    // Skin 3's skeleton with the base's joints, in a file of other bytes than the base's.
    game.insert(S3_SKEL, rigs::skeleton_named("three", &["Root", "Spine"]));
    let env = env(game, NoHistory);
    let skin = Skin {
        skeleton: S3_SKEL.into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    let mesh = rigs::mesh(&[("hat", HAT)], 0);
    let files = [
        (skin.mesh.as_str(), mesh),
        (skin.texture.as_str(), blob(&skin.texture)),
    ];
    let run = FactRun::of(&env, &skin0(vec![entry(SKIN0, &skin)]).files(WAD, &files));
    let premium = Why::PremiumSkeleton {
        of: vec![Slot(3), Slot(9)],
    };
    assert_eq!(
        check(&run, 0, Aspect::Skeleton),
        Fit {
            outcome: Outcome::DoesNotFit,
            why: premium
        },
        "{}",
        run.dump()
    );
    assert_eq!(check(&run, 3, Aspect::Skeleton).why, Why::SlotSkeleton);
}

#[test]
fn an_own_skeleton_without_a_joint_the_mesh_binds_does_not_fit() {
    let env = env(game(), NoHistory);
    // Every vertex binds to the joint at index 4, and the skeleton has two.
    let mesh = rigs::mesh(&[("hat", HAT)], 4);
    let skeleton = rigs::skeleton(&["Root", "Hat"]);
    let run = FactRun::of(&env, &own_rig(mesh, skeleton, None, &[]));
    for slot in [0, 3] {
        assert_eq!(
            check(&run, slot, Aspect::Skeleton),
            Fit {
                outcome: Outcome::DoesNotFit,
                why: Why::Unbound
            },
            "at skin{slot}\n{}",
            run.dump()
        );
    }
}

// #MOD-ARTIFACT-0024: visible.
#[test]
fn a_visible_submesh_of_a_premium_skin_fits_that_skin_alone() {
    let env = env(game(), NoHistory);
    // Skin 3's cape and wings beside the author's hat. The wings are hidden, and are left out
    // of the reason: the visible ones decide.
    let mesh = rigs::mesh(&[("hat", HAT), ("cape", CAPE), ("wings", WINGS)], 0);
    let skeleton = rigs::skeleton(&["Root", "Hat"]);
    let run = FactRun::of(&env, &own_rig(mesh, skeleton, Some("Wings"), &[]));
    assert_eq!(
        check(&run, 0, Aspect::Submeshes),
        Fit {
            outcome: Outcome::DoesNotFit,
            why: Why::CopiedVisible { submeshes: cape() }
        },
        "{}",
        run.dump()
    );
    assert_eq!(
        check(&run, 3, Aspect::Submeshes),
        Fit {
            outcome: Outcome::Fits,
            why: Why::NoCopiedSubmesh
        },
        "at skin 3 the cape and the wings are the slot's own"
    );
}

// #MOD-ARTIFACT-0024: hidden and shown.
#[test]
fn a_hidden_submesh_of_a_premium_skin_that_an_event_shows_does_not_fit() {
    let env = env(game(), NoHistory);
    let mesh = rigs::mesh(&[("hat", HAT), ("cape", CAPE), ("wings", WINGS)], 0);
    let skeleton = rigs::skeleton(&["Root", "Hat"]);
    let archive = own_rig(mesh, skeleton, Some("Cape Wings"), &["Cape"]);
    let run = FactRun::of(&env, &archive);
    assert_eq!(
        check(&run, 0, Aspect::Submeshes),
        Fit {
            outcome: Outcome::DoesNotFit,
            why: Why::CopiedShown { submeshes: cape() }
        },
        "{}",
        run.dump()
    );
}

// #MOD-ARTIFACT-0024: hidden and never shown.
#[test]
fn a_hidden_submesh_of_a_premium_skin_that_nothing_shows_fits_once_removed() {
    let env = env(game(), NoHistory);
    let mesh = rigs::mesh(&[("hat", HAT), ("cape", CAPE)], 0);
    let skeleton = rigs::skeleton(&["Root", "Hat"]);
    // The graph shows the hat, which is no copy.
    let run = FactRun::of(&env, &own_rig(mesh, skeleton, Some("Cape"), &["Hat"]));
    let fit = check(&run, 0, Aspect::Submeshes);
    assert_eq!(
        fit,
        Fit {
            outcome: Outcome::FitsWithFix,
            why: Why::CopiedHidden { submeshes: cape() }
        },
        "{}",
        run.dump()
    );
    assert!(!fit.blocks());
}
