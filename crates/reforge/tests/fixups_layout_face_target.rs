//! The fixup of a clip's event facing the camera: the event of the class 13.8 retired becomes
//! the class Riot gave it, its values kept, or goes where no build since the mod's own read it.
//! An event of the old class left as it is crashes the client at load (#GAME-BEHAVIOR-0028).

use ltk_meta::path::MapKey;
use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyKind, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::layout::FaceTarget;
use reforge::fixups::{After, FixupSet};
use reforge::game::Schema;
use reforge::model::{BinHash, FieldPath, name_hash};
use reforge_testkit::bins::{float, object, pointer};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind};
use reforge_testkit::{FixRun, MemoryHistory, SchemaRows, skins};

const ID: &str = "graph-face-target";
const CLIP: &str = "AtomicClipData";
const EVENTS: &str = "mEventDataMap";
const OLD: &str = "FaceCameraEventData";
const NEW: &str = "FaceTargetEventData";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";
const TARGET: &str = "16.19.8207193";

/// The target's schema: the new class.
fn schema() -> Schema {
    SchemaRows::new(8_207_193)
        .class(NEW, &[field("YRotationDegrees", kind("F32"))])
        .build()
}

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

/// The target. With `first_shipped`, a history in which the mod's dated file first shipped at
/// that build.
fn env(first_shipped: Option<&str>) -> Env {
    let mut env = Env::builder().game(skins::game_at(TARGET)).schema(schema());
    if let Some(build) = first_shipped {
        let history = MemoryHistory::new(&["13.7.5031019", build, TARGET]);
        env = env.history(Box::new(history.with_content(DATED, &dated_bytes(), 1..=2)));
    }
    env.build().expect("an environment")
}

/// A map of events by name.
fn events(entries: Vec<(&str, V)>) -> V {
    let entries = entries.into_iter().map(|(name, event)| {
        let key = V::Hash(values::Hash::new(name_hash(name)));
        (key, event)
    });
    let map = values::Map::new(PropertyKind::Hash, PropertyKind::Struct, entries.collect())
        .unwrap_or_else(|error| panic!("a map of events does not build: {error}"));
    V::Map(map)
}

/// A clip with a camera event of the old class and a particle event.
fn clip() -> BinObject {
    let camera = pointer(OLD, vec![("YRotationDegrees", float(135.0))]);
    let particle = pointer("ParticleEventData", vec![("mStartFrame", float(2.0))]);
    let map = events(vec![("FaceCamera", camera), ("Spark", particle)]);
    object("Dance", CLIP, vec![(EVENTS, map)])
}

fn set() -> FixupSet {
    FixupSet::empty().with(FaceTarget)
}

fn fixed(env: &Env) -> FixRun<'_> {
    let archive = skin_archive(vec![clip()]).file(skins::WAD, DATED, dated_bytes());
    FixRun::of(env, &set(), &archive)
}

/// The class of each event the clip holds, by name.
fn classes(run: &FixRun<'_>) -> Vec<(BinHash, BinHash)> {
    let Some(V::Map(map)) = run.field("Dance", EVENTS) else {
        panic!("the clip holds no map of events");
    };
    let rows = map.entries().iter().map(|(key, event)| match (key, event) {
        (V::Hash(key), V::Struct(event)) => (
            key.value
                .try_as_bin_hash()
                .expect("an event key is a 4-byte hash"),
            event.class_hash,
        ),
        other => panic!("an event of another shape: {other:?}"),
    });
    rows.collect()
}

fn at_event(name: &str) -> FieldPath {
    let map = FieldPath::root().field(name_hash(CLIP), name_hash(EVENTS));
    map.key(MapKey::try_from(&V::Hash(values::Hash::new(name_hash(name)))).expect("a key"))
}

#[test]
fn a_camera_event_takes_the_new_class_with_its_values() {
    let env = env(None);
    let run = fixed(&env);
    assert_eq!(
        classes(&run),
        [
            (name_hash("FaceCamera"), name_hash(NEW)),
            (name_hash("Spark"), name_hash("ParticleEventData")),
        ]
    );
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.path, at_event("FaceCamera"));
    assert!(matches!(change.after, After::Value(_)));

    let Some(V::Map(map)) = run.field("Dance", EVENTS) else {
        panic!("no map");
    };
    let Some((_, V::Struct(event))) = map.entries().first() else {
        panic!("no event");
    };
    let rotation = event.properties.get(&name_hash("YRotationDegrees"));
    assert_eq!(rotation, Some(&float(135.0)));
}

#[test]
fn a_mod_made_before_the_change_keeps_its_camera_event() {
    let env = env(Some("13.7.5031020"));
    let run = fixed(&env);
    assert_eq!(run.dating.build(), Some(5_031_020));
    assert_eq!(classes(&run)[0], (name_hash("FaceCamera"), name_hash(NEW)));
}

#[test]
fn a_camera_event_no_build_since_the_mods_own_read_is_removed() {
    // #ADR-0065: dated at 13.8, the first build that has no class of that name.
    let env = env(Some("13.8.5036621"));
    let run = fixed(&env);
    assert_eq!(run.dating.build(), Some(5_036_621));
    assert_eq!(
        classes(&run),
        [(name_hash("Spark"), name_hash("ParticleEventData"))]
    );
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.path, at_event("FaceCamera"));
    assert_eq!(change.after, After::Removed);
}

#[test]
fn the_fixup_applied_twice_changes_nothing_the_second_time() {
    let env = env(None);
    let once = fixed(&env);
    let after = once.object("Dance").clone();
    let twice = once.again(&set());
    assert!(twice.changes.is_empty(), "{:?}", twice.changes);
    assert_eq!(twice.object("Dance"), &after);
}
