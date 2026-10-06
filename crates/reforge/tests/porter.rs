//! `Porter` and `Repairer`: one archive taken through the stages by a run, all at once and a
//! stage at a time, with the report of the run as far as it went. The game is
//! `reforge_testkit::skins`.

use std::collections::BTreeSet;
use std::io::Cursor;

use camino::{Utf8Path, Utf8PathBuf};
use reforge::diag::{Action, DiagnosticPolicy, Filter, Stage};
use reforge::env::Env;
use reforge::facts::FactsError;
use reforge::fixups::{FixError, FixupSet};
use reforge::game::MAX_CHAIN;
use reforge::history::NoHistory;
use reforge::load::{Dating, Fault, LoadError, NoSkin, ReworkTable};
use reforge::model::{ContentId, path_hash};
use reforge::package::{
    Blocked, EVIDENCE_FORMAT, EvidenceBound, EvidenceDoc, EvidencePiece, EvidencePolicy,
    EvidenceUnit, MAX_OVERLAY_BINS, Made, Modpkg, OverlayCapped, PackageWriter, PieceKind,
    ProjectDir, README_FILE, REPORT_FILE, WriteError, WriterKind, Written, dds_to_tex,
};
use reforge::port::{AnimReuse, PlanError, Policy, SHADERS_BIN, TargetPolicy};
use reforge::{Evidence, Porter, Repairer, Run, RunError};
use reforge_testkit::bins::{bin, blob, entry, linked_bin, particle, resolver_with};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::mods::FILE_NAME;
use reforge_testkit::packs::{DdsPixels, dds};
use reforge_testkit::plans::{NEW_TEX, OLD_TEX, reworked};
use reforge_testkit::skins::{self, BASE_MESH, BASE_TEX, WAD, skin_bin};
use reforge_testkit::{BrokenGame, BrokenHistory, MemoryGame, MemoryHistory, ModArchive};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const OWN_PARTICLE: &str = "Characters/Test/Skins/Skin0/Particles/Test_Own";
const OWN_TEX: &str = "assets/mod/a/own.tex";

/// The mod's own rig at skin0 with a particle of its own, reached through the resolver.
fn own_archive() -> ModArchive {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
        resolver_with(RESOURCES0, &[("Test_Q_Cast", OWN_PARTICLE)]),
        particle(OWN_PARTICLE, OWN_TEX),
    ];
    let mut files = own_files("a");
    files.push((OWN_TEX.into(), blob(OWN_TEX)));
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files)
}

/// The bytes of [`own_archive`].
fn own_mod() -> Vec<u8> {
    own_archive().build()
}

/// A mod of one texture in a WAD of no champion: `load` ends at `archive.no-skin`.
fn no_skin_mod() -> Vec<u8> {
    let archive = ModArchive::new().file("Other.wad.client", "assets/other/x.tex", blob("x"));
    archive.build()
}

fn game_env() -> Env {
    env(skins::game(), NoHistory)
}

/// The parts of a run's report that are there, by name.
fn parts(run: &Run<'_>) -> Vec<&'static str> {
    let report = run.report();
    let parts = [
        ("mod", report.meta.is_some()),
        ("dating", report.dating.is_some()),
        ("inventory", report.inventory.is_some()),
        ("provenance", report.provenance.is_some()),
        ("changes", report.changes.is_some()),
        ("facts", report.facts.is_some()),
        ("targets", report.targets.is_some()),
        ("port", report.port.is_some()),
        ("package", report.package.is_some()),
        ("again", report.again.is_some()),
    ];
    let there = parts.into_iter().filter(|(_, there)| *there);
    there.map(|(name, _)| name).collect()
}

#[test]
fn a_port_runs_every_stage_but_the_write() {
    let env = game_env();
    let porter = Porter::new(&env);
    let run = porter.port(&own_mod(), FILE_NAME).expect("no fault");

    assert_eq!(run.halted(), None);
    assert!(run.archive().is_none(), "load took the archive");
    assert!(run.fixed().is_some());
    assert!(run.dating().is_some() && run.inventory().is_some() && run.provenance().is_some());
    assert!(run.changes().is_some());
    assert!(run.facts().is_some());
    let targets = run.targets().expect("targets");
    assert_eq!(targets.decisions.len(), 1);
    assert!(run.port().is_some());
    let package = run.package().expect("a package");
    assert!(!package.is_empty());
    assert!(run.overlay().is_some(), "the patches were applied");
    let stages: Vec<Stage> = run.log().iter().map(|d| d.stage).collect();
    assert!(stages.contains(&Stage::Check), "{stages:?}");
    assert!(!stages.contains(&Stage::Write), "{stages:?}");

    assert_eq!(
        parts(&run),
        [
            "mod",
            "dating",
            "inventory",
            "provenance",
            "changes",
            "facts",
            "targets",
            "port",
            "package"
        ]
    );
    let report = run.report();
    assert_eq!(report.halted, None);
    assert_eq!(report.diagnostics.len(), run.log().len());
    let row = report.package.expect("the package's row");
    assert!(
        row.patches.iter().all(|patch| patch.applied.is_some()),
        "each patch is reported applied"
    );
}

#[test]
fn a_run_a_stage_at_a_time_holds_each_result_and_reports_as_the_whole_run() {
    let env = game_env();
    let porter = Porter::new(&env);
    let bytes = own_mod();
    let mut run = porter.run();

    let wads = run
        .read(&bytes, FILE_NAME)
        .expect("the archive reads")
        .wads
        .len();
    assert_eq!(wads, 1);
    assert!(run.module().is_none());
    assert_eq!(parts(&run), [] as [&str; 0]);

    let units = run.load().expect("the mod loads").units.len();
    assert_eq!(units, 1);
    assert!(run.fixed().is_none());
    assert_eq!(parts(&run), ["mod", "dating", "inventory", "provenance"]);

    run.fix().expect("the mod is fixed");
    assert!(run.changes().is_some());
    run.gather().expect("the facts are gathered");
    let chosen = run
        .decide()
        .expect("the targets are chosen")
        .decisions
        .len();
    assert_eq!(chosen, 1);
    assert!(run.port().is_none());
    assert_eq!(
        parts(&run),
        [
            "mod",
            "dating",
            "inventory",
            "provenance",
            "changes",
            "facts",
            "targets"
        ]
    );

    let before = run.names().paths().count();
    run.plan().expect("the port is planned");
    assert!(
        run.names().paths().count() > before,
        "the run learns the paths the plan makes"
    );
    assert!(!run.build().expect("the package is built").is_empty());
    assert!(run.overlay().is_none());
    run.check().expect("the package is checked");
    assert!(run.overlay().is_some());

    let whole = porter.port(&bytes, FILE_NAME).expect("no fault");
    assert_eq!(run.report().to_json(), whole.report().to_json());
}

#[test]
fn a_stage_asked_again_gives_what_the_run_holds() {
    let env = game_env();
    let porter = Porter::new(&env);
    let mut run = porter.port(&own_mod(), FILE_NAME).expect("no fault");
    let raised = run.log().len();

    run.load().expect("the mod");
    run.fix().expect("the fixed mod");
    run.gather().expect("the facts");
    run.decide().expect("the targets");
    run.plan().expect("the plan");
    run.build().expect("the package");
    run.check().expect("checked");
    run.through(Stage::Check).expect("no fault");
    assert_eq!(run.log().len(), raised, "no stage ran twice");
}

#[test]
fn a_run_stopped_after_a_stage_reports_the_stages_it_ran() {
    let env = game_env();
    let porter = Porter::new(&env);
    let bytes = own_mod();
    let loaded = ["mod", "dating", "inventory", "provenance"];
    let stops: [(Stage, &[&str]); 7] = [
        (Stage::Load, &[]),
        (Stage::Fix, &["changes"]),
        (Stage::Facts, &["changes", "facts"]),
        (Stage::Decide, &["changes", "facts", "targets"]),
        (Stage::Plan, &["changes", "facts", "targets", "port"]),
        (
            Stage::Build,
            &["changes", "facts", "targets", "port", "package"],
        ),
        (
            Stage::Check,
            &["changes", "facts", "targets", "port", "package"],
        ),
    ];
    for (last, more) in stops {
        let mut run = porter.run();
        run.read(&bytes, FILE_NAME).expect("the archive reads");
        run.through(last).expect("no fault");
        let expected: Vec<&str> = loaded.iter().chain(more).copied().collect();
        assert_eq!(parts(&run), expected, "stopped after {last}");
        let latest = run.log().iter().map(|d| d.stage).max();
        assert!(latest <= Some(last), "stopped after {last}: {latest:?}");
        assert_eq!(run.overlay().is_some(), last == Stage::Check);
    }
}

#[test]
fn a_run_names_its_stages_and_says_which_ran_to_their_end() {
    let env = game_env();
    let bytes = own_mod();
    let ran = |run: &Run<'_>| -> Vec<Stage> {
        let stages = run.stages().iter().copied();
        stages.filter(|&stage| run.ran(stage)).collect()
    };

    let porter = Porter::new(&env);
    let mut run = porter.run();
    assert_eq!(
        run.stages(),
        [
            Stage::Read,
            Stage::Load,
            Stage::Fix,
            Stage::Facts,
            Stage::Decide,
            Stage::Plan,
            Stage::Build,
            Stage::Check,
            Stage::Write
        ]
    );
    assert_eq!(ran(&run), []);
    run.read(&bytes, FILE_NAME).expect("the archive reads");
    assert_eq!(ran(&run), [Stage::Read]);
    // `load` takes the archive, and `read` still ran.
    run.through(Stage::Decide).expect("no fault");
    assert_eq!(ran(&run), run.stages()[..5]);
    run.through(Stage::Build).expect("no fault");
    assert_eq!(ran(&run), run.stages()[..7]);
    run.check().expect("the package is checked");
    assert_eq!(ran(&run), run.stages()[..8]);
    assert_eq!(run.halted_at(), None);

    let repairer = Repairer::new(&env);
    let mut repair = repairer.repair(&bytes, FILE_NAME).expect("no fault");
    let in_place = [
        Stage::Read,
        Stage::Load,
        Stage::Fix,
        Stage::Build,
        Stage::Check,
        Stage::Write,
    ];
    assert_eq!(repair.stages(), in_place);
    assert_eq!(ran(&repair), in_place[..5]);

    // A write the policy refuses did not run. One that wrote did.
    let dir = tempfile::tempdir().expect("a directory");
    let out = Utf8Path::from_path(dir.path()).expect("a UTF-8 path");
    let project = ProjectDir::new(out);
    let writers: [&dyn PackageWriter; 1] = [&project];
    let strict = DiagnosticPolicy::default().with(Filter::all(), Action::Block);
    assert!(repair.write(&writers, &strict).is_err());
    assert!(!repair.ran(Stage::Write));
    repair
        .write(&writers, &DiagnosticPolicy::default())
        .expect("the package writes");
    assert_eq!(ran(&repair), in_place);
}

#[test]
fn a_stage_a_fatal_diagnostic_ended_did_not_run() {
    let env = game_env();
    let porter = Porter::new(&env);
    let run = porter
        .port(&no_skin_mod(), FILE_NAME)
        .expect("a halt is no error");
    assert_eq!(run.halted_at(), Some(Stage::Load));
    assert!(run.ran(Stage::Read));
    assert!(run.module().is_some(), "the mod as far as load went");
    assert!(!run.ran(Stage::Load) && !run.ran(Stage::Fix));

    let mut run = porter.run();
    assert!(run.read(b"no zip", FILE_NAME).is_err());
    assert_eq!(run.halted_at(), Some(Stage::Read));
    assert!(!run.ran(Stage::Read));
}

#[test]
fn a_porter_decides_under_its_policy_and_records_the_compared_ones() {
    let env = game_env();
    let policy = Policy {
        target: TargetPolicy::Base,
        anim_reuse: AnimReuse::Allow,
    };
    let porter = Porter::new(&env)
        .policy(policy)
        .compare(policy.other_levels());
    let mut run = porter.run();
    run.read(&own_mod(), FILE_NAME).expect("the archive reads");
    run.through(Stage::Decide).expect("no fault");

    let targets = run.targets().expect("targets");
    assert_eq!(targets.policy, policy);
    let alternatives = targets.alternatives.values().next().expect("alternatives");
    assert_eq!(alternatives.len(), policy.other_levels().len());
    assert!(run.port().is_none());
}

#[test]
fn a_repair_builds_in_place_with_no_facts_and_is_whole_when_repaired_again() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    let mut run = repairer.repair(&own_mod(), FILE_NAME).expect("no fault");

    assert_eq!(run.halted(), None);
    assert!(run.fixed().is_some() && run.package().is_some());
    assert!(run.inventory().is_none() && run.provenance().is_none());
    assert!(run.facts().is_none() && run.targets().is_none() && run.port().is_none());
    assert!(run.overlay().is_none());
    assert!(matches!(
        run.gather(),
        Err(RunError::NotInARepair(Stage::Facts))
    ));
    assert!(matches!(
        run.plan(),
        Err(RunError::NotInARepair(Stage::Plan))
    ));
    assert_eq!(parts(&run), ["mod", "dating", "changes", "package"]);

    let again = run.again().expect("no fault").clone();
    assert!(again.same(), "{again:?}");
    assert_eq!(run.repaired_again(), Some(&again));
    assert_eq!(
        parts(&run),
        ["mod", "dating", "changes", "package", "again"]
    );
}

#[test]
fn a_port_is_not_repaired_again() {
    let env = game_env();
    let porter = Porter::new(&env);
    let mut run = porter.port(&own_mod(), FILE_NAME).expect("no fault");
    assert!(matches!(run.again(), Err(RunError::NotARepair)));
}

#[test]
fn a_run_halted_in_load_reports_the_mod_as_far_as_it_went() {
    let env = game_env();
    let porter = Porter::new(&env);
    let mut run = porter
        .port(&no_skin_mod(), FILE_NAME)
        .expect("a halt is no error");

    let halted = run.halted().expect("the run halted");
    assert_eq!(run.log().of::<NoSkin>().count(), 1);
    assert!(run.module().is_some(), "the mod as far as load went");
    assert!(run.fixed().is_none() && run.package().is_none());
    assert!(matches!(run.fix(), Err(RunError::Halted(at)) if at == halted));
    assert!(matches!(run.build(), Err(RunError::Halted(_))));

    assert_eq!(parts(&run), ["mod"]);
    let report = run.report();
    assert_eq!(report.halted, Some(halted.0.0));
    assert_eq!(report.wads.len(), 1);
    assert!(report.inputs.is_some());
    assert_eq!(report.diagnostics.len(), run.log().len());

    let repairer = Repairer::new(&env);
    let repair = repairer
        .repair(&no_skin_mod(), FILE_NAME)
        .expect("a halt is no error");
    assert_eq!(parts(&repair), ["mod"]);
    assert_eq!(repair.report().halted, Some(halted.0.0));
}

#[test]
fn a_run_halted_in_read_reports_its_diagnostics() {
    let env = game_env();
    let porter = Porter::new(&env);
    let mut run = porter.run();
    assert!(matches!(
        run.read(b"no zip", FILE_NAME),
        Err(RunError::Halted(_))
    ));
    run.through(Stage::Check).expect("a halt is no error");

    assert!(run.module().is_none());
    assert!(matches!(run.load(), Err(RunError::Halted(_))));
    assert_eq!(parts(&run), [] as [&str; 0]);
    let report = run.report();
    assert!(report.halted.is_some());
    assert_eq!(report.diagnostics.len(), 1);
}

#[test]
fn a_stage_whose_input_the_run_does_not_hold_is_not_reached() {
    let env = game_env();
    let porter = Porter::new(&env);
    let mut run = porter.run();
    let not_reached = |result: Result<(), RunError>| match result {
        Err(RunError::NotReached { stage, needs }) => (stage, needs),
        other => panic!("not reached, and got {other:?}"),
    };

    assert_eq!(
        not_reached(run.load().map(drop)),
        (Stage::Load, Stage::Read)
    );
    assert_eq!(not_reached(run.fix().map(drop)), (Stage::Fix, Stage::Load));
    run.read(&own_mod(), FILE_NAME).expect("the archive reads");
    run.load().expect("the mod loads");
    assert_eq!(
        not_reached(run.gather().map(drop)),
        (Stage::Facts, Stage::Fix)
    );
    assert_eq!(
        not_reached(run.build().map(drop)),
        (Stage::Build, Stage::Fix)
    );
    let project = ProjectDir::new("unused");
    let writers: [&dyn PackageWriter; 1] = [&project];
    assert_eq!(
        not_reached(run.write(&writers, &DiagnosticPolicy::default())),
        (Stage::Write, Stage::Build)
    );
    assert!(run.log().of::<Blocked>().next().is_none());
}

/// An environment whose game lists the chunk at a path and does not read it.
fn env_broken_at(game: MemoryGame, path: &str) -> Env {
    let game = BrokenGame::at(game, path);
    let sizes = game.sizes();
    let env = Env::builder().game(game).sizes(sizes);
    env.build().expect("an environment")
}

/// A porter's run of the mod, taken through a stage with no fault.
fn run_through<'a>(porter: &'a Porter<'_>, last: Stage) -> Run<'a> {
    let mut run = porter.run();
    run.read(&own_mod(), FILE_NAME).expect("the archive reads");
    run.through(last).expect("no fault up to the stage");
    run
}

#[test]
fn a_history_that_does_not_answer_is_a_fault_of_load_and_no_halt() {
    let history = BrokenHistory(MemoryHistory::new(&["16.19.1"]));
    let env = env(skins::game(), history);
    let porter = Porter::new(&env);
    let mut run = porter.run();
    run.read(&own_mod(), FILE_NAME).expect("the archive reads");
    let loaded = run.load().map(drop);
    assert!(
        matches!(loaded, Err(RunError::Load(LoadError::History(_)))),
        "{loaded:?}"
    );
    assert_eq!(run.halted(), None);
    assert!(run.dating().is_none());

    // A fault ends `port` and `repair` with the error, where a halt ends them with the run.
    let whole = porter.port(&own_mod(), FILE_NAME).map(drop);
    assert!(matches!(whole, Err(RunError::Load(_))), "{whole:?}");
    let repairer = Repairer::new(&env);
    let repaired = repairer.repair(&own_mod(), FILE_NAME).map(drop);
    assert!(matches!(repaired, Err(RunError::Load(_))), "{repaired:?}");
}

/// The game with a bin skin 7's bin links, which does not read: `load` reads the skin bins
/// alone, and `fix` the bins they link for their types.
fn env_broken_past_skin7() -> Env {
    const LINKED: &str = "data/characters/test/skins/skin7_shared.bin";
    let mut game = skins::game();
    let skin7 = entry("Characters/Test/Skins/Skin7", &skins::skin7());
    game.insert(&skin_bin("test", 7), linked_bin(vec![skin7], &[LINKED]));
    game.insert(LINKED, bin(vec![]));
    env_broken_at(game, LINKED)
}

#[test]
fn a_game_bin_that_does_not_read_while_the_types_are_read_is_a_fault_of_fix() {
    // `fix` reads the game's bins of every skin of the mod's characters for their types.
    let env = env_broken_past_skin7();
    let porter = Porter::new(&env);
    let mut run = run_through(&porter, Stage::Load);
    let fixed = run.fix().map(drop);
    assert!(
        matches!(fixed, Err(RunError::Fix(FixError::Game(_)))),
        "{fixed:?}"
    );
    assert_eq!(run.halted(), None);
    assert!(run.fixed().is_none() && run.changes().is_none());
}

#[test]
fn a_fault_of_fix_leaves_the_run_holding_the_mod_as_load_left_it() {
    let env = env_broken_past_skin7();
    let porter = Porter::new(&env);
    let mut run = run_through(&porter, Stage::Load);
    let before = parts(&run);
    assert_eq!(before, ["mod", "dating", "inventory", "provenance"]);
    assert!(matches!(run.fix(), Err(RunError::Fix(_))));
    // The report of the run as far as it went holds what `load` gave.
    assert!(run.module().is_some());
    assert_eq!(parts(&run), before);
}

#[test]
fn a_game_skeleton_that_does_not_read_is_a_fault_of_facts() {
    let env = env_broken_at(skins::game(), skins::BASE_SKEL);
    let porter = Porter::new(&env);
    let mut run = run_through(&porter, Stage::Fix);
    let gathered = run.gather().map(drop);
    assert!(
        matches!(gathered, Err(RunError::Facts(FactsError::Game(_)))),
        "{gathered:?}"
    );
    assert_eq!(run.halted(), None);
    assert!(run.fixed().is_some() && run.facts().is_none());
    assert_eq!(
        parts(&run),
        ["mod", "dating", "inventory", "provenance", "changes"]
    );
}

#[test]
fn the_bin_of_shader_definitions_that_does_not_read_is_a_fault_of_plan() {
    let mut game = skins::game();
    game.insert(SHADERS_BIN, bin(vec![]));
    let env = env_broken_at(game, SHADERS_BIN);
    let porter = Porter::new(&env);
    let mut run = run_through(&porter, Stage::Decide);
    let planned = run.plan().map(drop);
    assert!(
        matches!(planned, Err(RunError::Plan(PlanError::Game(_)))),
        "{planned:?}"
    );
    assert_eq!(run.halted(), None);
    assert!(run.targets().is_some() && run.port().is_none());
}

#[test]
fn a_game_texture_that_does_not_read_while_a_texture_is_judged_is_a_fault_of_build() {
    // The mod's DDS at the base texture's path goes into TEX's container, where it is
    // judged against the game's file: a file of that size, so its bytes are read.
    let texture = dds(DdsPixels::Dxt1, (8, 8), 4);
    let mut theirs = dds_to_tex(&texture).expect("TEX holds it");
    theirs[12] ^= 0xff;
    let mut game = skins::game();
    game.insert(skins::BASE_TEX, theirs);
    let env = env_broken_at(game, skins::BASE_TEX);
    let skin = bin(vec![entry(SKIN0, &skins::base_skin())]);
    let archive = ModArchive::new()
        .file(skins::WAD, &skin_bin("test", 0), skin)
        .file(skins::WAD, skins::BASE_TEX, texture);
    let repairer = Repairer::new(&env);
    let mut run = repairer.run();
    run.read(&archive.build(), FILE_NAME)
        .expect("the archive reads");
    run.through(Stage::Fix).expect("no fault up to the build");
    let built = run.build().map(drop);
    assert!(
        matches!(built, Err(RunError::Build(Fault::Game(_)))),
        "{built:?}"
    );
    assert_eq!(run.halted(), None);
    assert!(run.package().is_none());
    assert_eq!(parts(&run), ["mod", "dating", "changes"]);
}

#[test]
fn a_game_bin_that_does_not_read_while_the_patches_are_applied_is_a_fault_of_check() {
    // The game's base skin bin leads a chain of linked bins longer than the stages before
    // `check` follow. The overlay loads the whole chain.
    let link = |n: usize| format!("data/characters/test/linked{n}.bin");
    let mut game = skins::game();
    let base = vec![entry(SKIN0, &skins::base_skin())];
    game.insert(&skin_bin("test", 0), linked_bin(base, &[&link(1)]));
    for n in 1..=MAX_CHAIN {
        let next = link(n + 1);
        let links: &[&str] = if n < MAX_CHAIN { &[&next] } else { &[] };
        let objects = vec![particle(&format!("Linked{n}"), skins::BASE_TEX)];
        game.insert(&link(n), linked_bin(objects, links));
    }
    let env = env_broken_at(game, &link(MAX_CHAIN));
    let porter = Porter::new(&env);
    let mut run = run_through(&porter, Stage::Build);
    let checked = run.check();
    assert!(matches!(checked, Err(RunError::Check(_))), "{checked:?}");
    assert_eq!(run.halted(), None);
    // The package stands, unchecked.
    assert!(run.package().is_some() && run.overlay().is_none());
    let checks = run.log().iter().filter(|d| d.stage == Stage::Check);
    assert_eq!(checks.count(), 0);
}

#[test]
fn a_patched_bin_whose_load_passes_the_overlays_cap_is_reported() {
    let link = |n: usize| format!("data/characters/test/linked{n}.bin");
    let mut game = skins::game();
    let base = vec![entry(SKIN0, &skins::base_skin())];
    game.insert(&skin_bin("test", 0), linked_bin(base, &[&link(1)]));
    let last = MAX_OVERLAY_BINS + 2;
    for n in 1..=last {
        let next = link(n + 1);
        let links: &[&str] = if n < last { &[&next] } else { &[] };
        game.insert(&link(n), linked_bin(vec![], links));
    }
    let env = env(game, NoHistory);
    let porter = Porter::new(&env);
    let mut run = run_through(&porter, Stage::Build);
    run.check().expect("the checks run");
    let capped: Vec<&OverlayCapped> = run.log().of().collect();
    assert_eq!(capped.len(), 1);
    assert_eq!(capped[0].target.as_str(), skin_bin("test", 0));
    assert_eq!(capped[0].cap, MAX_OVERLAY_BINS);
}

#[test]
fn a_writer_that_does_not_write_is_a_fault_of_write_with_the_writers_kind() {
    let env = game_env();
    let porter = Porter::new(&env);
    let mut run = porter.port(&own_mod(), FILE_NAME).expect("no fault");
    let dir = tempfile::tempdir().expect("a directory");
    let out = camino::Utf8Path::from_path(dir.path()).expect("a UTF-8 path");
    let project = ProjectDir::new(out);
    // A directory that is no mod project is where the project goes.
    let theirs = project.path_of(run.package().expect("a package"));
    fs_err::create_dir_all(&theirs).expect("a directory");
    fs_err::write(theirs.join("notes.txt"), "someone's own").expect("a file");

    let writers: [&dyn PackageWriter; 1] = [&project];
    let written = run.write(&writers, &DiagnosticPolicy::default());
    let Err(RunError::Write { kind, source }) = written else {
        panic!("the writer does not write: {written:?}");
    };
    assert_eq!(kind, WriterKind::Project);
    assert!(
        matches!(&source, WriteError::Occupied(path) if *path == theirs),
        "{source:?}"
    );
    assert_eq!(run.log().of::<Written>().count(), 0);
    assert_eq!(run.log().of::<Blocked>().count(), 0);
}

#[test]
fn a_write_puts_the_report_of_the_run_beside_the_package() {
    let env = game_env();
    let porter = Porter::new(&env);
    let mut run = porter.port(&own_mod(), FILE_NAME).expect("no fault");
    let dir = tempfile::tempdir().expect("a directory");
    let out = camino::Utf8Path::from_path(dir.path()).expect("a UTF-8 path");
    let project = ProjectDir::new(out);
    let writers: [&dyn PackageWriter; 1] = [&project];

    // A policy that blocks on anything refuses the write, and nothing is written.
    let strict = DiagnosticPolicy::default().with(Filter::all(), Action::Block);
    assert!(matches!(
        run.write(&writers, &strict),
        Err(RunError::Refused(_))
    ));
    assert_eq!(run.log().of::<Blocked>().count(), 1);
    assert_eq!(fs_err::read_dir(out).expect("the directory").count(), 0);

    let before = run.report().to_json() + "\n";
    run.write(&writers, &DiagnosticPolicy::default())
        .expect("the package writes");
    assert_eq!(run.log().of::<Written>().count(), 1);
    let package = run.package().expect("a package");
    let beside = project.path_of(package).join(REPORT_FILE);
    let beside = fs_err::read_to_string(beside).expect("the report beside the package");
    assert_eq!(beside, before, "the run as it stood before the write");
}

/// Bytes the game shipped at the base texture's path at the first build, large enough to
/// date a mod.
fn dated_texture() -> Vec<u8> {
    vec![b't'; 5000]
}

/// A mod with no bin that restores the model from before the rework of [`reworked`]: its own
/// mesh, the old texture at the base texture's path, a file at a path the rework dropped and
/// one at a path only the rework has.
fn restoring_mod() -> Vec<u8> {
    let archive = ModArchive::new()
        .file(WAD, BASE_MESH, b"the mod's own mesh".to_vec())
        .file(WAD, BASE_TEX, dated_texture())
        .file(WAD, OLD_TEX, b"the old model's texture".to_vec())
        .file(
            WAD,
            NEW_TEX,
            b"the mod's file at the reworked model's path".to_vec(),
        );
    archive.build()
}

/// The game with the history of [`reworked`], in which it also held [`dated_texture`] at the
/// base texture's path at the first build, and the table of that rework.
fn reworked_env() -> (Env, ReworkTable) {
    let (history, reworks) = reworked();
    let history = history
        .with_content(BASE_TEX, &dated_texture(), 0..=0)
        .with_path(BASE_TEX, 0..=1);
    (env(skins::game(), history), reworks)
}

/// A new directory for a test, as a UTF-8 path.
fn scratch() -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("a UTF-8 path");
    (dir, path)
}

/// Writes a run's package as a project directory and gives the README of the project.
fn written_readme(run: &mut Run<'_>, out: &Utf8Path) -> String {
    let project = ProjectDir::new(out);
    let writers: [&dyn PackageWriter; 1] = [&project];
    run.write(&writers, &DiagnosticPolicy::default())
        .expect("the package writes");
    let package = run.package().expect("a package");
    let readme = project.path_of(package).join(README_FILE);
    fs_err::read_to_string(readme).expect("the README of the project")
}

/// #ADR-0041: the package of a repair holds what a second run cannot judge again from it.
#[test]
fn a_repair_embeds_the_evidence_of_its_run_in_the_package_it_writes() {
    let (env, reworks) = reworked_env();
    let repairer = Repairer::new(&env).reworks(reworks);
    let bytes = restoring_mod();
    let mut run = repairer.repair(&bytes, FILE_NAME).expect("no fault");
    assert_eq!(run.halted(), None);
    let held = run.package().expect("a package").evidence.clone();
    let held = held.expect("the run gave its package the evidence");

    let (_guard, out) = scratch();
    let readme = written_readme(&mut run, &out);
    assert_eq!(readme, held.readme());
    let doc = EvidenceDoc::parse(&readme).expect("the evidence reads");
    let doc = doc.expect("the README holds the evidence");
    assert_eq!(doc, held);

    assert_eq!(doc.format, EVIDENCE_FORMAT);
    assert_eq!(doc.run, Made::Repaired);
    assert_eq!(doc.archive.sha256, ContentId::of(&bytes).hex());
    assert_eq!(doc.inputs.build.as_deref(), Some("16.19.1"));
    let history = doc.inputs.history.as_ref().expect("the history");
    assert_eq!((history.head.as_str(), history.builds), ("c1", 2));
    // The old texture dates the mod at the first build, and the package ships it: the WAD
    // restores the old model.
    let bound = EvidenceBound {
        at: 0,
        build: "16.18.1".into(),
    };
    assert_eq!(doc.dating.not_before, Some(bound));
    let piece = EvidencePiece {
        piece: PieceKind::File,
        hash: format!("{:016x}", path_hash(BASE_TEX).0),
        key: ContentId::of(&dated_texture()).hex(),
    };
    assert_eq!(doc.dating.evidence, Some(piece));
    assert_eq!(doc.restored, ["test.wad.client"]);
    assert_eq!(doc.fixups, FixupSet::standard().ids());
    assert_eq!(doc.port, None);

    // What `again` judges the second run with is in the document.
    let dating = run.dating().expect("the dating");
    assert_eq!(dating.build(), Some(1));
    assert_eq!(dating.position().map(|at| at.0), Some(0));
    assert_eq!((doc.dating.dated, doc.dating.at_bound), (1, 1));
}

/// #ADR-0041: a port's package holds the policy and where each unit went, and the `.modpkg`
/// carries the README as its readme.
#[test]
fn a_port_embeds_its_policy_and_targets_and_the_modpkg_holds_them() {
    let env = game_env();
    let policy = Policy {
        target: TargetPolicy::Base,
        anim_reuse: AnimReuse::Allow,
    };
    let porter = Porter::new(&env).policy(policy);
    let bytes = own_mod();
    let mut run = porter.port(&bytes, FILE_NAME).expect("no fault");
    let reason = {
        let targets = run.targets().expect("targets");
        let decision = targets.decisions.values().next().expect("a decision");
        assert_eq!(decision.chosen.map(|slot| slot.0), Some(0));
        decision.reason.as_str()
    };

    let (_guard, out) = scratch();
    let (project, packed) = (ProjectDir::new(&out), Modpkg::new(&out));
    let writers: [&dyn PackageWriter; 2] = [&project, &packed];
    run.write(&writers, &DiagnosticPolicy::default())
        .expect("the package writes");
    let package = run.package().expect("a package");
    let held = package.evidence.clone().expect("the evidence");
    let readme = project.path_of(package).join(README_FILE);
    let readme = fs_err::read_to_string(readme).expect("the README of the project");
    let doc = EvidenceDoc::parse(&readme).expect("the evidence reads");
    assert_eq!(doc.as_ref(), Some(&held));

    assert_eq!(held.run, Made::Ported);
    assert_eq!(held.archive.sha256, ContentId::of(&bytes).hex());
    assert_eq!(held.inputs.history, None);
    assert_eq!(held.dating.not_before, None);
    assert_eq!(held.restored, Vec::<String>::new());
    let port = held.port.as_ref().expect("what the port decided");
    let policy = EvidencePolicy {
        target: "base".into(),
        anim_reuse: "allow".into(),
    };
    assert_eq!(port.policy, policy);
    let unit = EvidenceUnit {
        character: "test".into(),
        source: 0,
        target: 0,
        reason: reason.into(),
    };
    assert_eq!(port.units, [unit]);

    let file = fs_err::read(packed.path_of(package)).expect("the .modpkg reads");
    let mut mounted = ltk_modpkg::Modpkg::mount_from_reader(Cursor::new(file)).expect("it mounts");
    let packed_readme = mounted.load_readme().expect("the .modpkg has a readme");
    assert_eq!(packed_readme, readme.into_bytes());
}

#[test]
fn two_runs_of_one_archive_write_the_same_readme() {
    let (env, reworks) = reworked_env();
    let repairer = Repairer::new(&env).reworks(reworks);
    let porter = Porter::new(&env);
    let readme_of = |mut run: Run<'_>| {
        let (_guard, out) = scratch();
        written_readme(&mut run, &out).into_bytes()
    };
    let repair = || {
        repairer
            .repair(&restoring_mod(), FILE_NAME)
            .expect("no fault")
    };
    assert_eq!(readme_of(repair()), readme_of(repair()));
    let port = || porter.port(&own_mod(), FILE_NAME).expect("no fault");
    let first = readme_of(port());
    assert_eq!(first, readme_of(port()));
    assert!(!first.contains(&b'\r'), "the lines end in a line feed");
}

/// A README of the mod's own is an entry outside its WADs, which no package carries: the
/// project's README is the run's, and a block of evidence the mod wrote is not read.
#[test]
fn a_readme_of_the_mods_own_does_not_reach_the_packages_readme() {
    let env = game_env();
    let theirs = format!("# Theirs\n\n```{EVIDENCE_FORMAT}\n{{ \"restored\": [\"x\"] }}\n```\n");
    let plain = own_archive();
    let with_readme = plain
        .clone()
        .extra(README_FILE, theirs.clone().into_bytes())
        .extra("META/readme.md", theirs.into_bytes());
    let repairer = Repairer::new(&env);
    let readme_of = |bytes: &[u8]| {
        let mut run = repairer.repair(bytes, FILE_NAME).expect("no fault");
        let (_guard, out) = scratch();
        let readme = written_readme(&mut run, &out);
        (readme, run.package().expect("a package").evidence.clone())
    };
    let (readme, held) = readme_of(&with_readme.build());
    assert!(!readme.contains("Theirs"), "{readme}");
    let doc = EvidenceDoc::parse(&readme).expect("the evidence reads");
    assert_eq!(doc, held);
    let doc = doc.expect("the run's evidence");
    assert_eq!(doc.restored, Vec::<String>::new());

    // The two archives differ by their digest alone.
    let (_, without) = readme_of(&plain.build());
    let mut without = without.expect("the run's evidence");
    assert_ne!(without.archive, doc.archive);
    without.archive = doc.archive.clone();
    assert_eq!(without, doc);
}

#[test]
fn the_report_holds_the_evidence_the_package_embeds() {
    let env = game_env();
    let porter = Porter::new(&env);
    let mut run = porter.run();
    run.read(&own_mod(), FILE_NAME).expect("the archive reads");
    run.through(Stage::Plan).expect("no fault");
    assert_eq!(run.report().evidence, None, "no package is built");

    run.through(Stage::Check).expect("no fault");
    let held = run.package().expect("a package").evidence.clone();
    assert!(held.is_some());
    let report = run.report();
    assert_eq!(report.evidence, held);
    let json: serde_json::Value = serde_json::from_str(&report.to_json()).expect("JSON");
    assert_eq!(json["evidence"]["format"], EVIDENCE_FORMAT);
    assert_eq!(json["evidence"]["run"], "port");
    assert_eq!(json["evidence"]["archive"]["sha256"], json["mod"]["sha256"]);
}

/// The pieces of a dating taken from an earlier run are ids of that run's mod, so a run
/// judged with one writes no evidence of its own.
#[test]
fn a_run_judged_with_an_earlier_runs_evidence_embeds_none() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    let earlier = Evidence {
        dating: Dating::default(),
        restored: BTreeSet::new(),
    };
    let mut run = repairer.run().known(earlier);
    run.read(&own_mod(), FILE_NAME).expect("the archive reads");
    run.through(Stage::Check).expect("no fault");
    assert_eq!(run.package().expect("a package").evidence, None);
    assert_eq!(run.report().evidence, None);
}
