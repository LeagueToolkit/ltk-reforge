//! `Settings` as data (#ADR-0094): each stage declares the settings it reads, and two runs'
//! settings differ first at the stage a rerun resumes from.

use reforge::archive::Caps;
use reforge::diag::Stage;
use reforge::env::Env;
use reforge::fixups::{FixupSelection, FixupSet, Rule};
use reforge::history::NoHistory;
use reforge::load::ReworkTable;
use reforge::package::CheckSet;
use reforge::port::{AnimReuse, Policy, TargetPolicy};
use reforge::snapshot::StageSettings;
use reforge::{Porter, Repairer};
use reforge_testkit::facts::env;
use reforge_testkit::skins;

fn game_env() -> Env {
    env(skins::game(), NoHistory)
}

const BASE: Policy = Policy {
    target: TargetPolicy::Base,
    anim_reuse: AnimReuse::Allow,
};

/// The earliest stage at which a port under `changed` reads other settings than the standard.
fn earliest(env: &Env, changed: impl Fn(Porter<'_>) -> Porter<'_>) -> Option<Stage> {
    let standard = Porter::new(env);
    let other = changed(Porter::new(env));
    standard.settings().earliest_changed(other.settings())
}

#[test]
fn a_port_under_the_same_settings_changes_at_no_stage() {
    let env = game_env();
    assert_eq!(earliest(&env, |porter| porter), None);
}

#[test]
fn each_setting_changes_first_at_the_earliest_stage_that_reads_it() {
    let env = game_env();
    let caps = Caps {
        entries: 1 << 10,
        ..Caps::default()
    };
    assert_eq!(earliest(&env, |p| p.caps(caps)), Some(Stage::Read));
    assert_eq!(earliest(&env, |p| p.keep_stale(true)), Some(Stage::Load));
    let reworks = ReworkTable::new([("test", "16.19")]);
    assert_eq!(
        earliest(&env, |p| p.reworks(reworks.clone())),
        Some(Stage::Load)
    );
    let files = FixupSelection::standard().without("legacy-formats");
    let files = FixupSet::from(&files);
    assert_eq!(
        earliest(&env, |p| p.fixups(files.clone())),
        Some(Stage::Load)
    );
    let bins = FixupSelection::standard().without("known-artifacts-value-color");
    let bins = FixupSet::from(&bins);
    assert_eq!(earliest(&env, |p| p.fixups(bins.clone())), Some(Stage::Fix));
    // A rule under a built-in rule's id (`rules::filtering`): the same ids, other data.
    let junk = Rule::remove_fields("Material", ["junk"]).id("vfx-filtering");
    let rule = FixupSelection::standard().rule(junk);
    let rule = FixupSet::from(&rule);
    assert_eq!(earliest(&env, |p| p.fixups(rule.clone())), Some(Stage::Fix));
    assert_eq!(earliest(&env, |p| p.policy(BASE)), Some(Stage::Decide));
    assert_eq!(
        earliest(&env, |p| p.compare(vec![BASE])),
        Some(Stage::Decide)
    );
    assert_eq!(
        earliest(&env, |p| p.game_events_only(true)),
        Some(Stage::Plan)
    );
    assert_eq!(
        earliest(&env, |p| p.keep_other_slots(true)),
        Some(Stage::Plan)
    );
    assert_eq!(earliest(&env, |p| p.keep_dds(true)), Some(Stage::Build));
    let checks = CheckSet::empty();
    assert_eq!(
        earliest(&env, |p| p.checks(checks.clone())),
        Some(Stage::Check)
    );
}

#[test]
fn a_repair_and_a_port_differ_first_at_load() {
    let env = game_env();
    let (porter, repairer) = (Porter::new(&env), Repairer::new(&env));
    let earliest = repairer.settings().earliest_changed(porter.settings());
    assert_eq!(earliest, Some(Stage::Load));
}

#[test]
fn a_repairs_choice_of_dds_changes_first_at_build() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    let dds = Repairer::new(&env).keep_dds(true);
    let earliest = repairer.settings().earliest_changed(dds.settings());
    assert_eq!(earliest, Some(Stage::Build));
}

#[test]
fn each_stage_declares_the_settings_its_snapshot_holds() {
    let env = game_env();
    let porter = Porter::new(&env).policy(BASE);
    let StageSettings::Decide(decide) = porter.settings().declared(Stage::Decide) else {
        panic!("decide declares the decide settings");
    };
    assert_eq!(decide.policy, BASE);
    let StageSettings::Read(caps) = porter.settings().declared(Stage::Read) else {
        panic!("read declares the caps");
    };
    assert_eq!(caps, Caps::default());
    assert_eq!(
        porter.settings().declared(Stage::Write),
        StageSettings::Write
    );
}
