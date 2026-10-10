//! A run's cancel (#ADR-0097): read between the steps of each stage, it ends the stage under
//! way after the step it is on. The stage writes no snapshot, the one before it stands, and
//! the report names the stage. The game is `reforge_testkit::skins`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use reforge::diag::{Cancel, Cancelled, DiagnosticPolicy, Sink, Stage};
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::package::{Check, CheckCx, CheckInfo, CheckSet, PackageWriter};
use reforge::snapshot::{Snapshots, to_json};
use reforge::store::{BlobStore, MemoryStore};
use reforge::{Porter, Repairer, Run, RunError, Start};
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

/// A check that sets a cancel when it runs.
struct Cancelling {
    info: CheckInfo,
    cancel: Cancel,
}

impl Check for Cancelling {
    fn info(&self) -> &CheckInfo {
        &self.info
    }

    fn run(&self, _: &CheckCx<'_>, _: &mut Sink<'_>) {
        self.cancel.cancel();
    }
}

/// A check that counts the times it ran.
struct Counted {
    info: CheckInfo,
    ran: Arc<AtomicUsize>,
}

impl Check for Counted {
    fn info(&self) -> &CheckInfo {
        &self.info
    }

    fn run(&self, _: &CheckCx<'_>, _: &mut Sink<'_>) {
        self.ran.fetch_add(1, Ordering::Relaxed);
    }
}

/// Two checks: the first sets `cancel`, the second counts its runs in `ran`.
fn cancelling_checks(cancel: &Cancel, ran: &Arc<AtomicUsize>) -> CheckSet {
    CheckSet::empty()
        .with(Cancelling {
            info: CheckInfo::built_in("cancelling", "Sets the cancel", None),
            cancel: cancel.clone(),
        })
        .with(Counted {
            info: CheckInfo::built_in("counted", "Counts its runs", None),
            ran: Arc::clone(ran),
        })
}

fn flush(run: Run<'_>, store: &MemoryStore) {
    for bytes in run.into_layer().into_values() {
        store.put(bytes);
    }
}

/// #ADR-0097: the first check sets the run's cancel, and the second does not run. `check`
/// writes no snapshot and the report names it; the run's snapshots, flushed, resume after
/// `build` into the run that was not cancelled.
fn cancelled_in_check<'p>(
    cancelled: impl Fn() -> Run<'p>,
    straight: impl Fn() -> Run<'p>,
    resume: impl Fn(Start, Arc<MemoryStore>) -> Run<'p>,
    ran: &AtomicUsize,
) {
    let bytes = own_mod();
    let mut whole = straight();
    whole.read(&bytes, FILE_NAME).expect("the archive reads");
    whole.through(Stage::Check).expect("no fault");
    assert!(whole.ran(Stage::Check) && whole.cancelled_at().is_none());
    let straight_build = to_json(whole.build_snapshot().expect("the build snapshot"));
    let ran_whole = ran.swap(0, Ordering::Relaxed);
    assert_eq!(ran_whole, 1, "both checks run in a run with no cancel");

    let store = Arc::new(MemoryStore::new());
    let mut run = cancelled().in_store(Arc::clone(&store));
    run.read(&bytes, FILE_NAME).expect("the archive reads");
    run.through(Stage::Check).expect("a cancel is no error");
    assert_eq!(
        ran.load(Ordering::Relaxed),
        0,
        "the second check did not run"
    );
    assert_eq!(run.cancelled_at(), Some(Stage::Check));
    assert_eq!(run.halted(), None);
    assert!(run.ran(Stage::Build) && !run.ran(Stage::Check));
    assert!(run.check_snapshot().is_none());
    let build = to_json(run.build_snapshot().expect("the build snapshot stands"));
    assert_eq!(build, straight_build, "the build snapshot is whole");
    assert!(matches!(
        run.check(),
        Err(RunError::Cancelled(Cancelled(Stage::Check)))
    ));
    let report = run.report();
    assert_eq!(report.cancelled, Some(Stage::Check));
    assert_eq!(report.halted, None);
    let json: serde_json::Value = serde_json::from_str(&report.to_json()).expect("JSON");
    assert_eq!(json["cancelled"], "check");
    let writers: [&dyn PackageWriter; 0] = [];
    assert!(matches!(
        run.write(&writers, &DiagnosticPolicy::default()),
        Err(RunError::Cancelled(Cancelled(Stage::Check)))
    ));

    let snapshots: Snapshots = run.snapshots().expect("the snapshots");
    assert!(snapshots.check.is_none());
    flush(run, &store);
    let mut resumed = resume(Start::Build(Box::new(snapshots)), store);
    resumed.through(Stage::Check).expect("no fault");
    assert_eq!(resumed.report().to_json(), whole.report().to_json());
}

#[test]
fn a_cancel_set_in_the_first_check_of_a_port_ends_check_before_the_second() {
    let env = game_env();
    let cancel = Cancel::new();
    let ran = Arc::new(AtomicUsize::new(0));
    let porter = Porter::new(&env)
        .checks(cancelling_checks(&cancel, &ran))
        .cancel(cancel);
    let unread = Porter::new(&env).checks(cancelling_checks(&Cancel::new(), &ran));
    cancelled_in_check(
        || porter.run(),
        || unread.run(),
        |from, store| unread.resume(from, store).expect("the run resumes"),
        &ran,
    );
}

#[test]
fn a_cancel_set_in_the_first_check_of_a_repair_ends_check_before_the_second() {
    let env = game_env();
    let cancel = Cancel::new();
    let ran = Arc::new(AtomicUsize::new(0));
    let repairer = Repairer::new(&env)
        .checks(cancelling_checks(&cancel, &ran))
        .cancel(cancel);
    let unread = Repairer::new(&env).checks(cancelling_checks(&Cancel::new(), &ran));
    cancelled_in_check(
        || repairer.run(),
        || unread.run(),
        |from, store| unread.resume(from, store).expect("the run resumes"),
        &ran,
    );
}

#[test]
fn a_cancel_set_before_the_run_ends_load_before_its_first_step() {
    let env = game_env();
    let cancel = Cancel::new();
    cancel.cancel();
    let porter = Porter::new(&env);
    let mut run = porter.run().cancel(cancel);
    run.read(&own_mod(), FILE_NAME)
        .expect("read reads no cancel");
    run.through(Stage::Check).expect("a cancel is no error");
    assert_eq!(run.cancelled_at(), Some(Stage::Load));
    assert!(run.archive().is_some(), "the raw snapshot stands");
    assert!(run.load_snapshot().is_none() && run.module().is_none());
    assert!(matches!(
        run.fix(),
        Err(RunError::Cancelled(Cancelled(Stage::Load)))
    ));
    assert_eq!(run.report().cancelled, Some(Stage::Load));
}

#[test]
fn a_cancel_never_set_changes_nothing() {
    let env = game_env();
    let bytes = own_mod();
    let porter = Porter::new(&env);
    let repairer = Repairer::new(&env);
    let runs: [(Run<'_>, Run<'_>); 2] = [
        (porter.run(), porter.run().cancel(Cancel::new())),
        (repairer.run(), repairer.run().cancel(Cancel::new())),
    ];
    for (mut plain, mut unset) in runs {
        for run in [&mut plain, &mut unset] {
            run.read(&bytes, FILE_NAME).expect("the archive reads");
            run.through(Stage::Check).expect("no fault");
            assert!(run.ran(Stage::Check) && run.cancelled_at().is_none());
        }
        assert_eq!(unset.report().to_json(), plain.report().to_json());
        let json = |run: &Run<'_>| {
            let snapshots = run.snapshots().expect("the snapshots");
            let check = snapshots.check.as_ref().map(to_json);
            (to_json(&snapshots.load), check)
        };
        assert_eq!(json(&unset), json(&plain));
        let report: serde_json::Value =
            serde_json::from_str(&unset.report().to_json()).expect("JSON");
        assert!(
            report.get("cancelled").is_none(),
            "a run not cancelled names no stage"
        );
    }
}
