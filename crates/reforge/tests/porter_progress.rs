//! A run's progress (#ADR-0097): each stage reports its steps as they begin, by kind and
//! name, numbered from 1 and counted, to a listener the caller registers on the log. A cancel
//! set at a stage's first step ends the stage before its second. The game is
//! `reforge_testkit::skins`.

use std::sync::{Arc, Mutex};

use reforge::diag::{Cancel, Diagnostics, Progress, Stage, Step};
use reforge::env::Env;
use reforge::facts::FACTS_PARTS;
use reforge::history::NoHistory;
use reforge::load::LOAD_PARTS;
use reforge::package::CheckSet;
use reforge::port::{PLAN_PARTS, Policy};
use reforge::snapshot::{Snapshots, to_json};
use reforge::store::{BlobStore, MemoryStore};
use reforge::{Porter, Repairer, Run, Start};
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

fn game_env() -> Env {
    env(skins::game(), NoHistory)
}

/// A log whose listener keeps every step it hears, and sets `cancel` at the first step of
/// `stage`.
fn heard(cancel_at: Option<(Stage, Cancel)>) -> (Diagnostics, Arc<Mutex<Vec<Progress>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&events);
    let mut log = Diagnostics::new();
    log.listen_progress(move |progress| {
        kept.lock().expect("one thread").push(progress.clone());
        if let Some((stage, cancel)) = &cancel_at
            && progress.stage == *stage
            && progress.number == 1
        {
            cancel.cancel();
        }
    });
    (log, events)
}

/// The events heard, by stage in the order the stages began. Each stage's events are
/// numbered from 1 in order, with one count, and none of a stage follows another's.
fn by_stage(events: &[Progress]) -> Vec<(Stage, Vec<Step>)> {
    let mut stages: Vec<(Stage, Vec<Step>)> = Vec::new();
    for event in events {
        match stages.last_mut() {
            Some((stage, steps)) if *stage == event.stage => steps.push(event.step.clone()),
            _ => {
                assert!(
                    stages.iter().all(|(stage, _)| *stage != event.stage),
                    "{} began again after another stage",
                    event.stage
                );
                stages.push((event.stage, vec![event.step.clone()]));
            }
        }
    }
    for (stage, _) in &stages {
        let of_stage: Vec<&Progress> = events.iter().filter(|e| e.stage == *stage).collect();
        let count = of_stage[0].count;
        let numbers: Vec<usize> = of_stage.iter().map(|e| e.number).collect();
        assert_eq!(numbers, (1..=numbers.len()).collect::<Vec<_>>(), "{stage}");
        assert!(of_stage.iter().all(|e| e.count == count), "{stage}");
    }
    stages
}

/// The events of a run that ended, each stage's last step numbered its count.
fn whole(events: &[Progress]) -> Vec<(Stage, Vec<Step>)> {
    let stages = by_stage(events);
    for (stage, steps) in &stages {
        let last = events
            .iter()
            .rfind(|e| e.stage == *stage)
            .expect("an event");
        assert_eq!(last.number, last.count, "{stage}: {steps:?}");
    }
    stages
}

fn parts(names: &[&str]) -> Vec<Step> {
    names.iter().map(|name| Step::part(name)).collect()
}

/// The `fix` steps: one per fixup and bin, the bins in one order under each fixup.
fn fixups_over_bins(steps: &[Step], bins: usize) {
    let pairs: Vec<(&str, &str)> = steps
        .iter()
        .map(|step| match step {
            Step::Fixup { fixup, bin } => (fixup.as_str(), bin.as_str()),
            other => panic!("{other:?} is no fixup over a bin"),
        })
        .collect();
    assert_eq!(pairs.len() % bins, 0, "{pairs:?}");
    let first: Vec<&str> = pairs[..bins].iter().map(|(_, bin)| *bin).collect();
    for chunk in pairs.chunks(bins) {
        let fixup = chunk[0].0;
        assert!(chunk.iter().all(|(id, _)| *id == fixup), "{chunk:?}");
        let order: Vec<&str> = chunk.iter().map(|(_, bin)| *bin).collect();
        assert_eq!(order, first);
    }
}

/// How many bins of the mod hold an object: what `fix` walks.
fn bins_with_objects(run: &Run<'_>) -> usize {
    let m = run.module().expect("a mod");
    m.bins
        .values()
        .filter(|bin| !bin.objects.is_empty())
        .count()
}

#[test]
fn the_progress_of_a_port_is_each_stages_steps_in_order_each_counted() {
    let env = game_env();
    let compared = Policy::default().other_levels();
    let porter = Porter::new(&env).compare(compared.clone());
    let (log, events) = heard(None);
    let mut run = porter.run().log_from(log);
    run.read(&own_mod(), FILE_NAME).expect("the archive reads");
    run.through(Stage::Check).expect("no fault");
    let events = events.lock().expect("one thread").clone();
    let stages = whole(&events);

    let order: Vec<Stage> = stages.iter().map(|(stage, _)| *stage).collect();
    assert_eq!(
        order,
        [
            Stage::Read,
            Stage::Load,
            Stage::Fix,
            Stage::Facts,
            Stage::Decide,
            Stage::Plan,
            Stage::Build,
            Stage::Check,
        ]
    );
    let steps = |stage: Stage| &stages.iter().find(|(s, _)| *s == stage).expect("heard").1;

    // The WADs in the order their first entries are read; the raw snapshot sorts them.
    let mut read: Vec<String> = steps(Stage::Read)
        .iter()
        .map(|step| match step {
            Step::Wad { wad } => wad.clone(),
            other => panic!("{other:?} is no WAD"),
        })
        .collect();
    read.sort_by_key(|wad| wad.to_lowercase());
    let raw = run.archive().expect("read").wads.iter();
    let wads: Vec<String> = raw.map(|wad| wad.name.to_string()).collect();
    assert_eq!(read, wads);
    assert_eq!(steps(Stage::Load), &parts(LOAD_PARTS));
    fixups_over_bins(steps(Stage::Fix), bins_with_objects(&run));
    assert_eq!(steps(Stage::Facts), &parts(FACTS_PARTS));
    let policies: Vec<Step> = std::iter::once(Policy::default())
        .chain(compared)
        .map(|policy| Step::Policy {
            policy: policy.label(),
        })
        .collect();
    assert_eq!(steps(Stage::Decide), &policies);
    assert_eq!(steps(Stage::Plan), &parts(PLAN_PARTS));

    let port = run.port().expect("a plan");
    let built = steps(Stage::Build);
    let count = port.units.len() + port.audio.banks.len() + port.files.len();
    assert_eq!(built.len(), count + port.hud.champions.len());
    assert!(matches!(&built[0], Step::Unit { unit } if unit == "test skin0"));
    let files: Vec<Step> = port
        .files
        .values()
        .map(|place| Step::File {
            path: place.to_string(),
        })
        .collect();
    let first_file = port.units.len() + port.audio.banks.len();
    assert_eq!(&built[first_file..count], &files[..]);

    let mut checked = vec![Step::part("patches applied")];
    checked.extend(CheckSet::standard().infos().iter().map(|info| Step::Check {
        check: info.id.to_string(),
    }));
    assert_eq!(steps(Stage::Check), &checked);
}

#[test]
fn the_progress_of_a_repair_is_each_stages_steps_in_order_each_counted() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    let (log, events) = heard(None);
    let mut run = repairer.run().log_from(log);
    run.read(&own_mod(), FILE_NAME).expect("the archive reads");
    run.through(Stage::Check).expect("no fault");
    let events = events.lock().expect("one thread").clone();
    let stages = whole(&events);

    let order: Vec<Stage> = stages.iter().map(|(stage, _)| *stage).collect();
    let kept = [
        Stage::Read,
        Stage::Load,
        Stage::Fix,
        Stage::Build,
        Stage::Check,
    ];
    assert_eq!(order, kept);
    let steps = |stage: Stage| &stages.iter().find(|(s, _)| *s == stage).expect("heard").1;
    assert_eq!(steps(Stage::Load), &parts(&LOAD_PARTS[..6]));
    fixups_over_bins(steps(Stage::Fix), bins_with_objects(&run));
    let m = run.fixed().expect("a fixed mod");
    let built = steps(Stage::Build);
    assert_eq!(built.len(), m.files.len());
    assert!(built.iter().all(|step| matches!(step, Step::File { .. })));
    let checks: Vec<Step> = (CheckSet::standard().infos().iter())
        .map(|info| Step::Check {
            check: info.id.to_string(),
        })
        .collect();
    assert_eq!(steps(Stage::Check), &checks);
}

#[test]
fn a_run_whose_progress_is_heard_reports_as_one_that_is_not() {
    let env = game_env();
    let porter = Porter::new(&env);
    let (log, _) = heard(None);
    let mut heard_run = porter.run().log_from(log);
    let mut plain = porter.run();
    for run in [&mut heard_run, &mut plain] {
        run.read(&own_mod(), FILE_NAME).expect("the archive reads");
        run.through(Stage::Check).expect("no fault");
    }
    assert_eq!(heard_run.report().to_json(), plain.report().to_json());
}

/// The snapshot kept after `stage`, as JSON. The raw snapshot is no JSON: its WADs' names.
fn kept_after(run: &Run<'_>, stage: Stage) -> Option<String> {
    match stage {
        Stage::Read => run
            .archive()
            .map(|raw| format!("{:?}", raw.wads.iter().map(|w| &w.name).collect::<Vec<_>>())),
        Stage::Load => run.load_snapshot().map(to_json),
        Stage::Fix => run.fix_snapshot().map(to_json),
        Stage::Facts => run.facts_snapshot().map(to_json),
        Stage::Decide => run.decide_snapshot().map(to_json),
        Stage::Plan => run.plan_snapshot().map(to_json),
        Stage::Build => run.build_snapshot().map(to_json),
        Stage::Check => run.check_snapshot().map(to_json),
        Stage::Write => None,
    }
}

/// Where a run resumes after `stage`, from what a cancelled run kept.
fn start_after(run: &Run<'_>, stage: Stage) -> Start {
    let snapshots = || -> Box<Snapshots> { Box::new(run.snapshots().expect("the snapshots")) };
    match stage {
        Stage::Read => run.archive().cloned().expect("the raw snapshot").into(),
        Stage::Load => run
            .load_snapshot()
            .cloned()
            .expect("the load snapshot")
            .into(),
        Stage::Fix => Start::Fix(snapshots()),
        Stage::Facts => Start::Facts(snapshots()),
        Stage::Decide => Start::Decide(snapshots()),
        Stage::Plan => Start::Plan(snapshots()),
        Stage::Build => Start::Build(snapshots()),
        Stage::Check | Stage::Write => panic!("no stage runs after {stage}"),
    }
}

/// #ADR-0097: for each stage from `load` on, a cancel set at its first step ends it before
/// its second. The stage has no snapshot, the one before it is the straight run's, the report
/// names the stage, and a run resumed from what it kept writes the straight run's report.
fn cancelled_at_each_stage<'p>(
    run: impl Fn() -> Run<'p>,
    resume: impl Fn(Start, Arc<MemoryStore>) -> Run<'p>,
) {
    let bytes = own_mod();
    let mut straight = run();
    straight.read(&bytes, FILE_NAME).expect("the archive reads");
    straight.through(Stage::Check).expect("no fault");
    let report = straight.report().to_json();
    let stages: Vec<Stage> = straight.stages().to_vec();

    for pair in stages.windows(2) {
        let (before, stage) = (pair[0], pair[1]);
        if stage == Stage::Write {
            continue;
        }
        let cancel = Cancel::new();
        let (log, events) = heard(Some((stage, cancel.clone())));
        let store = Arc::new(MemoryStore::new());
        let mut cancelled = run()
            .log_from(log)
            .cancel(cancel)
            .in_store(Arc::clone(&store));
        cancelled
            .read(&bytes, FILE_NAME)
            .expect("the archive reads");
        cancelled
            .through(Stage::Check)
            .expect("a cancel is no error");

        let events = events.lock().expect("one thread").clone();
        let heard = by_stage(&events);
        let (last, steps) = heard.last().expect("a step was heard");
        assert_eq!(*last, stage, "no stage after {stage} began");
        assert_eq!(steps.len(), 1, "{stage} ended before its second step");
        assert!(events.last().expect("an event").count >= 2, "{stage}");
        assert_eq!(cancelled.cancelled_at(), Some(stage));
        assert_eq!(cancelled.report().cancelled, Some(stage));
        assert!(!cancelled.ran(stage) && kept_after(&cancelled, stage).is_none());
        assert_eq!(
            kept_after(&cancelled, before),
            kept_after(&straight, before),
            "the snapshot after {before} is whole"
        );

        let start = start_after(&cancelled, before);
        for blob in cancelled.into_layer().into_values() {
            store.put(blob);
        }
        let mut resumed = resume(start, store);
        resumed.through(Stage::Check).expect("no fault");
        assert_eq!(resumed.report().to_json(), report, "resumed after {before}");
    }
}

#[test]
fn a_cancel_at_the_first_step_of_each_stage_of_a_port_ends_it_before_the_second() {
    let env = game_env();
    let porter = Porter::new(&env).compare(Policy::default().other_levels());
    cancelled_at_each_stage(
        || porter.run(),
        |from, store| porter.resume(from, store).expect("the run resumes"),
    );
}

#[test]
fn a_cancel_at_the_first_step_of_each_stage_of_a_repair_ends_it_before_the_second() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    cancelled_at_each_stage(
        || repairer.run(),
        |from, store| repairer.resume(from, store).expect("the run resumes"),
    );
}
