//! `Porter` and `Repairer`: one archive taken through the stages by a run, all at once and a
//! stage at a time, with the report of the run as far as it went. The game is
//! `reforge_testkit::skins`.

use reforge::diag::{Action, DiagnosticPolicy, Filter, Stage};
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::load::NoSkin;
use reforge::package::{Blocked, PackageWriter, ProjectDir, REPORT_FILE, Written};
use reforge::port::{AnimReuse, Policy, TargetPolicy};
use reforge::{Porter, Repairer, Run, RunError};
use reforge_testkit::ModArchive;
use reforge_testkit::bins::{bin, blob, entry, particle, resolver_with};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::mods::FILE_NAME;
use reforge_testkit::skins::{self, skin_bin};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const OWN_PARTICLE: &str = "Characters/Test/Skins/Skin0/Particles/Test_Own";
const OWN_TEX: &str = "assets/mod/a/own.tex";

/// The mod's own rig at skin0 with a particle of its own, reached through the resolver.
fn own_mod() -> Vec<u8> {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
        resolver_with(RESOURCES0, &[("Test_Q_Cast", OWN_PARTICLE)]),
        particle(OWN_PARTICLE, OWN_TEX),
    ];
    let mut files = own_files("a");
    files.push((OWN_TEX.into(), blob(OWN_TEX)));
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files).build()
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
