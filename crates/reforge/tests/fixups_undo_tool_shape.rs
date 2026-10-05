//! A tool's emitter shape (#MOD-ARTIFACT-0031): a shape of a class of 14.1 under the old
//! `shape`, and a point's offset under `constantValue`, as the `SpawnShape` the target reads.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::layout::SpawnShape;
use reforge::fixups::undo::ToolShape;
use reforge::fixups::{After, Changed, FixupSet, Outcome, Removed};
use reforge::game::Schema;
use reforge::model::{BinHash, FieldPath, name_hash};
use reforge_testkit::bins::{embedded, float, object, pointer};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{embed, field, kind, list, pointer as to};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const ID: &str = "vfx-tool-shape";
const SHAPE_ID: &str = "vfx-spawn-shape";
const EMITTER: &str = "VfxEmitterDefinitionData";
const LEGACY: &str = "VfxShapeLegacy";
const POINT: &str = "VfxShapePointDoNotUse";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";

/// The emitter's `shape` up to build 120, and from 121 its `SpawnShape` and
/// `EmitterPosition`, as 14.1 laid them out.
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
            ],
        )
        .class(
            LEGACY,
            &[
                field("emitOffset", vector()).from(121),
                field("emitRotationAngles", list("Embed")).from(121),
            ],
        )
        .class(POINT, &[field("emitOffset", kind("Vec3")).from(121)])
        .build()
}

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

/// The target at build 300. With `made`, a history in which the mod's dated file first
/// shipped at that build.
fn env(made: Option<u64>) -> Env {
    let mut env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema());
    if let Some(made) = made {
        let builds = [
            "1.0.50".to_owned(),
            format!("1.0.{made}"),
            "16.19.300".into(),
        ];
        let builds: Vec<&str> = builds.iter().map(String::as_str).collect();
        let history = MemoryHistory::new(&builds);
        env = env.history(Box::new(history.with_content(DATED, &dated_bytes(), 1..=2)));
    }
    env.build().expect("an environment")
}

fn archive(objects: Vec<BinObject>) -> ModArchive {
    skin_archive(objects).file(skins::WAD, DATED, dated_bytes())
}

fn set() -> FixupSet {
    FixupSet::empty().with(ToolShape)
}

fn emitter(entry: &str, fields: Vec<(&str, V)>) -> BinObject {
    object(entry, EMITTER, fields)
}

fn fixed<'a>(env: &'a Env, fields: Vec<(&str, V)>) -> FixRun<'a> {
    FixRun::of(env, &set(), &archive(vec![emitter("E", fields)]))
}

fn vector(x: f32) -> V {
    embedded("ValueVector3", vec![("constantValue", float(x))])
}

fn vec3(x: f32, y: f32, z: f32) -> V {
    V::Vector3(values::Vector3::new(glam::Vec3::new(x, y, z)))
}

/// Riot's point shape, as `SpawnShape` holds it from 14.1.
fn point(x: f32) -> V {
    pointer(POINT, vec![("emitOffset", vec3(x, 2.0, 3.0))])
}

fn on_emitter(field: &str) -> FieldPath {
    FieldPath::root().field(name_hash(EMITTER), name_hash(field))
}

fn inside(holder: &str, class: &str, field: &str) -> FieldPath {
    on_emitter(holder).field(name_hash(class), name_hash(field))
}

#[test]
fn the_point_class_is_the_one_tools_write_by_its_hash() {
    assert_eq!(name_hash(POINT), BinHash(0xee39_916f));
}

#[test]
fn a_legacy_shape_under_shape_becomes_the_spawn_shape() {
    let env = env(None);
    let renamed = embedded(
        LEGACY,
        vec![
            ("emitOffset", vector(2.0)),
            ("birthTranslation", embedded("ValueVector3", vec![])),
            ("emitRotationAngles", float(9.0)),
        ],
    );
    let fields = vec![("shape", renamed), ("EmitterPosition", vector(1.0))];
    let run = fixed(&env, fields);
    assert!(run.field("E", "shape").is_none());
    // A pointer, where the tool kept the embed of the old field.
    let spawn = run.child("E", "SpawnShape");
    assert!(matches!(run.field("E", "SpawnShape"), Some(V::Struct(_))));
    assert_eq!(spawn.class_hash, name_hash(LEGACY));
    let held: Vec<_> = spawn.properties.keys().copied().collect();
    assert_eq!(held, ["emitOffset", "emitRotationAngles"].map(name_hash));
    assert_eq!(run.field("E", "EmitterPosition"), Some(&vector(1.0)));

    let [offset, angles, translation] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(offset.path, inside("shape", LEGACY, "emitOffset"));
    let to = inside("SpawnShape", LEGACY, "emitOffset");
    assert_eq!(offset.after, After::Moved { to });
    assert_eq!(angles.outcome(), Outcome::Changed);
    // The tool emptied the translation it gave the position.
    assert_eq!(
        translation.path,
        inside("shape", LEGACY, "birthTranslation")
    );
    assert_eq!(translation.after, After::Removed);
    assert_eq!(offset.path.spelled(&run.speller()), "shape.emitOffset");
    let [changed] = run.raised::<Changed>(ID)[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!((changed.0.values, changed.0.objects), (2, 1));
    assert_eq!(run.raised::<Removed>(ID).len(), 1);
}

#[test]
fn a_birth_translation_the_tool_left_whole_becomes_the_position() {
    let env = env(None);
    let renamed = embedded(LEGACY, vec![("birthTranslation", vector(3.0))]);
    let run = fixed(&env, vec![("shape", renamed)]);
    assert_eq!(run.field("E", "EmitterPosition"), Some(&vector(3.0)));
    assert!(run.field("E", "SpawnShape").is_none());
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    let to = on_emitter("EmitterPosition");
    assert_eq!(change.after, After::Moved { to });
}

#[test]
fn a_point_under_shape_becomes_the_spawn_shape() {
    let env = env(None);
    let renamed = embedded(POINT, vec![("emitOffset", vec3(1.0, 2.0, 3.0))]);
    let run = fixed(&env, vec![("shape", renamed)]);
    assert!(run.field("E", "shape").is_none());
    assert_eq!(run.field("E", "SpawnShape"), Some(&point(1.0)));
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.path, inside("shape", POINT, "emitOffset"));
    let to = inside("SpawnShape", POINT, "emitOffset");
    assert_eq!(change.after, After::Moved { to });
}

#[test]
fn a_points_offset_under_constant_value_becomes_its_emit_offset() {
    let env = env(None);
    let misplaced = pointer(POINT, vec![("constantValue", vec3(-155.0, 450.0, 0.0))]);
    let run = fixed(&env, vec![("SpawnShape", misplaced)]);
    let mended = pointer(POINT, vec![("emitOffset", vec3(-155.0, 450.0, 0.0))]);
    assert_eq!(run.field("E", "SpawnShape"), Some(&mended));
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.path, inside("SpawnShape", POINT, "constantValue"));
    let to = inside("SpawnShape", POINT, "emitOffset");
    assert_eq!(change.after, After::Moved { to });
}

#[test]
fn riots_point_shape_is_left() {
    let env = env(None);
    let with_both = pointer(
        POINT,
        vec![
            ("emitOffset", vec3(1.0, 2.0, 3.0)),
            ("constantValue", vec3(4.0, 5.0, 6.0)),
        ],
    );
    let objects = vec![
        emitter("Riot", vec![("SpawnShape", point(1.0))]),
        emitter("Both", vec![("SpawnShape", with_both)]),
    ];
    let run = FixRun::of(&env, &set(), &archive(objects.clone()));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("Riot"), &objects[0]);
    assert_eq!(run.object("Both"), &objects[1]);
}

#[test]
fn the_spawn_shape_an_emitter_holds_stays() {
    let env = env(None);
    let made = pointer(LEGACY, vec![("emitOffset", vector(5.0))]);
    let renamed = embedded(LEGACY, vec![("emitOffset", vector(2.0))]);
    let renamed_point = embedded(POINT, vec![("emitOffset", vec3(1.0, 2.0, 3.0))]);
    for shape in [renamed, renamed_point] {
        let run = fixed(&env, vec![("shape", shape), ("SpawnShape", made.clone())]);
        assert!(run.field("E", "shape").is_none());
        assert_eq!(run.field("E", "SpawnShape"), Some(&made));
        // No build read the tool's value: nothing is lost by it.
        let [change] = run.changes.of(ID) else {
            panic!("{:?}", run.changes);
        };
        assert_eq!(change.after, After::Removed);
    }
}

#[test]
fn a_shape_of_a_class_a_build_held_there_is_left_to_the_layouts() {
    let env = env(None);
    let old = embedded("VfxShape", vec![("emitOffset", vector(2.0))]);
    let objects = vec![
        emitter("Old", vec![("shape", old)]),
        emitter("Float", vec![("shape", float(1.0))]),
    ];
    let run = FixRun::of(&env, &set(), &archive(objects.clone()));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("Old"), &objects[0]);
    assert_eq!(run.object("Float"), &objects[1]);
}

#[test]
fn a_tools_shape_is_undone_whatever_the_mods_build() {
    // A tool that copies game files dates the mod at its own run (#MOD-ARTIFACT-0029). Neither
    // date reads the tool's value.
    for made in [110, 150] {
        let env = env(Some(made));
        let renamed = embedded(LEGACY, vec![("emitOffset", vector(2.0))]);
        let set = set().with(SpawnShape);
        let objects = vec![emitter("E", vec![("shape", renamed)])];
        let run = FixRun::of(&env, &set, &archive(objects));
        assert_eq!(run.dating.build(), Some(made));
        assert_eq!(run.child("E", "SpawnShape").class_hash, name_hash(LEGACY));
        assert_eq!(run.changes.of(ID).len(), 1);
        assert!(run.changes.of(SHAPE_ID).is_empty(), "{:?}", run.changes);
    }
}

#[test]
fn the_fixup_applied_twice_changes_nothing_the_second_time() {
    let renamed = embedded(
        LEGACY,
        vec![
            ("emitOffset", vector(2.0)),
            ("birthTranslation", vector(3.0)),
        ],
    );
    let renamed_point = embedded(POINT, vec![("constantValue", vec3(1.0, 2.0, 3.0))]);
    let objects = vec![
        emitter("Renamed", vec![("shape", renamed)]),
        emitter("Point", vec![("shape", renamed_point)]),
    ];
    let entries = ["Renamed", "Point"];
    let env = env(None);
    let once = FixRun::of(&env, &set(), &archive(objects));
    // The point's offset moves twice: to `SpawnShape`, then to `emitOffset`.
    assert_eq!(once.changes.of(ID).len(), 4);
    assert_eq!(
        once.field("Point", "SpawnShape"),
        Some(&pointer(POINT, vec![("emitOffset", vec3(1.0, 2.0, 3.0))]))
    );
    let after = entries.map(|entry| once.object(entry).clone());
    let twice = once.again(&set());
    assert!(twice.changes.is_empty(), "{:?}", twice.changes);
    assert_eq!(after, entries.map(|entry| twice.object(entry).clone()));
}

#[test]
fn the_fixup_runs_where_the_target_reads_the_spawn_shape() {
    let old_target = Env::builder()
        .game(skins::game_at("1.0.110"))
        .schema(schema())
        .build()
        .expect("an environment");
    let renamed = embedded(LEGACY, vec![("emitOffset", vector(2.0))]);
    let objects = vec![emitter("E", vec![("shape", renamed)])];
    let run = FixRun::of(&old_target, &set(), &archive(objects.clone()));
    assert!(run.changes.is_empty());
    assert_eq!(run.object("E"), &objects[0]);
}
