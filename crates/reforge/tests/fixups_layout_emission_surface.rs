//! The fixup of an emission surface: its own fields in the struct `EmissionSurface` points to,
//! as 15.22 laid it out, and what chose the source of the emission removed with the move.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyKind as Kind, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::layout::EmissionSurface;
use reforge::fixups::{After, Changed, FixupSet, Outcome, Removed};
use reforge::game::Schema;
use reforge::model::value::as_str;
use reforge::model::{FieldPath, name_hash};
use reforge_testkit::bins::{flag, float, object, pointer, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind, list, pointer as to};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const ID: &str = "vfx-emission-surface";
const SURFACE: &str = "VfxEmissionSurfaceData";
const MESH: &str = "VfxEmissionMeshData";
const SKELETON: &str = "VfxEmissionSkeletonData";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";

/// The surface's own fields up to build 120, and from 121 `EmissionSurface` and the two
/// classes that hold them. `useAvatarPose` became `EmissionSource` at 110.
fn schema() -> Schema {
    let string = || kind("String");
    SchemaRows::new(300)
        .class(
            SURFACE,
            &[
                field("EmissionSurface", to("IVfxEmissionSurface")).from(121),
                field("meshName", string()).until(120),
                field("skeletonName", string()).until(120),
                field("JointMask", list("Hash")).until(120),
                field("unplaced", kind("F32")).until(120),
                field("useAvatarPose", kind("Bool")).until(109),
                field("EmissionSource", kind("U8")).from(110).until(120),
            ],
        )
        .class(
            MESH,
            &[
                field("meshName", string()).from(121),
                field("skeletonName", string()).from(121),
            ],
        )
        .class(
            SKELETON,
            &[
                field("skeletonName", string()).from(121),
                field("JointMask", list("Hash")).from(121),
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

fn set() -> FixupSet {
    FixupSet::empty().with(EmissionSurface)
}

fn surface(entry: &str, fields: Vec<(&str, V)>) -> BinObject {
    object(entry, SURFACE, fields)
}

/// One surface fixed, for a mod with no date.
fn fixed<'a>(env: &'a Env, fields: Vec<(&str, V)>) -> FixRun<'a> {
    FixRun::of(env, &set(), &archive(vec![surface("S", fields)]))
}

fn joint_mask() -> V {
    let joints = vec![V::Hash(values::Hash::new(name_hash("L_Hand")))];
    let list = values::Container::new(Kind::Hash, joints).expect("a list of hashes");
    V::Container(list)
}

fn source(value: u8) -> V {
    values::U8::new(value).into()
}

fn on_surface(field: &str) -> FieldPath {
    FieldPath::root().field(name_hash(SURFACE), name_hash(field))
}

/// A mesh surface of 15.21.
fn mesh_fields() -> Vec<(&'static str, V)> {
    vec![
        ("meshName", text("assets/mod/a/prop.skn")),
        ("skeletonName", text("assets/mod/a/prop.skl")),
        ("EmissionSource", source(1)),
        ("useAvatarPose", flag(true)),
    ]
}

#[test]
fn a_surfaces_own_fields_move_into_the_mesh_class() {
    let env = env(false);
    let run = fixed(&env, mesh_fields());
    let inner = run.child("S", "EmissionSurface");
    assert_eq!(inner.class_hash, name_hash(MESH));
    let held: Vec<_> = inner.properties.keys().copied().collect();
    assert_eq!(held, ["meshName", "skeletonName"].map(name_hash));
    let mesh = inner.properties.get(&name_hash("meshName"));
    assert_eq!(mesh.and_then(as_str), Some("assets/mod/a/prop.skn"));
    // What chose the source went with the move, as Riot's conversion dropped it.
    let left: Vec<_> = run.object("S").properties.keys().copied().collect();
    assert_eq!(left, [name_hash("EmissionSurface")]);

    let outcomes: Vec<Outcome> = run.changes.of(ID).iter().map(|c| c.outcome()).collect();
    let (moved, removed) = (Outcome::Changed, Outcome::Removed);
    assert_eq!(outcomes, [moved, moved, removed, removed]);
    let first = &run.changes.of(ID)[0];
    assert_eq!(first.path, on_surface("meshName"));
    let to = on_surface("EmissionSurface").field(name_hash(MESH), name_hash("meshName"));
    assert_eq!(first.after, After::Moved { to });
    let [changed] = run.raised::<Changed>(ID)[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!((changed.0.values, changed.0.objects), (2, 1));
    let [removed] = run.raised::<Removed>(ID)[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!(removed.0.values, 2);
}

#[test]
fn a_joint_mask_with_no_mesh_moves_into_the_skeleton_class() {
    let env = env(false);
    let fields = vec![
        ("skeletonName", text("assets/mod/a/prop.skl")),
        ("JointMask", joint_mask()),
    ];
    let run = fixed(&env, fields);
    // Only the skeleton's class declares both.
    let inner = run.child("S", "EmissionSurface");
    assert_eq!(inner.class_hash, name_hash(SKELETON));
    assert_eq!(
        inner.properties.get(&name_hash("JointMask")),
        Some(&joint_mask())
    );
    assert_eq!(inner.properties.len(), 2);
}

#[test]
fn a_surface_no_class_holds_whole_takes_the_mesh_class() {
    let env = env(false);
    let fields = vec![
        ("meshName", text("assets/mod/a/prop.skn")),
        ("JointMask", joint_mask()),
    ];
    let run = fixed(&env, fields);
    let inner = run.child("S", "EmissionSurface");
    assert_eq!(inner.class_hash, name_hash(MESH));
    let held: Vec<_> = inner.properties.keys().copied().collect();
    assert_eq!(held, [name_hash("meshName")]);
    // The mesh's class declares no joint mask: it stays on the surface.
    assert_eq!(run.field("S", "JointMask"), Some(&joint_mask()));
    assert_eq!(run.changes.of(ID).len(), 1);
}

#[test]
fn a_surface_holding_only_what_riot_retired_loses_it() {
    let env = env(false);
    let run = fixed(&env, vec![("useAvatarPose", flag(true))]);
    assert!(run.object("S").properties.is_empty());
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.path, on_surface("useAvatarPose"));
    assert_eq!(change.after, After::Removed);
}

#[test]
fn a_field_no_surface_class_declares_stays() {
    let env = env(false);
    let fields = vec![("unplaced", float(1.0)), ("EmissionSource", source(1))];
    let run = fixed(&env, fields);
    assert_eq!(run.field("S", "unplaced"), Some(&float(1.0)));
    assert!(run.field("S", "EmissionSurface").is_none());
    assert!(run.field("S", "EmissionSource").is_none());
    assert_eq!(run.changes.of(ID).len(), 1);
}

#[test]
fn a_surface_that_holds_an_emission_surface_is_left() {
    let env = env(false);
    let made = pointer(MESH, vec![("meshName", text("assets/mod/a/prop.skn"))]);
    let fields = vec![
        ("EmissionSurface", made),
        ("skeletonName", text("assets/mod/a/prop.skl")),
        ("EmissionSource", source(1)),
    ];
    let run = fixed(&env, fields.clone());
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("S"), &surface("S", fields));
}

#[test]
fn a_layout_the_mods_own_build_did_not_read_is_left() {
    // Made at 150 or later, when every build since read the new layout: the surface's own
    // fields never showed.
    let env = env(true);
    let fields = vec![
        ("meshName", text("assets/mod/a/prop.skn")),
        ("skeletonName", text("assets/mod/a/prop.skl")),
    ];
    let run = fixed(&env, fields.clone());
    assert_eq!(run.dating.build(), Some(150));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("S"), &surface("S", fields));
}

#[test]
fn what_riot_retired_goes_whatever_the_mods_date() {
    // As ltk-forge does (#ADR-0004): no build from 150 on read either field, and no struct is
    // made of fields that never showed.
    let env = env(true);
    let run = fixed(&env, mesh_fields());
    let left: Vec<_> = run.object("S").properties.keys().copied().collect();
    assert_eq!(left, ["meshName", "skeletonName"].map(name_hash));
    let outcomes: Vec<Outcome> = run.changes.of(ID).iter().map(|c| c.outcome()).collect();
    assert_eq!(outcomes, [Outcome::Removed, Outcome::Removed]);
}

#[test]
fn the_fixup_applied_twice_changes_nothing_the_second_time() {
    let objects = vec![
        surface("Mesh", mesh_fields()),
        surface(
            "Skeleton",
            vec![
                ("skeletonName", text("assets/mod/a/prop.skl")),
                ("JointMask", joint_mask()),
            ],
        ),
        surface(
            "Mixed",
            vec![
                ("meshName", text("assets/mod/a/prop.skn")),
                ("JointMask", joint_mask()),
                ("unplaced", float(1.0)),
            ],
        ),
        surface("Pose", vec![("useAvatarPose", flag(true))]),
    ];
    let entries = ["Mesh", "Skeleton", "Mixed", "Pose"];
    for dated in [false, true] {
        let env = env(dated);
        let once = FixRun::of(&env, &set(), &archive(objects.clone()));
        assert!(!once.changes.is_empty());
        let after = entries.map(|entry| once.object(entry).clone());
        let twice = once.again(&set());
        assert!(twice.changes.is_empty(), "{:?}", twice.changes);
        assert_eq!(after, entries.map(|entry| twice.object(entry).clone()));
    }
}

#[test]
fn the_fixup_runs_where_the_target_reads_the_emission_surface() {
    // A target of the old layout declares `meshName` on the surface and no `EmissionSurface`.
    let old_target = Env::builder()
        .game(skins::game_at("1.0.115"))
        .schema(schema())
        .build()
        .expect("an environment");
    let run = fixed(&old_target, mesh_fields());
    assert!(run.changes.is_empty());
    assert_eq!(run.object("S"), &surface("S", mesh_fields()));
}
