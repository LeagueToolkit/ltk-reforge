//! The origin of a unit: the evidence gathered, and the verdict read from it. The game is
//! `reforge_testkit::skins`.

use reforge::facts::origin::{Aspect, Basis};
use reforge::facts::{Evidence, Origin, OriginEvidence, OriginNone, Verdict, Weight};
use reforge::history::NoHistory;
use reforge::model::{Slot, name_hash};
use reforge_testkit::FactRun;
use reforge_testkit::bins::{Skin, blob, particle};
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, S3_PARTICLE_TEX, S3_TEX};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

fn origin(run: &FactRun<'_>, slot: u16) -> Origin {
    let origin = run.facts.origins.of(run.unit("test", slot));
    origin
        .unwrap_or_else(|| panic!("the unit has an origin\n{}", run.dump()))
        .clone()
}

fn aspects(origin: &Origin) -> Vec<(Aspect, Slot, Weight)> {
    let items = origin.evidence.items.iter();
    items
        .map(|item| (item.aspect, item.slot, item.weight))
        .collect()
}

#[test]
fn a_paste_names_its_premium_skin() {
    let env = env(skins::game(), NoHistory);
    let files = [mod_skin("test", 0, SKIN0, &skins::skin3(), vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let origin = origin(&run, 0);
    assert_eq!(
        (origin.verdict, origin.strength),
        (Verdict::Premium(Slot(3)), Some(Weight::Strong))
    );
    let found = aspects(&origin);
    for aspect in [
        Aspect::Resolver,
        Aspect::Graph,
        Aspect::SkinName,
        Aspect::Mesh,
        Aspect::Skeleton,
        Aspect::Texture,
    ] {
        assert!(
            found.contains(&(aspect, Slot(3), Weight::Strong)),
            "{aspect:?} in {found:?}"
        );
    }
    let resolver = &origin.evidence.items[0];
    let named = Basis::Link {
        entry: name_hash("Characters/Test/Skins/Skin3/Resources"),
    };
    assert_eq!(resolver.basis, named);
    assert!(!origin.evidence.own_mesh);
    assert!(origin.ties.is_empty() && origin.conflicting.is_none());
}

#[test]
fn a_name_of_the_units_own_slot_says_nothing() {
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
    let origin = origin(&run, 0);
    assert!(
        origin.evidence.items.is_empty(),
        "{:?}",
        origin.evidence.items
    );
    assert!(origin.evidence.own_mesh);
    assert_eq!((origin.verdict, origin.strength), (Verdict::Custom, None));
    assert!(run.found::<OriginNone>().is_empty());
}

#[test]
fn another_champions_skin_folder_is_no_evidence() {
    let env = env(skins::game(), NoHistory);
    let mut files = own_files("a");
    let borrowed = "assets/characters/other/skins/skin08/other_skin08.tex";
    let skin = Skin {
        texture: borrowed.into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    files.push((borrowed.into(), blob(borrowed)));
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![own_graph("a", GRAPH0)],
    ));
    let run = FactRun::of(&env, &archive(&files));
    assert_eq!(origin(&run, 0).verdict, Verdict::Custom, "{}", run.dump());
}

#[test]
fn a_texture_under_a_premium_skins_folder_is_evidence_for_it() {
    let env = env(skins::game(), NoHistory);
    let mut files = own_files("a");
    let pasted = "assets/characters/test/skins/skin03/test_skin03_recolor.tex";
    let skin = Skin {
        texture: pasted.into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    files.push((pasted.into(), blob(pasted)));
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![own_graph("a", GRAPH0)],
    ));
    let run = FactRun::of(&env, &archive(&files));
    let origin = origin(&run, 0);
    assert_eq!(
        aspects(&origin),
        [(Aspect::Texture, Slot(3), Weight::Strong)],
        "{}",
        run.dump()
    );
    assert_eq!(
        origin.evidence.items[0].basis,
        Basis::SkinFolder { folder: Slot(3) }
    );
    assert_eq!(origin.verdict, Verdict::Premium(Slot(3)));
}

#[test]
fn borrowed_effects_are_no_evidence_of_a_paste() {
    let env = env(skins::game(), NoHistory);
    // The mod ships skin 3's glow texture, as the game holds it, for a particle of its own.
    let mut files = own_files("a");
    let skin = own_skin("a", GRAPH0, RESOURCES0);
    let glow = particle(
        "Characters/Test/Skins/Skin0/Particles/Glow",
        S3_PARTICLE_TEX,
    );
    files.push((S3_PARTICLE_TEX.into(), blob(S3_PARTICLE_TEX)));
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![own_graph("a", GRAPH0), glow],
    ));
    let run = FactRun::of(&env, &archive(&files));
    assert_eq!(origin(&run, 0).verdict, Verdict::Custom, "{}", run.dump());

    // The same with skin 3's body texture, which is no effects file: weak evidence.
    let mut files = own_files("a");
    let body = particle("Characters/Test/Skins/Skin0/Particles/Glow", S3_TEX);
    files.push((S3_TEX.into(), blob(S3_TEX)));
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &skin,
        vec![own_graph("a", GRAPH0), body],
    ));
    let run = FactRun::of(&env, &archive(&files));
    let origin = origin(&run, 0);
    assert_eq!(
        aspects(&origin),
        [(Aspect::Files, Slot(3), Weight::Weak)],
        "{}",
        run.dump()
    );
    assert_eq!(
        (origin.verdict, origin.strength),
        (Verdict::Premium(Slot(3)), Some(Weight::Weak))
    );
}

fn evidence(items: &[(Aspect, u16, Weight)]) -> OriginEvidence {
    let item = |&(aspect, slot, weight): &(Aspect, u16, Weight)| Evidence {
        aspect,
        slot: Slot(slot),
        weight,
        basis: Basis::SkinFile,
    };
    OriginEvidence {
        items: items.iter().map(item).collect(),
        own_mesh: false,
    }
}

#[test]
fn each_side_wins_only_where_it_leads_strictly() {
    use Aspect::{Files, Mesh, Objects, Skeleton, Texture};
    use Weight::{Strong, Weak};
    let judge = |items: &[(Aspect, u16, Weight)]| Origin::judge(evidence(items));

    let premium = judge(&[
        (Mesh, 3, Strong),
        (Skeleton, 3, Strong),
        (Texture, 0, Strong),
    ]);
    assert_eq!(
        (premium.verdict, premium.strength),
        (Verdict::Premium(Slot(3)), Some(Strong))
    );
    assert_eq!(
        premium.conflicting,
        Some(Slot(3)),
        "strong evidence on both sides"
    );

    let base = judge(&[
        (Mesh, 0, Strong),
        (Skeleton, 0, Strong),
        (Texture, 3, Strong),
    ]);
    assert_eq!(base.verdict, Verdict::Base);

    // A tie on strong evidence is decided by the weak.
    let tied = judge(&[(Mesh, 3, Strong), (Texture, 0, Strong), (Objects, 3, Weak)]);
    assert_eq!(
        (tied.verdict, tied.strength),
        (Verdict::Premium(Slot(3)), Some(Weak))
    );
    let undecided = judge(&[(Mesh, 3, Strong), (Texture, 0, Strong)]);
    assert_eq!(
        (undecided.verdict, undecided.strength),
        (Verdict::Unknown, None)
    );

    // Two premium skins that weigh the same: the lower slot, and the tie is kept.
    let ambiguous = judge(&[(Mesh, 7, Strong), (Skeleton, 3, Strong)]);
    assert_eq!(ambiguous.verdict, Verdict::Premium(Slot(3)));
    assert_eq!(ambiguous.ties, [Slot(3), Slot(7)]);

    let weak_base = judge(&[(Files, 0, Weak)]);
    assert_eq!(
        (weak_base.verdict, weak_base.strength),
        (Verdict::Base, Some(Weak))
    );

    let nothing = Origin::judge(OriginEvidence::default());
    assert_eq!(nothing.verdict, Verdict::Unknown);
    let own = Origin::judge(OriginEvidence {
        items: Vec::new(),
        own_mesh: true,
    });
    assert_eq!(own.verdict, Verdict::Custom);
}
