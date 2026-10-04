//! The origin of a unit: the evidence gathered, and the verdict read from it. The game is
//! `reforge_testkit::skins`.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyKind, PropertyValueEnum};
use reforge::facts::origin::{Aspect, Basis, Tallied};
use reforge::facts::{
    Evidence, ObjectOrigin, Origin, OriginAmbiguous, OriginConflicting, OriginEvidence, OriginNone,
    Verdict, Via, Weight,
};
use reforge::history::NoHistory;
use reforge::model::known::{class, field};
use reforge::model::{Slot, Spelled, name_hash};
use reforge_testkit::bins::{
    Skin, bin, blob, embedded, entry, graph, object, particle, pointer, resolver, resolver_with,
    text,
};
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::skins::{
    self, BASE_CLIPS, BASE_PARTICLE, BASE_PARTICLE_TEX, BASE_TEX, S3_CLIPS, S3_MESH,
    S3_PARTICLE_TEX, S3_SKEL, S3_TEX,
};
use reforge_testkit::sounds::with_audio;
use reforge_testkit::{FactRun, MemoryGame};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const GRAPH3: &str = "Characters/Test/Animations/Skin3";
const RESOURCES3: &str = "Characters/Test/Skins/Skin3/Resources";

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

/// The files of a skin0 mod on a model of its own, whose bin holds `objects`.
fn own_mod(objects: Vec<BinObject>) -> Vec<(String, Vec<u8>)> {
    let mut files = own_files("a");
    files.push((skins::skin_bin("test", 0), bin(objects)));
    files
}

/// The entry of a skin0 mod on a model of its own.
fn own_entry() -> BinObject {
    entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0))
}

// #MOD-ARTIFACT-0002: the signs by name.
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
    let mostly = Basis::Mostly {
        what: Tallied::Files,
        count: 1,
        base: 0,
    };
    assert_eq!(origin.evidence.items[0].basis, mostly);
    assert_eq!(
        (origin.verdict, origin.strength),
        (Verdict::Premium(Slot(3)), Some(Weight::Weak))
    );
}

#[test]
fn an_audio_tag_of_a_premium_skin_is_evidence_for_it() {
    let env = env(skins::game(), NoHistory);
    // A tag of the unit's own slot and one of a skin the game does not have say nothing.
    let tags = ["Test", "TestSkin00", "TestSkin55", "TestSkin03"];
    let objects = vec![with_audio(own_entry(), &tags, &[]), own_graph("a", GRAPH0)];
    let run = FactRun::of(&env, &archive(&own_mod(objects)));
    let origin = origin(&run, 0);
    assert_eq!(
        aspects(&origin),
        [(Aspect::AudioTag, Slot(3), Weight::Strong)],
        "{}",
        run.dump()
    );
    let named = Basis::Name {
        text: Spelled::new("TestSkin03"),
    };
    assert_eq!(origin.evidence.items[0].basis, named);
    assert_eq!(origin.verdict, Verdict::Premium(Slot(3)));
}

/// The game with a texture of skin 7's own, which leaves the base's texture to the base.
fn game_with_the_base_texture_apart() -> MemoryGame {
    let mut game = skins::game();
    let skin7 = Skin {
        texture: "assets/characters/test/skins/skin07/test_skin07.tex".into(),
        ..skins::skin7()
    };
    let objects = vec![
        entry("Characters/Test/Skins/Skin7", &skin7),
        resolver("Characters/Test/Skins/Skin7/Resources"),
    ];
    game.insert(&skins::skin_bin("test", 7), bin(objects));
    game
}

// #MOD-ARTIFACT-0002: a file of the base that premium skins also use, and the mod's own bytes
// at the base's path, say nothing.
#[test]
fn the_bases_file_is_evidence_for_the_base_where_it_is_the_bases_alone() {
    let skin = Skin {
        texture: BASE_TEX.into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    let objects = || vec![entry(SKIN0, &skin), own_graph("a", GRAPH0)];

    let env = env(game_with_the_base_texture_apart(), NoHistory);
    let run = FactRun::of(&env, &archive(&own_mod(objects())));
    let origin = origin(&run, 0);
    assert_eq!(
        aspects(&origin),
        [(Aspect::Texture, Slot(0), Weight::Strong)],
        "{}",
        run.dump()
    );
    assert_eq!(origin.evidence.items[0].basis, Basis::BaseFile);
    assert_eq!(
        (origin.verdict, origin.strength),
        (Verdict::Base, Some(Weight::Strong))
    );

    // The mod's own bytes at the path are where any base-skin mod puts its texture.
    let mut files = own_mod(objects());
    files.push((BASE_TEX.into(), blob("the mod's texture")));
    let run = FactRun::of(&env, &archive(&files));
    assert_eq!(
        self::origin(&run, 0).verdict,
        Verdict::Custom,
        "{}",
        run.dump()
    );

    // In the game where skin 7 is on the base's texture, the texture is no one skin's.
    let env = reforge_testkit::facts::env(skins::game(), NoHistory);
    let run = FactRun::of(&env, &archive(&own_mod(objects())));
    assert_eq!(
        self::origin(&run, 0).verdict,
        Verdict::Custom,
        "{}",
        run.dump()
    );
}

/// A graph whose clip map holds these clips, each a name and the animation file it plays.
fn clip_graph(entry_name: &str, clips: &[(&str, &str)]) -> BinObject {
    let clip = |(name, file): &(&str, &str)| {
        let file = vec![(field::ANIMATION_FILE_PATH, text(file))];
        let resource = embedded("AnimationResourceData", file);
        let key = PropertyValueEnum::Hash(values::Hash::new(name_hash(name)));
        let fields = vec![(field::ANIMATION_RESOURCE_DATA, resource)];
        (key, pointer(class::ATOMIC_CLIP_DATA, fields))
    };
    let clips = clips.iter().map(clip).collect();
    let map = values::Map::new(PropertyKind::Hash, PropertyKind::Struct, clips)
        .unwrap_or_else(|error| panic!("a map of clips does not build: {error}"));
    let map = PropertyValueEnum::Map(map);
    let fields = vec![(field::CLIP_DATA_MAP, map)];
    object(entry_name, class::ANIMATION_GRAPH_DATA, fields)
}

fn particle_of(skin: u16, key: &str) -> String {
    format!("Characters/Test/Skins/Skin{skin}/Particles/{key}")
}

/// A resolver of a skin's particles under these keys.
fn resolver_of(entry_name: &str, skin: u16, keys: &[&str]) -> BinObject {
    let targets: Vec<String> = keys.iter().map(|key| particle_of(skin, key)).collect();
    let keys: Vec<(&str, &str)> = keys
        .iter()
        .copied()
        .zip(targets.iter().map(String::as_str))
        .collect();
    resolver_with(entry_name, &keys)
}

/// The game with clip maps on the graphs of the base and of skin 3, and keys on their
/// resolvers. Skin 7 plays the base's graph and has a resolver with no key.
fn mapped_game() -> MemoryGame {
    let mut game = skins::game();
    let base = vec![
        entry(SKIN0, &skins::base_skin()),
        clip_graph(GRAPH0, &[("Idle", BASE_CLIPS[0]), ("Run", BASE_CLIPS[1])]),
        resolver_of(RESOURCES0, 0, &["Q", "W"]),
    ];
    game.insert(&skins::skin_bin("test", 0), bin(base));
    let skin3 = vec![
        entry("Characters/Test/Skins/Skin3", &skins::skin3()),
        clip_graph(GRAPH3, &[("Idle", S3_CLIPS[0]), ("Recall", S3_CLIPS[1])]),
        resolver_of(RESOURCES3, 3, &["Q", "R"]),
    ];
    game.insert(&skins::skin_bin("test", 3), bin(skin3));
    game
}

// #MOD-ARTIFACT-0002: fewer edits against skin N's graph than against any other skin's.
#[test]
fn a_graph_closest_to_one_skins_is_evidence_for_it() {
    let env = env(mapped_game(), NoHistory);
    // Skin 3's two clips and a dance of the mod's own, under the base's name.
    let clips = [
        ("Idle", S3_CLIPS[0]),
        ("Recall", S3_CLIPS[1]),
        ("Dance", "assets/mod/a/dance.anm"),
    ];
    let objects = vec![own_entry(), clip_graph(GRAPH0, &clips)];
    let run = FactRun::of(&env, &archive(&own_mod(objects)));
    let origin = origin(&run, 0);
    assert_eq!(
        aspects(&origin),
        [(Aspect::GraphCloseness, Slot(3), Weight::Strong)],
        "{}",
        run.dump()
    );
    // Against the base's: two clips added and one removed. `Idle` under another file is no
    // edit.
    let closer = Basis::Closer {
        edits: 1,
        base: Some(3),
    };
    assert_eq!(origin.evidence.items[0].basis, closer);
    assert_eq!(
        (origin.verdict, origin.strength),
        (Verdict::Premium(Slot(3)), Some(Weight::Strong))
    );
}

// #MOD-ARTIFACT-0002: fewer edits against skin N's resolver than against any other skin's.
#[test]
fn a_resolver_closest_to_one_skins_is_evidence_for_it() {
    let env = env(mapped_game(), NoHistory);
    let origin_with = |resolver: BinObject| {
        let objects = vec![own_entry(), own_graph("a", GRAPH0), resolver];
        let run = FactRun::of(&env, &archive(&own_mod(objects)));
        origin(&run, 0)
    };

    // Skin 3's keys and one more.
    let pasted = origin_with(resolver_of(RESOURCES0, 3, &["Q", "R", "E"]));
    assert_eq!(
        aspects(&pasted),
        [(Aspect::ResolverCloseness, Slot(3), Weight::Strong)]
    );
    let closer = Basis::Closer {
        edits: 1,
        base: Some(4),
    };
    assert_eq!(pasted.evidence.items[0].basis, closer);
    assert_eq!(pasted.verdict, Verdict::Premium(Slot(3)));

    // The base's keys and one more.
    let based = origin_with(resolver_of(RESOURCES0, 0, &["Q", "W", "E"]));
    assert_eq!(
        aspects(&based),
        [(Aspect::ResolverCloseness, Slot(0), Weight::Strong)]
    );
    let closer = Basis::Closer {
        edits: 1,
        base: Some(1),
    };
    assert_eq!(based.evidence.items[0].basis, closer);
    assert_eq!(based.verdict, Verdict::Base);

    // One key of its own is as far from the one as from the other: no skin is closer.
    let apart = origin_with(resolver_of(RESOURCES0, 0, &["E"]));
    assert!(apart.evidence.items.is_empty(), "{:?}", apart.evidence);
    assert_eq!(apart.verdict, Verdict::Custom);
}

// #MOD-ARTIFACT-0002: skin 3's objects saved in the base's bin, one of them under the base's
// name.
#[test]
fn game_objects_mostly_of_one_premium_skin_are_weak_evidence_for_it() {
    let env = env(skins::game(), NoHistory);
    let objects = vec![
        own_entry(),
        graph(GRAPH0, S3_CLIPS),
        graph(GRAPH3, S3_CLIPS),
        particle(BASE_PARTICLE, BASE_PARTICLE_TEX),
    ];
    let run = FactRun::of(&env, &archive(&own_mod(objects)));
    let renamed = run.facts.objects.origin(run.fix.id(GRAPH0));
    let pasted = ObjectOrigin::GameRenamed {
        entry: name_hash(GRAPH3),
        slot: Some(Slot(3)),
        via: Via::Install,
    };
    assert_eq!(renamed, Some(&pasted), "{}", run.dump());

    let origin = origin(&run, 0);
    assert_eq!(
        aspects(&origin),
        [(Aspect::Objects, Slot(3), Weight::Weak)],
        "{}",
        run.dump()
    );
    let mostly = Basis::Mostly {
        what: Tallied::Objects,
        count: 2,
        base: 1,
    };
    assert_eq!(origin.evidence.items[0].basis, mostly);
    assert_eq!(
        (origin.verdict, origin.strength),
        (Verdict::Premium(Slot(3)), Some(Weight::Weak))
    );
}

#[test]
fn game_objects_of_the_base_alone_are_weak_evidence_for_the_base() {
    let env = env(skins::game(), NoHistory);
    let objects = vec![
        own_entry(),
        own_graph("a", GRAPH0),
        particle(BASE_PARTICLE, BASE_PARTICLE_TEX),
    ];
    let run = FactRun::of(&env, &archive(&own_mod(objects)));
    let origin = origin(&run, 0);
    assert_eq!(
        aspects(&origin),
        [(Aspect::Objects, Slot(0), Weight::Weak)],
        "{}",
        run.dump()
    );
    let base_only = Basis::BaseOnly {
        what: Tallied::Objects,
        count: 1,
    };
    assert_eq!(origin.evidence.items[0].basis, base_only);
    assert_eq!(
        (origin.verdict, origin.strength),
        (Verdict::Base, Some(Weight::Weak))
    );
}

#[test]
fn premium_skins_that_weigh_the_same_are_reported_and_the_lower_slot_taken() {
    let env = env(skins::game(), NoHistory);
    // Skin 3's mesh under skin 7's resolver.
    let skin = Skin {
        mesh: S3_MESH.into(),
        resolver: "Characters/Test/Skins/Skin7/Resources".into(),
        ..skins::base_skin()
    };
    let files = [mod_skin("test", 0, SKIN0, &skin, vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let origin = origin(&run, 0);
    assert_eq!(
        aspects(&origin),
        [
            (Aspect::Resolver, Slot(7), Weight::Strong),
            (Aspect::Mesh, Slot(3), Weight::Strong)
        ],
        "{}",
        run.dump()
    );
    assert_eq!(origin.verdict, Verdict::Premium(Slot(3)));
    let ambiguous: Vec<OriginAmbiguous> = run.found();
    assert_eq!(ambiguous.len(), 1);
    assert_eq!(
        (ambiguous[0].unit, ambiguous[0].slots.as_slice()),
        (run.unit("test", 0), &[Slot(3), Slot(7)][..])
    );
    assert!(run.found::<OriginConflicting>().is_empty());
    assert!(run.found::<OriginNone>().is_empty());
}

#[test]
fn strong_evidence_for_the_base_beside_a_premium_skins_is_reported() {
    let env = env(game_with_the_base_texture_apart(), NoHistory);
    // Skin 3's mesh and skeleton under the base's texture.
    let skin = Skin {
        mesh: S3_MESH.into(),
        skeleton: S3_SKEL.into(),
        ..skins::base_skin()
    };
    let files = [mod_skin("test", 0, SKIN0, &skin, vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let origin = origin(&run, 0);
    assert_eq!(
        (origin.verdict, origin.strength),
        (Verdict::Premium(Slot(3)), Some(Weight::Strong)),
        "{}",
        run.dump()
    );
    let conflicting: Vec<OriginConflicting> = run.found();
    assert_eq!(conflicting.len(), 1);
    assert_eq!(
        (conflicting[0].unit, conflicting[0].slot),
        (run.unit("test", 0), Slot(3))
    );
    assert!(run.found::<OriginAmbiguous>().is_empty());
}

#[test]
fn a_unit_no_evidence_decides_is_reported() {
    let env = env(skins::game(), NoHistory);
    // The base's model, which skin 7 is on too, under a name no skin of the game has.
    let skin = Skin {
        name: "Mine".into(),
        ..skins::base_skin()
    };
    let files = [mod_skin("test", 0, SKIN0, &skin, vec![])];
    let run = FactRun::of(&env, &archive(&files));
    let origin = origin(&run, 0);
    assert!(origin.evidence.items.is_empty(), "{}", run.dump());
    assert_eq!((origin.verdict, origin.strength), (Verdict::Unknown, None));
    let none: Vec<OriginNone> = run.found();
    assert_eq!(none.len(), 1);
    assert_eq!(none[0].unit, run.unit("test", 0));
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
