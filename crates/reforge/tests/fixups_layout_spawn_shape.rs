//! The fixups of an emitter's spawn shape: the old `shape` as `SpawnShape` and
//! `EmitterPosition`, as 14.1 laid it out, and a converter's birth translation taken out of a
//! `VfxShapeLegacy`.

use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::layout::{BirthTranslation, SpawnShape};
use reforge::fixups::{After, Changed, FixupSet, Lost, Outcome, Removed};
use reforge::game::Schema;
use reforge::model::{FieldPath, name_hash};
use reforge_testkit::bins::{embedded, field_of, float, object, pointer};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{embed, field, list, pointer as to};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const SHAPE_ID: &str = "vfx-spawn-shape";
const MEND_ID: &str = "vfx-spawn-shape-birth-translation";
const EMITTER: &str = "VfxEmitterDefinitionData";
const LEGACY: &str = "VfxShapeLegacy";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";

/// The emitter's `shape` up to build 120, and from 121 its `SpawnShape` and
/// `EmitterPosition`. No build declares a `birthTranslation` on `VfxShapeLegacy`.
fn schema() -> Schema {
    let vector = || embed("ValueVector3");
    SchemaRows::new(300)
        .class(
            EMITTER,
            &[
                field("shape", embed("VfxShape")).until(120),
                field("SpawnShape", to("IVfxShape")).from(121),
                field("EmitterPosition", vector()).from(121),
            ],
        )
        .class(
            "VfxShape",
            &[
                field("birthTranslation", vector()).until(120),
                field("emitOffset", vector()).until(120),
                field("emitRotationAngles", list("Embed")).until(120),
            ],
        )
        .class(
            LEGACY,
            &[
                field("emitOffset", vector()).from(121),
                field("emitRotationAngles", list("Embed")).from(121),
            ],
        )
        .build()
}

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

/// The target at build 300. With `dated`, a history in which the mod's dated file first
/// shipped at build 150.
fn env(dated: bool) -> Env {
    let mut env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema());
    if dated {
        let history = MemoryHistory::new(&["1.0.50", "1.0.150", "16.19.300"]);
        env = env.history(Box::new(history.with_content(DATED, &dated_bytes(), 1..=2)));
    }
    env.build().expect("an environment")
}

fn archive(objects: Vec<BinObject>) -> ModArchive {
    skin_archive(objects).file(skins::WAD, DATED, dated_bytes())
}

/// The two fixups, which the set orders.
fn set() -> FixupSet {
    FixupSet::empty().with(BirthTranslation).with(SpawnShape)
}

fn emitter(entry: &str, fields: Vec<(&str, V)>) -> BinObject {
    object(entry, EMITTER, fields)
}

/// One emitter fixed by the two fixups, for a mod with no date.
fn fixed<'a>(env: &'a Env, fields: Vec<(&str, V)>) -> FixRun<'a> {
    FixRun::of(env, &set(), &archive(vec![emitter("E", fields)]))
}

fn vector(x: f32) -> V {
    embedded("ValueVector3", vec![("constantValue", float(x))])
}

fn constant(value: Option<&V>) -> Option<f32> {
    match field_of(value?, "constantValue")? {
        V::F32(x) => Some(x.value),
        _ => None,
    }
}

/// An old shape with these fields.
fn shape(fields: Vec<(&str, V)>) -> V {
    embedded("VfxShape", fields)
}

/// The path of a field of the struct in a field of the emitter.
fn inside(holder: &str, class: &str, field: &str) -> FieldPath {
    let emitter = FieldPath::root().field(name_hash(EMITTER), name_hash(holder));
    emitter.field(name_hash(class), name_hash(field))
}

fn on_emitter(field: &str) -> FieldPath {
    FieldPath::root().field(name_hash(EMITTER), name_hash(field))
}

#[test]
fn an_old_shape_becomes_a_legacy_spawn_shape_and_a_position() {
    let env = env(false);
    let old = shape(vec![
        ("birthTranslation", vector(1.0)),
        ("emitOffset", vector(2.0)),
    ]);
    let run = fixed(&env, vec![("shape", old)]);
    assert!(run.field("E", "shape").is_none());
    assert_eq!(constant(run.field("E", "EmitterPosition")), Some(1.0));
    let spawn = run.child("E", "SpawnShape");
    assert_eq!(spawn.class_hash, name_hash(LEGACY));
    let offset = spawn.properties.get(&name_hash("emitOffset"));
    assert_eq!(constant(offset), Some(2.0));
    assert!(
        !spawn
            .properties
            .contains_key(&name_hash("birthTranslation"))
    );
    // The spawn shape takes its place before the position.
    let held: Vec<_> = run.object("E").properties.keys().copied().collect();
    assert_eq!(held, ["SpawnShape", "EmitterPosition"].map(name_hash));

    let [offset, translation] = run.changes.of(SHAPE_ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(offset.path, inside("shape", "VfxShape", "emitOffset"));
    let to = inside("SpawnShape", LEGACY, "emitOffset");
    assert_eq!(offset.after, After::Moved { to });
    let from = inside("shape", "VfxShape", "birthTranslation");
    assert_eq!(translation.path, from);
    let to = on_emitter("EmitterPosition");
    assert_eq!(translation.after, After::Moved { to });
    assert_eq!(
        translation.path.spelled(&run.speller()),
        "shape.birthTranslation"
    );
    let [changed] = run.raised::<Changed>(SHAPE_ID)[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!((changed.0.values, changed.0.objects), (2, 1));
    assert!(run.changes.of(MEND_ID).is_empty());
}

#[test]
fn a_shape_of_a_birth_translation_alone_makes_no_spawn_shape() {
    let env = env(false);
    let old = shape(vec![("birthTranslation", vector(1.0))]);
    let run = fixed(&env, vec![("shape", old)]);
    assert_eq!(constant(run.field("E", "EmitterPosition")), Some(1.0));
    assert!(run.field("E", "SpawnShape").is_none());
    assert!(run.field("E", "shape").is_none());
}

#[test]
fn an_empty_shape_is_removed() {
    let env = env(false);
    let run = fixed(&env, vec![("shape", shape(vec![]))]);
    assert!(run.object("E").properties.is_empty());
    let [change] = run.changes.of(SHAPE_ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.path, on_emitter("shape"));
    assert_eq!(change.after, After::Removed);
    assert_eq!(run.raised::<Removed>(SHAPE_ID).len(), 1);
}

#[test]
fn the_spawn_shape_and_the_position_an_emitter_holds_stay() {
    let env = env(false);
    let old = shape(vec![
        ("birthTranslation", vector(1.0)),
        ("emitOffset", vector(2.0)),
    ]);
    let made = pointer(LEGACY, vec![("emitOffset", vector(5.0))]);
    let fields = vec![
        ("shape", old),
        ("SpawnShape", made.clone()),
        ("EmitterPosition", vector(6.0)),
    ];
    let run = fixed(&env, fields);
    assert!(run.field("E", "shape").is_none());
    assert_eq!(run.field("E", "SpawnShape"), Some(&made));
    assert_eq!(constant(run.field("E", "EmitterPosition")), Some(6.0));
    let outcomes: Vec<Outcome> = run.changes.iter().map(|c| c.outcome()).collect();
    assert_eq!(outcomes, [Outcome::Lost, Outcome::Lost]);
    let [lost] = run.raised::<Lost>(SHAPE_ID)[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!(lost.0.values, 2);
}

#[test]
fn a_shape_that_is_no_struct_is_left() {
    let env = env(false);
    let fields = vec![("shape", float(1.0))];
    let run = fixed(&env, fields.clone());
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("E"), &emitter("E", fields));
}

#[test]
fn a_birth_translation_misplaced_in_a_legacy_shape_becomes_the_position() {
    let env = env(false);
    let misplaced = pointer(LEGACY, vec![("birthTranslation", vector(3.0))]);
    let run = fixed(&env, vec![("SpawnShape", misplaced)]);
    assert_eq!(constant(run.field("E", "EmitterPosition")), Some(3.0));
    // The shape stays, with what else it holds: here nothing.
    let spawn = run.child("E", "SpawnShape");
    assert_eq!(spawn.class_hash, name_hash(LEGACY));
    assert!(spawn.properties.is_empty());

    let [change] = run.changes.of(MEND_ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(
        change.path,
        inside("SpawnShape", LEGACY, "birthTranslation")
    );
    let to = on_emitter("EmitterPosition");
    assert_eq!(change.after, After::Moved { to });
    assert_eq!(
        change.path.spelled(&run.speller()),
        "SpawnShape.birthTranslation"
    );
    assert_eq!(run.raised::<Changed>(MEND_ID).len(), 1);
    assert!(run.changes.of(SHAPE_ID).is_empty());
}

#[test]
fn a_misplaced_birth_translation_gives_way_to_the_position_an_emitter_holds() {
    let env = env(false);
    let misplaced = pointer(
        LEGACY,
        vec![
            ("birthTranslation", vector(3.0)),
            ("emitOffset", vector(2.0)),
        ],
    );
    let fields = vec![("SpawnShape", misplaced), ("EmitterPosition", vector(6.0))];
    let run = fixed(&env, fields);
    assert_eq!(constant(run.field("E", "EmitterPosition")), Some(6.0));
    let spawn = run.child("E", "SpawnShape");
    let held: Vec<_> = spawn.properties.keys().copied().collect();
    assert_eq!(held, [name_hash("emitOffset")]);
    // No build read the one in the shape: nothing is lost by it.
    let [change] = run.changes.of(MEND_ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.after, After::Removed);
    assert!(run.raised::<Lost>(MEND_ID).is_empty());
}

#[test]
fn an_emitter_with_a_shape_and_a_converters_shape_takes_its_position_from_the_shape() {
    // The mod's own build read `shape`. The converter's shape is fixed second, by its id.
    let ids = set().ids().join(" ");
    assert_eq!(ids, format!("{SHAPE_ID} {MEND_ID}"));
    let env = env(false);
    let old = shape(vec![("birthTranslation", vector(1.0))]);
    let misplaced = pointer(LEGACY, vec![("birthTranslation", vector(3.0))]);
    let run = fixed(&env, vec![("shape", old), ("SpawnShape", misplaced)]);
    assert_eq!(constant(run.field("E", "EmitterPosition")), Some(1.0));
    assert!(run.child("E", "SpawnShape").properties.is_empty());
    let [moved] = run.changes.of(SHAPE_ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(moved.outcome(), Outcome::Changed);
    let [dropped] = run.changes.of(MEND_ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(dropped.after, After::Removed);
}

#[test]
fn a_layout_the_mods_own_build_did_not_read_is_left() {
    // Made at 150 or later: no build since read `shape`, and none read a birth translation
    // in a `VfxShapeLegacy`. Neither showed.
    let env = env(true);
    let old = shape(vec![
        ("birthTranslation", vector(1.0)),
        ("emitOffset", vector(2.0)),
    ]);
    let misplaced = pointer(LEGACY, vec![("birthTranslation", vector(3.0))]);
    let objects = vec![
        emitter("Old", vec![("shape", old)]),
        emitter("Misplaced", vec![("SpawnShape", misplaced)]),
    ];
    let run = FixRun::of(&env, &set(), &archive(objects.clone()));
    assert_eq!(run.dating.build(), Some(150));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("Old"), &objects[0]);
    assert_eq!(run.object("Misplaced"), &objects[1]);
}

#[test]
fn the_fixups_applied_twice_change_nothing_the_second_time() {
    let old = |x: f32| {
        shape(vec![
            ("birthTranslation", vector(x)),
            ("emitOffset", vector(2.0)),
        ])
    };
    let misplaced = || pointer(LEGACY, vec![("birthTranslation", vector(3.0))]);
    let objects = vec![
        emitter("Old", vec![("shape", old(1.0))]),
        emitter("Empty", vec![("shape", shape(vec![]))]),
        emitter("Misplaced", vec![("SpawnShape", misplaced())]),
        emitter(
            "Both",
            vec![("shape", old(4.0)), ("SpawnShape", misplaced())],
        ),
        emitter(
            "Held",
            vec![
                ("SpawnShape", misplaced()),
                ("EmitterPosition", vector(6.0)),
            ],
        ),
    ];
    let entries = ["Old", "Empty", "Misplaced", "Both", "Held"];
    for dated in [false, true] {
        let env = env(dated);
        let once = FixRun::of(&env, &set(), &archive(objects.clone()));
        assert_eq!(once.changes.is_empty(), dated);
        let after = entries.map(|entry| once.object(entry).clone());
        let twice = once.again(&set());
        assert!(twice.changes.is_empty(), "{:?}", twice.changes);
        assert_eq!(after, entries.map(|entry| twice.object(entry).clone()));
    }
}

#[test]
fn the_fixups_run_where_the_target_reads_the_spawn_shape() {
    // A target of the old layout declares `shape` and no `SpawnShape`.
    let old_target = Env::builder()
        .game(skins::game_at("1.0.110"))
        .schema(schema())
        .build()
        .expect("an environment");
    let old = shape(vec![("birthTranslation", vector(1.0))]);
    let misplaced = pointer(LEGACY, vec![("birthTranslation", vector(3.0))]);
    let objects = vec![
        emitter("Old", vec![("shape", old)]),
        emitter("Misplaced", vec![("SpawnShape", misplaced)]),
    ];
    let run = FixRun::of(&old_target, &set(), &archive(objects.clone()));
    assert!(run.changes.is_empty());
    assert_eq!(run.object("Old"), &objects[0]);
    assert_eq!(run.object("Misplaced"), &objects[1]);
}
