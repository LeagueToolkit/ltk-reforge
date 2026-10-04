//! The decision over synthetic mods: the skin0-first rule with its premium fallback, a paste
//! on the base's rig staying on skin0, a mod for a premium slot staying there, forced targets,
//! sub-characters, two units on one slot, the levels of animation reuse and reworks. The game
//! is `reforge_testkit::skins`.

use reforge::diag::Diag;
use reforge::facts::{Made, Outcome, Verdict};
use reforge::history::NoHistory;
use reforge::load::ReworkTable;
use reforge::model::Slot;
use reforge::port::{
    AnimReuse, ForcedUnfit, Infeasible, NewBin, Policy, PreRework, Reason, Superseded,
    TargetChosen, TargetPolicy, Unjudged,
};
use reforge_testkit::bins::{Skin, blob, graph, resolver};
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, BASE_CLIPS, S3_CLIPS, S3_SKEL};
use reforge_testkit::{FactRun, MemoryHistory, rigs};

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
    assert!(!decision.candidates[0].feasible);
    assert!(decision.candidates[1].feasible);
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

/// A champion pasted from skin 3 beside its pet on the pet's base model.
fn champion_and_pet() -> Vec<(String, Vec<u8>)> {
    let pet = Skin {
        texture: "assets/mod/pet/body.tex".into(),
        ..skins::pet_skin()
    };
    vec![
        mod_skin("test", 0, SKIN0, &skins::skin3(), vec![]),
        (
            "assets/mod/pet/body.tex".into(),
            blob("assets/mod/pet/body.tex"),
        ),
        mod_skin("testpet", 0, "Characters/TestPet/Skins/Skin0", &pet, vec![]),
    ]
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
    assert_eq!(pet.new_bin, Some(Slot(3)));
    assert!(pet.unfit);
    let log = run.decided(block());
    assert_eq!(log.of::<NewBin>().count(), 1);
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
    // A paste of skin 3 in skin0, and the author's own version at slot 3.
    let mut files = own_files("a");
    files.push(mod_skin("test", 0, SKIN0, &skins::skin3(), vec![]));
    let own = own_skin(
        "a",
        "Characters/Test/Animations/Skin3",
        "Characters/Test/Skins/Skin3/Resources",
    );
    let extra = vec![own_graph("a", "Characters/Test/Animations/Skin3")];
    files.push(mod_skin(
        "test",
        3,
        "Characters/Test/Skins/Skin3",
        &own,
        extra,
    ));
    let run = FactRun::of(&env, &archive(&files));
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
    let allowed = at(AnimReuse::Allow);
    assert_eq!(
        (allowed.chosen, allowed.reason),
        (Some(Slot(0)), Reason::Base)
    );
    let clips = allowed.candidates[0]
        .checks
        .last()
        .expect("the clips are judged");
    assert_eq!(clips.fit.outcome, Outcome::Fits);
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
        "2 clips of skin 3, reuse needs its own rig and half its clips: own mesh and skeleton, 1 of 3 clips its own"
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

const OLD_TEX: &str = "assets/characters/test/skins/base/test_old.tex";
const NEW_TEX: &str = "assets/characters/test/skins/base/test_new.tex";

/// A history of two builds in which `test` is reworked at the second: the old texture's path
/// is dropped at it, and the new one's ships from it.
fn reworked() -> (MemoryHistory, ReworkTable) {
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]);
    let history = history.with_path(OLD_TEX, 0..=0).with_path(NEW_TEX, 1..=1);
    (history, ReworkTable::new([("test", "16.19")]))
}

#[test]
fn a_texture_for_the_old_model_is_left_to_the_game() {
    let (history, table) = reworked();
    let env = env(skins::game(), history);
    let skin = Skin {
        texture: OLD_TEX.into(),
        ..skins::base_skin()
    };
    let mut files = champion_and_pet();
    files[0] = mod_skin("test", 0, SKIN0, &skin, vec![]);
    files.push((OLD_TEX.into(), blob(OLD_TEX)));
    let run = FactRun::with_reworks(&env, &archive(&files), &table);
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
