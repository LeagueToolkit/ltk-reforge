//! Copies among a mod's skin units (#MOD-ARTIFACT-0001), and whose its objects are. The game
//! is `reforge_testkit::skins`.

use reforge::diag::Diag;
use reforge::facts::{CopiesFound, CopyKind, DistinctUnits, NearCopy, ObjectOrigin, UnitRole, Via};
use reforge::history::NoHistory;
use reforge::model::{Slot, name_hash};
use reforge::port::{AnimReuse, Policy, TargetPolicy};
use reforge_testkit::bins::{particle, resolver};
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, BASE_PARTICLE, BASE_PARTICLE_TEX};
use reforge_testkit::{FactRun, MemoryHistory};

/// One model of the mod's own, with its bin written to each slot under the slot's names.
fn spread(
    slots: &[u16],
    extra: impl Fn(u16) -> Vec<ltk_meta::BinObject>,
) -> Vec<(String, Vec<u8>)> {
    let mut files = own_files("a");
    for &slot in slots {
        let graph = format!("Characters/Test/Animations/Skin{slot}");
        let resources = format!("Characters/Test/Skins/Skin{slot}/Resources");
        let skin = own_skin("a", &graph, &resources);
        let mut objects = vec![own_graph("a", &graph), resolver(&resources)];
        objects.extend(extra(slot));
        files.push(mod_skin(
            "test",
            slot,
            &format!("Characters/Test/Skins/Skin{slot}"),
            &skin,
            objects,
        ));
    }
    files
}

#[test]
fn copies_at_other_slots_collapse_into_skin0() {
    let env = env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&spread(&[0, 1, 2, 9], |_| Vec::new())));
    let copies = &run.facts.copies;
    let representatives: Vec<_> = copies.representatives().collect();
    assert_eq!(representatives, [run.unit("test", 0)], "{}", run.dump());
    let Some(UnitRole::Representative { members, spread }) = copies.role(representatives[0]) else {
        panic!("skin0 is the representative");
    };
    assert!(*spread, "the cluster covers skins other than the base");
    assert!(
        members
            .iter()
            .all(|member| member.kind == CopyKind::Renamed),
        "{members:?}"
    );
    assert_eq!(
        copies.copy_of(run.unit("test", 9)),
        Some(representatives[0])
    );

    let found: Vec<CopiesFound> = run.found();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].slots, [Slot(1), Slot(2), Slot(9)]);
    assert!(run.found::<DistinctUnits>().is_empty());
    let policy = Policy {
        target: TargetPolicy::Auto,
        anim_reuse: AnimReuse::Block,
    };
    assert_eq!(
        policy.decide(&run.facts).decisions.len(),
        1,
        "a copy has no decision"
    );
}

#[test]
fn another_build_of_the_work_is_a_near_copy() {
    let env = env(skins::game(), NoHistory);
    // The copy at slot 1 holds one particle the others lack, among ten they share.
    let shared = |slot: u16| {
        let particles = (0..10).map(|i| {
            particle(
                &format!("Characters/Test/Particles/P{i}"),
                "assets/mod/a/body.tex",
            )
        });
        let odd = particle("Characters/Test/Particles/Odd", "assets/mod/a/body.tex");
        particles.chain((slot == 1).then_some(odd)).collect()
    };
    let run = FactRun::of(&env, &archive(&spread(&[0, 1], shared)));
    let kind = match run.facts.copies.role(run.unit("test", 1)) {
        Some(UnitRole::Copy { kind, .. }) => *kind,
        other => panic!("skin1 is a copy: {other:?}\n{}", run.dump()),
    };
    assert_eq!(
        kind,
        CopyKind::Near {
            shared: 13,
            differing: 1
        }
    );
    let near: Vec<NearCopy> = run.found();
    assert_eq!(near.len(), 1);
    assert_eq!(
        (near[0].of_slot, near[0].shared, near[0].differing),
        (Slot(0), 13, 1)
    );
}

#[test]
fn two_models_in_one_character_are_two_units() {
    let env = env(skins::game(), NoHistory);
    let mut files = own_files("a");
    files.extend(own_files("b"));
    for (prefix, slot) in [("a", 0u16), ("b", 3)] {
        let graph = format!("Characters/Test/Animations/Skin{slot}");
        let skin = own_skin(
            prefix,
            &graph,
            &format!("Characters/Test/Skins/Skin{slot}/Resources"),
        );
        files.push(mod_skin(
            "test",
            slot,
            &format!("Characters/Test/Skins/Skin{slot}"),
            &skin,
            vec![own_graph(prefix, &graph)],
        ));
    }
    let run = FactRun::of(&env, &archive(&files));
    assert_eq!(
        run.facts.copies.representatives().count(),
        2,
        "{}",
        run.dump()
    );
    assert!(!run.facts.copies.is_spread(run.unit("test", 0)));
    let distinct: Vec<DistinctUnits> = run.found();
    assert_eq!(distinct.len(), 1);
    assert_eq!(distinct[0].units, 2);
    assert_eq!(DistinctUnits::CODE.name, "copies.distinct-units");
}

#[test]
fn a_game_object_is_told_from_the_mods_own() {
    let own = particle(
        "Characters/Test/Skins/Skin0/Particles/Own",
        "assets/mod/a/body.tex",
    );
    // The base's particle as the game holds it, and again under another name.
    let exact = particle(BASE_PARTICLE, BASE_PARTICLE_TEX);
    let mut renamed = particle(BASE_PARTICLE, BASE_PARTICLE_TEX);
    renamed.path_hash = name_hash("Characters/Test/Skins/Skin5/Particles/Test_Base_Idle");
    let env = env(skins::game(), MemoryHistory::new(&["16.18.1", "16.19.1"]));
    let mut files = own_files("a");
    let skin = own_skin(
        "a",
        "Characters/Test/Animations/Skin0",
        "Characters/Test/Skins/Skin0/Resources",
    );
    files.push(mod_skin(
        "test",
        0,
        "Characters/Test/Skins/Skin0",
        &skin,
        vec![own, exact, renamed],
    ));
    let run = FactRun::of(&env, &archive(&files));
    let origin = |entry: &str| {
        let m = &run.fix.fixed;
        let (id, _) = m
            .objects
            .iter()
            .find(|(_, o)| o.entry == name_hash(entry))
            .expect("the object is loaded");
        *run.facts
            .objects
            .origin(id)
            .expect("every loaded object is judged")
    };
    assert_eq!(
        origin("Characters/Test/Skins/Skin0/Particles/Own"),
        ObjectOrigin::Own,
        "{}",
        run.dump()
    );
    let base = ObjectOrigin::GameExact {
        entry: name_hash(BASE_PARTICLE),
        slot: Some(Slot(0)),
        via: Via::Install,
    };
    assert_eq!(origin(BASE_PARTICLE), base);
    let pasted = ObjectOrigin::GameRenamed {
        entry: name_hash(BASE_PARTICLE),
        slot: Some(Slot(0)),
        via: Via::Install,
    };
    assert_eq!(
        origin("Characters/Test/Skins/Skin5/Particles/Test_Base_Idle"),
        pasted
    );
}
