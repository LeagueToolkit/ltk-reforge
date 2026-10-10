//! The decision over synthetic mods: the skin0-first rule with its premium fallback, a paste
//! on the base's rig staying on skin0, a mod for a premium slot staying there, forced targets,
//! sub-characters, two units on one slot, the levels of animation reuse, reworks and targets
//! given to one unit. The game is `reforge_testkit::skins`.

use std::collections::BTreeMap;

use reforge::diag::{Diag, Diagnostics, Stage};
use reforge::facts::{Aspect, ClipBlock, ClipsAt, Made, Outcome, Reused, Verdict, Why};
use reforge::history::NoHistory;
use reforge::load::ReworkTable;
use reforge::model::Slot;
use reforge::port::{
    AnimReuse, Considered, ForcedUnfit, Infeasible, Policy, PreRework, Reason, SubLeft, Superseded,
    TargetChosen, TargetConflict, TargetPolicy, UnitTarget, Unjudged, clip_fit,
};
use reforge_testkit::bins::{self, Skin, blob, graph, resolver};
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::plans::{
    NEW_TEX, OLD_TEX, champion_and_pet, champion_and_unbound_pet, old_model_and_pet,
    paste_and_own_skin3, paste_and_two_pets, reworked,
};
use reforge_testkit::skins::{self, BASE_CLIPS, S3_CLIPS, S3_SKEL};
use reforge_testkit::{FactRun, rigs};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

fn policy(target: TargetPolicy, anim_reuse: AnimReuse) -> Policy {
    Policy { target, anim_reuse }
}

fn block() -> Policy {
    policy(TargetPolicy::Auto, AnimReuse::Block)
}

fn codes(run: &FactRun<'_>, policy: Policy) -> Vec<&'static str> {
    run.decided(policy).iter().map(|d| d.code.name).collect()
}

#[test]
fn a_paste_on_the_premium_mesh_falls_back_to_its_skin() {
    let env = env(skins::game(), NoHistory);
    let files = [mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let decision = run.decide(block(), "test", 0);
    assert_eq!(decision.origin, Verdict::Premium(Slot(3)), "{}", run.dump());
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(3)), Reason::Fallback)
    );
    let weighed = decision.candidates.iter().map(|c| (c.why, c.feasible));
    let weighed: Vec<_> = weighed.collect();
    assert_eq!(
        weighed,
        [
            (Considered::BaseFirst, false),
            (Considered::PremiumOrigin, true)
        ]
    );
    assert_eq!(decision.chroma_followers, [Slot(9)]);
    let log = run.decided(block());
    let chosen: Vec<&TargetChosen> = log.of().collect();
    assert_eq!(chosen.len(), 1);
    assert_eq!(
        (chosen[0].slot, chosen[0].reason),
        (Slot(3), Reason::Fallback)
    );
}

#[test]
fn a_paste_of_a_skin_on_the_base_rig_stays_on_skin0() {
    let env = env(skins::game(), NoHistory);
    let files = [mod_skin("test", 0, SKIN0, &skins::skin7(), vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let decision = run.decide(block(), "test", 0);
    assert_eq!(decision.origin, Verdict::Premium(Slot(7)), "{}", run.dump());
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(0)), Reason::PasteCleaned)
    );
    assert!(decision.base_rig_origin);
}

#[test]
fn the_mods_own_rig_goes_to_skin0() {
    let env = env(skins::game(), NoHistory);
    let mut files = own_files("a");
    let skin = own_skin("a", GRAPH0, RESOURCES0);
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![own_graph("a", GRAPH0)],
    ));
    let run = FactRun::of(&env, &archive(&files));
    let decision = run.decide(block(), "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(0)), Reason::Base),
        "{}",
        run.dump()
    );
    assert!(
        matches!(decision.origin, Verdict::Custom | Verdict::Base),
        "{:?}",
        decision.origin
    );
}

#[test]
fn an_own_mesh_on_a_premium_skeleton_falls_back() {
    let env = env(skins::game(), NoHistory);
    let mut files = own_files("a");
    let skin = Skin {
        skeleton: S3_SKEL.into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![own_graph("a", GRAPH0)],
    ));
    let run = FactRun::of(&env, &archive(&files));
    let decision = run.decide(block(), "test", 0);
    assert_eq!(decision.origin, Verdict::Premium(Slot(3)), "{}", run.dump());
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(3)), Reason::Fallback)
    );
}

#[test]
fn an_own_mesh_on_a_premium_skeleton_file_of_the_bases_joints_goes_to_that_skin() {
    let mut game = skins::game();
    // Skin 3's skeleton with the base's joints, in a file of other bytes than the base's.
    game.insert(skins::BASE_SKEL, rigs::skeleton(&["Root", "Spine"]));
    game.insert(S3_SKEL, rigs::skeleton_named("three", &["Root", "Spine"]));
    // The base's resolver holds a key no other skin's does.
    let idle = bins::particle(skins::BASE_PARTICLE, skins::BASE_PARTICLE_TEX);
    let base_bin = vec![
        bins::entry(SKIN0, &skins::base_skin()),
        graph(GRAPH0, BASE_CLIPS),
        bins::resolver_with(RESOURCES0, &[("Idle", skins::BASE_PARTICLE)]),
        idle.clone(),
    ];
    game.insert(&skins::skin_bin("test", 0), bins::bin(base_bin));
    let env = env(game, NoHistory);
    let mut files = own_files("a");
    let skin = Skin {
        skeleton: S3_SKEL.into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    // The base's resolver with a key added, and the base's particle: beside skin 3's
    // skeleton, the unit derives from the base.
    let keys = [
        ("Idle", skins::BASE_PARTICLE),
        ("Glow", skins::BASE_PARTICLE),
    ];
    let objects = vec![
        own_graph("a", GRAPH0),
        bins::resolver_with(RESOURCES0, &keys),
        idle,
    ];
    files.push(mod_skin("test", 0, SKIN0, &skin, objects));
    let run = FactRun::of(&env, &archive(&files));
    let decision = run.decide(block(), "test", 0);
    assert_eq!(decision.premium, None, "{}", run.dump());
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(3)), Reason::SkeletonOrigin),
        "{}",
        run.dump()
    );
    let weighed = decision.candidates.iter().map(|c| (c.why, c.feasible));
    let weighed: Vec<_> = weighed.collect();
    assert_eq!(
        weighed,
        [
            (Considered::BaseFirst, false),
            (Considered::SkeletonSkin, true)
        ]
    );
    let skeleton = &decision.candidates[0].checks;
    let skeleton = skeleton
        .iter()
        .find(|check| check.aspect == Aspect::Skeleton);
    let why = skeleton.map(|check| check.fit.why.clone());
    assert!(matches!(why, Some(Why::PremiumSkeleton { .. })), "{why:?}");
}

#[test]
fn a_mod_only_for_a_premium_skin_stays_there() {
    let env = env(skins::game(), NoHistory);
    let mut files = own_files("a");
    let skin = own_skin(
        "a",
        "Characters/Test/Animations/Skin3",
        "Characters/Test/Skins/Skin3/Resources",
    );
    let extra = vec![own_graph("a", "Characters/Test/Animations/Skin3")];
    files.push(mod_skin(
        "test",
        3,
        "Characters/Test/Skins/Skin3",
        &skin,
        extra,
    ));
    let run = FactRun::of(&env, &archive(&files));
    let decision = run.decide(block(), "test", 3);
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(3)), Reason::Slot),
        "{}",
        run.dump()
    );
    assert!(decision.candidates.is_empty(), "no slot is weighed");
}

#[test]
fn a_forced_target_is_honoured_and_flagged() {
    let env = env(skins::game(), NoHistory);
    let files = [mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let forced = policy(TargetPolicy::Base, AnimReuse::Block);
    let decision = run.decide(forced, "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(0)), Reason::Forced)
    );
    assert!(decision.unfit);
    assert_eq!(decision.candidates[0].why, Considered::Policy);
    let log = run.decided(forced);
    let unfit: Vec<&ForcedUnfit> = log.of().collect();
    assert_eq!(unfit.len(), 1);
    assert!(!unfit[0].follows);
    assert_eq!(log.of::<TargetChosen>().count(), 0);

    // A slot the game has no skin at is asked of the facts all the same.
    let nowhere = policy(TargetPolicy::Fixed(Slot(55)), AnimReuse::Allow);
    let decision = run.decide(nowhere, "test", 0);
    assert_eq!(decision.chosen, Some(Slot(55)));
    assert!(decision.unfit, "the game has no skin 55");
}

#[test]
fn under_the_origin_policy_a_unit_goes_to_its_premium_origin_else_to_skin0() {
    let env = env(skins::game(), NoHistory);
    let origin = policy(TargetPolicy::Origin, AnimReuse::Block);
    let files = [mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    let paste = FactRun::of(&env, &archive(&files));
    let decision = paste.decide(origin, "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason, decision.unfit),
        (Some(Slot(3)), Reason::Forced, false),
        "{}",
        paste.dump()
    );
    assert_eq!(decision.candidates[0].why, Considered::Policy);
    // A forced target that fits is said by no finding.
    assert!(paste.decided(origin).is_empty());

    // The mod's own rig derives from no premium skin.
    let mut files = own_files("a");
    let skin = own_skin("a", GRAPH0, RESOURCES0);
    let extra = vec![own_graph("a", GRAPH0)];
    files.push(mod_skin("test", 0, SKIN0, &skin, extra));
    let own = FactRun::of(&env, &archive(&files));
    let decision = own.decide(origin, "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(0)), Reason::Forced),
        "{}",
        own.dump()
    );
}

#[test]
fn under_the_slot_policy_a_unit_stays_where_its_bin_sits() {
    let env = env(skins::game(), NoHistory);
    let files = [mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let stay = policy(TargetPolicy::Slot, AnimReuse::Block);
    let decision = run.decide(stay, "test", 0);
    // Under `auto` the paste falls back to skin 3: here it stays, and is flagged.
    assert_eq!(
        (decision.chosen, decision.reason, decision.unfit),
        (Some(Slot(0)), Reason::Forced, true),
        "{}",
        run.dump()
    );
    let log = run.decided(stay);
    let unfit: Vec<&ForcedUnfit> = log.of().collect();
    assert_eq!(unfit.len(), 1);
    assert_eq!((unfit[0].slot, unfit[0].follows), (Slot(0), false));
}

#[test]
fn a_sub_character_is_judged_by_the_joints_of_the_skin_it_follows_its_champion_to() {
    // The game ships the pet at skin 3 on another skeleton file with the joints of the skin0
    // file the mod's pet takes. Only the champion's unit is weighed at skin 3 by itself.
    let mut game = skins::game();
    let joints = ["Root", "Tail"];
    game.insert(skins::PET_SKEL, rigs::skeleton(&joints));
    let skeleton3 = "assets/characters/testpet/skins/skin03/testpet_skin03.skl";
    game.insert(skeleton3, rigs::skeleton(&joints));
    // A graph of its own: skin 3 plays none of the pet's clips.
    let pet3 = Skin {
        skeleton: skeleton3.into(),
        graph: "Characters/TestPet/Animations/Skin3".into(),
        ..skins::pet_skin()
    };
    let entry3 = bins::entry("Characters/TestPet/Skins/Skin3", &pet3);
    game.insert(&skins::skin_bin("testpet", 3), bins::bin(vec![entry3]));
    let env = env(game, NoHistory);
    let run = FactRun::of(&env, &archive(&champion_and_pet()));
    let pet = run.decide(block(), "testpet", 0);
    assert_eq!(
        (pet.chosen, pet.reason),
        (Some(Slot(3)), Reason::SubFollows),
        "{}",
        run.dump()
    );
    let checks = pet.candidates[0].checks.iter();
    let skeleton = checks
        .clone()
        .find(|check| check.aspect == Aspect::Skeleton);
    assert_eq!(skeleton.map(|check| &check.fit.why), Some(&Why::SlotJoints));
}

#[test]
fn a_sub_character_follows_its_champion() {
    let env = env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&champion_and_pet()));
    let champion = run.decide(block(), "test", 0);
    assert_eq!(champion.chosen, Some(Slot(3)), "{}", run.dump());
    let pet = run.decide(block(), "testpet", 0);
    // The game has no pet skin 3: the slot does not fit, and the pet follows all the same.
    assert_eq!(
        (pet.chosen, pet.reason),
        (Some(Slot(3)), Reason::SubFollows)
    );
    // #GAME-BEHAVIOR-0001: the client looks for the pet's skin 3 bin, which the game lacks.
    assert_eq!(pet.new_bin, Some(Slot(3)));
    assert!(pet.unfit);
    assert_eq!(pet.candidates[0].why, Considered::ChampionTarget);
    let log = run.decided(block());
    let unfit: Vec<&ForcedUnfit> = log.of().collect();
    assert!(unfit.len() == 1 && unfit[0].follows);
}

#[test]
fn a_sub_character_on_its_own_model_follows_to_the_base() {
    let env = env(skins::game(), NoHistory);
    let mut files = own_files("a");
    let skin = own_skin("a", GRAPH0, RESOURCES0);
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![own_graph("a", GRAPH0)],
    ));
    files.push(mod_skin(
        "testpet",
        0,
        "Characters/TestPet/Skins/Skin0",
        &skins::pet_skin(),
        vec![particle()],
    ));
    let run = FactRun::of(&env, &archive(&files));
    let pet = run.decide(block(), "testpet", 0);
    assert_eq!(
        (pet.chosen, pet.reason),
        (Some(Slot(0)), Reason::SubFollows),
        "{}",
        run.dump()
    );
    assert_eq!(pet.new_bin, None);
    assert!(
        codes(&run, block())
            .iter()
            .all(|code| *code == TargetChosen::CODE.name)
    );
}

#[test]
fn a_sub_character_whose_model_does_not_fit_the_base_is_left_to_the_game() {
    let env = env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&champion_and_unbound_pet()));
    let champion = run.decide(block(), "test", 0);
    assert_eq!(champion.chosen, Some(Slot(0)), "{}", run.dump());
    let pet = run.decide(block(), "testpet", 0);
    assert_eq!(
        (pet.chosen, pet.reason),
        (None, Reason::SubLeftToBase),
        "{}",
        run.dump()
    );
    let skeleton = pet.candidates[0].checks.iter();
    let skeleton: Vec<&Why> = skeleton
        .filter(|check| check.fit.blocks())
        .map(|check| &check.fit.why)
        .collect();
    assert_eq!(skeleton, [&Why::Unbound]);
    let log = run.decided(block());
    let left: Vec<&SubLeft> = log.of().collect();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].unit, pet.unit);
}

/// An object that makes a sub-character's bin differ from the game's.
fn particle() -> ltk_meta::BinObject {
    reforge_testkit::bins::particle(
        "Characters/TestPet/Skins/Skin0/Particles/Own",
        "assets/mod/pet/glow.tex",
    )
}

#[test]
fn a_unit_written_for_the_fallback_slot_supersedes_the_paste() {
    let env = env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&paste_and_own_skin3()));
    let targets = block().decide(&run.facts);
    let paste = run.decision(&targets, "test", 0);
    let written = run.decision(&targets, "test", 3);
    assert_eq!(
        (written.chosen, written.reason),
        (Some(Slot(3)), Reason::Slot),
        "{}",
        run.dump()
    );
    assert_eq!((paste.chosen, paste.reason), (None, Reason::Superseded));
    assert_eq!(paste.placed, (Some(Slot(3)), Reason::Fallback));
    assert_eq!(paste.taken_by, Some(written.unit));
    let log = run.decided(block());
    let superseded: Vec<&Superseded> = log.of().collect();
    assert_eq!(superseded.len(), 1);
    assert_eq!(
        (superseded[0].unit, superseded[0].by),
        (paste.unit, written.unit)
    );
    assert!(targets.conflicts.is_empty());
}

#[test]
fn two_units_of_one_rank_on_one_slot_leave_it_to_the_game() {
    // #ADR-0076: two pets on models of their own at slots 1 and 2 follow the champion's paste
    // of skin 3 there. Neither is written for skin 3, and neither is at skin0.
    let env = env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&paste_and_two_pets()));
    let targets = block().decide(&run.facts);
    let pets = [run.unit("testpet", 1), run.unit("testpet", 2)];
    for pet in pets {
        let decision = &targets.decisions[&pet];
        assert_eq!(decision.placed.0, Some(Slot(3)), "{}", run.dump());
        assert_eq!((decision.chosen, decision.reason), (None, Reason::Conflict));
    }
    assert_eq!(targets.target(run.unit("test", 0)), Some(Slot(3)));
    let mut log = Diagnostics::new();
    targets.report(&mut log.at(Stage::Decide));
    let conflicts: Vec<&TargetConflict> = log.of().collect();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        (conflicts[0].slot, &conflicts[0].units[..]),
        (Slot(3), &pets[..])
    );
}

#[test]
fn a_unit_in_a_conflict_raises_no_target_chosen() {
    // #ADR-0076: `target.conflict` says where the pets were placed, and no line of their own.
    let env = env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&paste_and_two_pets()));
    let log = run.decided(block());
    let chosen: Vec<&TargetChosen> = log.of().collect();
    let units: Vec<_> = chosen.iter().map(|chosen| chosen.unit).collect();
    assert_eq!(units, [run.unit("test", 0)], "{}", run.dump());
    assert_eq!(log.of::<TargetConflict>().count(), 1);
}

#[test]
fn a_unit_without_a_skin_entry_stays_unjudged() {
    let env = env(skins::game(), NoHistory);
    // The bin holds a resolver and no skin entry.
    let files = [(
        skins::skin_bin("test", 0),
        reforge_testkit::bins::bin(vec![resolver(RESOURCES0)]),
    )];
    let run = FactRun::of(&env, &archive(&files));
    let decision = run.decide(block(), "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(0)), Reason::Unjudged),
        "{}",
        run.dump()
    );
    assert_eq!(run.decided(block()).of::<Unjudged>().count(), 1);
}

/// A model of the mod's own that reads, with joints of its own, playing skin 3's game clips.
fn own_rig_on_skin3_clips(own_clips: &[&str]) -> Vec<(String, Vec<u8>)> {
    let joints = ["Root", "Spine", "Head"];
    let mut files = vec![
        (
            "assets/mod/a/mesh.skn".to_string(),
            rigs::mesh(&[("body", &[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0]])], 1),
        ),
        (
            "assets/mod/a/skeleton.skl".to_string(),
            rigs::skeleton(&joints),
        ),
        (
            "assets/mod/a/body.tex".to_string(),
            blob("assets/mod/a/body.tex"),
        ),
    ];
    let mut clips: Vec<&str> = S3_CLIPS.to_vec();
    for clip in own_clips {
        files.push(((*clip).to_string(), rigs::clip(&["Root", "Head"])));
        clips.push(clip);
    }
    let skin = own_skin("a", GRAPH0, RESOURCES0);
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![graph(GRAPH0, &clips)],
    ));
    files
}

/// The game with skin 3's clips as real clips: tracks on `Root` and on a joint of skin 3's.
fn game_with_clips() -> reforge_testkit::MemoryGame {
    let mut game = skins::game();
    for clip in S3_CLIPS {
        game.insert(clip, rigs::clip(&["Root", "Cape"]));
    }
    game
}

#[test]
fn the_level_of_animation_reuse_is_the_policys() {
    let env = env(game_with_clips(), NoHistory);
    // Two clips of skin 3's and one of its own: most of what it plays is another skin's.
    let run = FactRun::of(
        &env,
        &archive(&own_rig_on_skin3_clips(&["assets/mod/a/idle.anm"])),
    );
    let at = |level| run.decide(policy(TargetPolicy::Auto, level), "test", 0);
    let blocked = at(AnimReuse::Block);
    // The clips are skin 3's, and so the unit: its own rig fits there.
    assert_eq!(
        (blocked.chosen, blocked.reason),
        (Some(Slot(3)), Reason::ClipsOrigin),
        "{}",
        run.dump()
    );
    assert_eq!(blocked.played, Some(2));
    let weighed = blocked.candidates.iter().map(|c| (c.why, c.feasible));
    let weighed: Vec<_> = weighed.collect();
    assert_eq!(
        weighed,
        [
            (Considered::BaseFirst, false),
            (Considered::ClipsSkin, true)
        ]
    );
    let allowed = at(AnimReuse::Allow);
    assert_eq!(
        (allowed.chosen, allowed.reason),
        (Some(Slot(0)), Reason::Base)
    );
    let clips = allowed.candidates[0]
        .checks
        .last()
        .expect("the clips are judged");
    assert_eq!(
        (clips.aspect, clips.fit.outcome),
        (Aspect::Clips, Outcome::Fits)
    );
    assert_eq!(
        clips.fit.why.to_string(),
        "3 of 3 fit, 2 reused from other skins (allow)"
    );
    // Its own rig, and fewer than half its clips its own: `own-rig` refuses.
    let own_rig = at(AnimReuse::OwnRig);
    assert_eq!(own_rig.reason, Reason::ClipsOrigin);
    let clips = own_rig.candidates[0]
        .checks
        .last()
        .expect("the clips are judged");
    assert_eq!(
        clips.fit.why.to_string(),
        "2 clips of skin 3, which the slot does not play; reuse needs its own rig and half its \
         clips its own, or fewer than half borrowed: own mesh and skeleton, 1 own and 2 borrowed \
         of 3 clips"
    );
}

#[test]
fn own_rig_reuse_needs_half_the_clips_its_own() {
    let env = env(game_with_clips(), NoHistory);
    let own = ["assets/mod/a/idle.anm", "assets/mod/a/dance.anm"];
    let run = FactRun::of(&env, &archive(&own_rig_on_skin3_clips(&own)));
    let decision = run.decide(Policy::default(), "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(0)), Reason::Base),
        "{}",
        run.dump()
    );
    let clips = decision.candidates[0]
        .checks
        .last()
        .expect("the clips are judged");
    assert_eq!(
        clips.fit.why.to_string(),
        "4 of 4 fit, 2 reused from other skins (own-rig)"
    );
    // The alternatives are the other levels, each decided from the same facts.
    let targets = Policy::default()
        .decide(&run.facts)
        .compared(&run.facts, &Policy::default().other_levels());
    let others = &targets.alternatives[&decision.unit];
    assert_eq!(others.len(), 3);
    assert_eq!(others[0].policy.anim_reuse, AnimReuse::Block);
    assert_eq!(
        (others[0].chosen, others[0].reason),
        (Some(Slot(0)), Reason::Infeasible)
    );
    assert_eq!(run.decided(block()).of::<Infeasible>().count(), 1);
}

#[test]
fn a_unit_borrowing_one_clip_stays_at_its_slot() {
    // The base's rig and the base's two clips, and one of skin 3's: fewer than half.
    let clips: Vec<&str> = BASE_CLIPS.iter().copied().chain([S3_CLIPS[1]]).collect();
    let mut game = game_with_clips();
    game.insert(skins::BASE_SKEL, rigs::skeleton(&["Root", "Spine"]));
    let env = env(game, NoHistory);
    let files = [mod_skin(
        "test",
        0,
        SKIN0,
        &skins::base_skin(),
        vec![graph(GRAPH0, &clips)],
    )];
    let run = FactRun::of(&env, &archive(&files));
    let decision = run.decide(Policy::default(), "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason),
        (Some(Slot(0)), Reason::Base),
        "{}",
        run.dump()
    );
    let blocked = run.decide(block(), "test", 0);
    assert_eq!(
        blocked.reason,
        Reason::Infeasible,
        "one clip of skin 3 is not most of three"
    );
}

/// A unit's clips at a slot: `total` played, `fits` of them the slot's own, the others these.
fn clips_at(total: u32, fits: u32, reused: Vec<Reused>) -> ClipsAt {
    ClipsAt {
        total,
        fits,
        reused,
        graph_found: true,
        ..ClipsAt::default()
    }
}

/// Clips of skin 3 that the slot judged does not play, none of them judged by its joints.
fn of_skin3(clips: u32) -> Reused {
    Reused {
        from: Slot(3),
        clips,
        same_joints: false,
        joints_fit: 0,
        joints_extra: 0,
        joints_off: 0,
        joints_unknown: 0,
        extra_joints: BTreeMap::new(),
    }
}

fn refused(clips: u32) -> ClipBlock {
    let from = Slot(3);
    ClipBlock::Refused { clips, from }
}

fn blocked_by(blocks: Vec<ClipBlock>) -> (Outcome, Why) {
    (Outcome::DoesNotFit, Why::ClipsBlocked { blocks })
}

fn fit_of(at: &ClipsAt, level: AnimReuse) -> (Outcome, Why) {
    let fit = clip_fit(at, level);
    (fit.outcome, fit.why)
}

#[test]
fn a_unit_that_plays_no_clip_is_not_judged_by_its_clips() {
    for graph_found in [true, false] {
        let at = ClipsAt {
            graph_found,
            ..ClipsAt::default()
        };
        for level in AnimReuse::ALL {
            let expected = (Outcome::Unknown, Why::NoClips { graph_found });
            assert_eq!(fit_of(&at, level), expected);
            assert!(
                !clip_fit(&at, level).blocks(),
                "nothing judged rules no slot out"
            );
        }
    }
}

#[test]
fn under_block_a_clip_only_another_skin_plays_refuses_the_slot() {
    let at = clips_at(3, 2, vec![of_skin3(1)]);
    assert_eq!(fit_of(&at, AnimReuse::Block), blocked_by(vec![refused(1)]));
    // The slot's own clips alone fit at every level.
    let own = clips_at(2, 2, Vec::new());
    for level in AnimReuse::ALL {
        let why = Why::ClipsFit {
            fits: 2,
            total: 2,
            reused: 0,
            level: level.as_str().into(),
            unknown: 0,
        };
        assert_eq!(fit_of(&own, level), (Outcome::Fits, why));
    }
}

#[test]
fn under_same_joints_a_clip_fits_when_its_skin_has_the_slots_joints() {
    let same = Reused {
        same_joints: true,
        ..of_skin3(1)
    };
    let why = Why::ClipsFit {
        fits: 3,
        total: 3,
        reused: 1,
        level: "same-joints".into(),
        unknown: 0,
    };
    assert_eq!(
        fit_of(&clips_at(3, 2, vec![same]), AnimReuse::SameJoints),
        (Outcome::Fits, why)
    );
    let other = clips_at(3, 2, vec![of_skin3(1)]);
    assert_eq!(
        fit_of(&other, AnimReuse::SameJoints),
        blocked_by(vec![refused(1)])
    );
}

#[test]
fn under_allow_a_borrowed_clip_fits_where_it_moves_a_joint_of_the_skeleton() {
    // Four clips of skin 3: one on the skeleton's joints alone, one with other joints too,
    // one whose joints did not read, one on none of the skeleton's joints.
    let judged = Reused {
        joints_fit: 1,
        joints_extra: 1,
        joints_unknown: 1,
        joints_off: 1,
        ..of_skin3(4)
    };
    let at = clips_at(5, 1, vec![judged]);
    assert_eq!(fit_of(&at, AnimReuse::Allow), blocked_by(vec![refused(1)]));
    // Without the clip that moves nothing, the unread one is counted and does not block.
    let moving = Reused {
        joints_fit: 1,
        joints_extra: 1,
        joints_unknown: 1,
        ..of_skin3(3)
    };
    let why = Why::ClipsFit {
        fits: 3,
        total: 4,
        reused: 2,
        level: "allow".into(),
        unknown: 1,
    };
    assert_eq!(
        fit_of(&clips_at(4, 1, vec![moving]), AnimReuse::Allow),
        (Outcome::Fits, why)
    );
}

#[test]
fn under_own_rig_reuse_needs_the_authors_rig_and_half_the_clips_or_a_few_borrowed() {
    let borrowed = Reused {
        joints_fit: 3,
        ..of_skin3(3)
    };
    // The game's rig, three of four clips skin 3's: neither an own rig nor a few borrowed.
    let pasted = clips_at(4, 1, vec![borrowed.clone()]);
    let needs = ClipBlock::NeedsOwnRig {
        own_rig: false,
        own: 0,
        borrowed: 3,
        total: 4,
    };
    assert_eq!(
        fit_of(&pasted, AnimReuse::OwnRig),
        blocked_by(vec![refused(3), needs])
    );
    // The author's mesh and skeleton, and half the clips its own.
    let own_work = ClipsAt {
        own_rig: true,
        own: 3,
        ..clips_at(6, 3, vec![borrowed])
    };
    assert_eq!(fit_of(&own_work, AnimReuse::OwnRig).0, Outcome::Fits);
    // One clip borrowed beside three the slot plays.
    let one = Reused {
        joints_fit: 1,
        ..of_skin3(1)
    };
    let few = clips_at(4, 3, vec![one]);
    assert_eq!(fit_of(&few, AnimReuse::OwnRig).0, Outcome::Fits);
    assert_eq!(fit_of(&few, AnimReuse::Block), blocked_by(vec![refused(1)]));
}

#[test]
fn an_own_clip_that_moves_no_joint_of_the_skeleton_refuses_the_slot_at_every_level() {
    let at = ClipsAt {
        own_off: 1,
        own: 2,
        ..clips_at(2, 1, Vec::new())
    };
    for level in AnimReuse::ALL {
        let block = ClipBlock::OwnOff { clips: 1 };
        assert_eq!(fit_of(&at, level), blocked_by(vec![block]));
    }
}

#[test]
fn a_texture_for_the_old_model_is_left_to_the_game() {
    let (history, table) = reworked();
    let env = env(skins::game(), history);
    let run = FactRun::with_reworks(&env, &archive(&old_model_and_pet()), &table);
    let judged: Vec<_> = run.facts.reworks.iter().collect();
    assert_eq!(judged.len(), 1);
    assert_eq!(
        (judged[0].made, judged[0].older, judged[0].newer),
        (Made::Before, 1, 0)
    );
    let decision = run.decide(block(), "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason),
        (None, Reason::PreRework),
        "{}",
        run.dump()
    );
    // Its sub-character goes with it, without a word of its own.
    let pet = run.decide(block(), "testpet", 0);
    assert_eq!((pet.chosen, pet.reason), (None, Reason::PreRework));
    let log = run.decided(block());
    let raised: Vec<&PreRework> = log.of().collect();
    assert_eq!(raised.len(), 1);
    assert_eq!((raised[0].rework.as_str(), raised[0].older), ("16.19", 1));
    assert_eq!(log.len(), 1);
}

#[test]
fn a_file_at_a_path_shipped_since_the_rework_makes_the_mod_newer() {
    let (history, table) = reworked();
    let env = env(skins::game(), history);
    let skin = Skin {
        texture: NEW_TEX.into(),
        ..skins::base_skin()
    };
    let files = [
        mod_skin("test", 0, SKIN0, &skin, vec![]),
        (OLD_TEX.to_string(), blob(OLD_TEX)),
        (NEW_TEX.to_string(), blob(NEW_TEX)),
    ];
    let run = FactRun::with_reworks(&env, &archive(&files), &table);
    assert_eq!(
        run.facts.reworks.iter().next().map(|judged| judged.made),
        Some(Made::After)
    );
    assert_ne!(run.decide(block(), "test", 0).reason, Reason::PreRework);
}

#[test]
fn an_old_model_shipped_whole_is_judged_as_an_own_rig() {
    let (history, table) = reworked();
    let env = env(skins::game(), history);
    let mut files = own_files("a");
    let skin = Skin {
        texture: OLD_TEX.into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![own_graph("a", GRAPH0)],
    ));
    files.push((OLD_TEX.into(), blob(OLD_TEX)));
    let run = FactRun::with_reworks(&env, &archive(&files), &table);
    assert_eq!(
        run.facts.reworks.iter().next().map(|judged| judged.made),
        Some(Made::Before)
    );
    assert_ne!(
        run.decide(block(), "test", 0).reason,
        Reason::PreRework,
        "{}",
        run.dump()
    );
}

#[test]
fn without_a_history_no_rework_is_judged() {
    let env = env(skins::game(), NoHistory);
    let files = [
        mod_skin("test", 0, SKIN0, &skins::base_skin(), vec![]),
        (OLD_TEX.to_string(), blob(OLD_TEX)),
    ];
    let run = FactRun::with_reworks(
        &env,
        &archive(&files),
        &ReworkTable::new([("test", "16.19")]),
    );
    assert_eq!(run.facts.reworks.iter().count(), 0);
}

#[test]
fn a_mod_with_no_file_around_the_rework_and_no_date_is_not_left_to_the_game() {
    let (history, table) = reworked();
    let env = env(skins::game(), history);
    // The game's base model, and no file at a path the rework dropped or added.
    let files = [mod_skin("test", 0, SKIN0, &skins::base_skin(), vec![])];
    let run = FactRun::with_reworks(&env, &archive(&files), &table);
    let judged: Vec<_> = run.facts.reworks.iter().collect();
    assert_eq!(judged.len(), 1, "{}", run.dump());
    assert_eq!(
        (judged[0].made, judged[0].older, judged[0].newer),
        (Made::Unknown, 0, 0)
    );
    assert_ne!(run.decide(block(), "test", 0).reason, Reason::PreRework);
}

#[test]
fn deciding_twice_gives_the_same_targets() {
    let env = env(game_with_clips(), NoHistory);
    let run = FactRun::of(
        &env,
        &archive(&own_rig_on_skin3_clips(&["assets/mod/a/idle.anm"])),
    );
    for level in AnimReuse::ALL {
        let policy = policy(TargetPolicy::Auto, level);
        assert_eq!(policy.decide(&run.facts), policy.decide(&run.facts));
    }
}

#[test]
fn the_report_holds_the_facts_and_the_decisions() {
    let env = env(game_with_clips(), NoHistory);
    let run = FactRun::of(
        &env,
        &archive(&own_rig_on_skin3_clips(&["assets/mod/a/idle.anm"])),
    );
    let report = || {
        let policy = Policy::default();
        let targets = policy
            .decide(&run.facts)
            .compared(&run.facts, &policy.other_levels());
        let speller = run.fix.speller();
        let report = reforge::report::Report::builder()
            .env(&env)
            .module(&run.fix.fixed);
        report
            .facts(&run.facts, &env, &speller)
            .targets(&targets)
            .build()
    };
    let json: serde_json::Value =
        serde_json::from_str(&report().to_json()).expect("the report is JSON");
    assert_eq!(json["targets"]["policy"]["target"], "auto");
    assert_eq!(json["targets"]["policy"]["anim_reuse"], "own-rig");
    let decision = &json["targets"]["decisions"][0];
    assert_eq!(
        (decision["chosen"].as_u64(), decision["reason"].as_str()),
        (Some(3), Some("clips-origin"))
    );
    assert_eq!(decision["alternatives"].as_array().map(Vec::len), Some(3));
    let check = &decision["candidates"][0]["checks"][3];
    assert_eq!(check["aspect"], "clips");
    assert_eq!(check["fit"], "does-not-fit");
    assert!(
        check["detail"]
            .as_str()
            .is_some_and(|detail| detail.starts_with("2 clips of skin 3"))
    );
    let facts = &json["facts"];
    assert_eq!(facts["copies"][0]["role"], "representative");
    assert_eq!(facts["origins"][0]["verdict"], "custom");
    assert_eq!(facts["rigs"][0]["mesh"]["source"], "mod");
    assert_eq!(facts["clips"][0]["at"]["0"]["reused"][0]["from"], 3);
    assert_eq!(facts["candidates"][0]["slots"]["0"]["game_has_bin"], true);
    assert_eq!(
        report().to_json(),
        report().to_json(),
        "two reports of one run are equal"
    );
}

/// A target given to one unit (#ADR-0098), from its words.
fn given(text: &str) -> UnitTarget {
    UnitTarget::parse(text).expect("a unit's target")
}

/// What decisions under `given` raise, in a log of their own.
fn raised(run: &FactRun<'_>, given: &[UnitTarget]) -> Diagnostics {
    let mut log = Diagnostics::new();
    let targets = block().decide_with(&run.facts, given);
    targets.report(&mut log.at(Stage::Decide));
    log
}

#[test]
fn a_target_given_to_one_unit_stands_for_it_and_the_other_follows_the_policy() {
    // The paste of skin 3 at skin0 falls back to skin 3, where the unit written for skin 3
    // supersedes it. Given no target, the paste leaves, and skin 3's unit is the policy's.
    let env = env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&paste_and_own_skin3()));
    let targets = block().decide_with(&run.facts, &[given("Test:0=none")]);
    let paste = run.decision(&targets, "test", 0);
    let written = run.decision(&targets, "test", 3);
    assert_eq!(
        (paste.chosen, paste.reason, paste.taken_by),
        (None, Reason::PerUnit, None),
        "{}",
        run.dump()
    );
    assert!(paste.candidates.is_empty(), "no slot is weighed");
    assert_eq!(
        (written.chosen, written.reason),
        (Some(Slot(3)), Reason::Slot)
    );
    assert_eq!(targets.unit_targets, [given("test:0=none")]);
    let log = raised(&run, &[given("test:0=none")]);
    assert_eq!(log.of::<Superseded>().count(), 0);
    assert_eq!(log.of::<TargetChosen>().count(), 1);
}

#[test]
fn a_target_given_to_a_unit_that_fits_it_raises_nothing_of_its_own() {
    let env = env(skins::game(), NoHistory);
    let files = [mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let targets = block().decide_with(&run.facts, &[given("test:0=3")]);
    let decision = run.decision(&targets, "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason, decision.unfit),
        (Some(Slot(3)), Reason::PerUnit, false),
        "{}",
        run.dump()
    );
    assert_eq!(decision.candidates[0].why, Considered::PerUnit);
    assert!(raised(&run, &[given("test:0=3")]).is_empty());
}

#[test]
fn a_target_given_to_a_unit_its_work_does_not_fit_is_flagged() {
    // Under `auto` the paste of skin 3 falls back to skin 3: given skin0, it stays there.
    let env = env(skins::game(), NoHistory);
    let files = [mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let targets = block().decide_with(&run.facts, &[given("test:0=0")]);
    let decision = run.decision(&targets, "test", 0);
    assert_eq!(
        (decision.chosen, decision.reason, decision.unfit),
        (Some(Slot(0)), Reason::PerUnit, true),
        "{}",
        run.dump()
    );
    let log = raised(&run, &[given("test:0=0")]);
    let unfit: Vec<&ForcedUnfit> = log.of().collect();
    assert_eq!(unfit.len(), 1);
    assert_eq!((unfit[0].slot, unfit[0].follows), (Slot(0), false));
    assert_eq!(log.of::<TargetChosen>().count(), 0);
}

#[test]
fn a_target_given_to_one_of_two_tied_units_leaves_the_slot_to_the_other() {
    // #ADR-0076: the two pets tie on skin 3 and each leaves it. Given no target, the pet of
    // slot 2 leaves, and the pet of slot 1 follows its champion alone.
    let env = env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&paste_and_two_pets()));
    let targets = block().decide_with(&run.facts, &[given("testpet:2=none")]);
    let kept = run.decision(&targets, "testpet", 1);
    let left = run.decision(&targets, "testpet", 2);
    assert_eq!(
        (kept.chosen, kept.reason),
        (Some(Slot(3)), Reason::SubFollows),
        "{}",
        run.dump()
    );
    assert_eq!((left.chosen, left.reason), (None, Reason::PerUnit));
    assert!(targets.conflicts.is_empty());
    assert_eq!(targets.target(run.unit("test", 0)), Some(Slot(3)));
}

#[test]
fn a_target_given_to_a_unit_the_mod_lacks_changes_nothing() {
    let env = env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&paste_and_own_skin3()));
    let absent = [given("test:5=0"), given("other:0=3")];
    let mut targets = block().decide_with(&run.facts, &absent);
    assert_eq!(targets.unit_targets, absent, "the targets as given");
    targets.unit_targets.clear();
    assert_eq!(targets, block().decide(&run.facts));
    assert_eq!(
        raised(&run, &absent).iter().count(),
        run.decided(block()).iter().count()
    );
}
