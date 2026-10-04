//! The fixup of a primitive mesh's camera alignment: a mesh aligned on either axis comes out
//! aligned on both, as 13.8 made every mesh of the game.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::layout::MeshAlign;
use reforge::fixups::{After, Changed, FixupSet, Lost, Outcome, Removed};
use reforge::game::Schema;
use reforge::model::{FieldPath, name_hash};
use reforge_testkit::bins::{flag, object};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{FieldRow, field, kind};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const ID: &str = "vfx-mesh-align";
const MESH: &str = "VfxPrimitiveMesh";
const ATTACHED: &str = "VfxPrimitiveAttachedMesh";
const BASE: &str = "VfxPrimitiveMeshBase";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";

/// The old fields on the two classes up to build 120, and from 121 the new ones on their
/// base. The pitch of an attached mesh's base is a flag of a bit field.
fn schema() -> Schema {
    let old = |name: &str| field(name, kind("Bool")).until(120);
    let both = || -> [FieldRow; 2] { [old("mAlignYawToCamera"), old("mAlignPitchToCamera")] };
    SchemaRows::new(300)
        .class(
            BASE,
            &[
                field("AlignYawToCamera", kind("Bool")).from(121),
                field("AlignPitchToCamera", kind("Bool")).from(121),
            ],
        )
        .class(
            "VfxPrimitiveAttachedMeshBase",
            &[
                field("AlignYawToCamera", kind("Bool")).from(121),
                field("AlignPitchToCamera", kind("Flag")).from(121),
            ],
        )
        .derived(MESH, &[BASE], &both())
        .derived(ATTACHED, &["VfxPrimitiveAttachedMeshBase"], &both())
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
    FixupSet::empty().with(MeshAlign)
}

/// One mesh of a class fixed, for a mod with no date.
fn fixed<'a>(env: &'a Env, class: &str, fields: Vec<(&str, V)>) -> FixRun<'a> {
    FixRun::of(env, &set(), &archive(vec![object("M", class, fields)]))
}

fn at(class: &str, field: &str) -> FieldPath {
    FieldPath::root().field(name_hash(class), name_hash(field))
}

#[test]
fn a_mesh_aligned_on_either_axis_faces_the_camera_on_both() {
    let env = env(false);
    let run = fixed(&env, MESH, vec![("mAlignYawToCamera", flag(true))]);
    assert_eq!(run.field("M", "AlignYawToCamera"), Some(&flag(true)));
    assert_eq!(run.field("M", "AlignPitchToCamera"), Some(&flag(true)));
    assert!(run.field("M", "mAlignYawToCamera").is_none());

    // The axis that was on moved to its new name, and the other was set.
    let [yaw, pitch] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(yaw.path, at(MESH, "mAlignYawToCamera"));
    let to = at(MESH, "AlignYawToCamera");
    assert_eq!(yaw.after, After::Moved { to });
    assert_eq!(pitch.path, at(MESH, "AlignPitchToCamera"));
    assert!(pitch.before.is_none() && matches!(pitch.after, After::Value(_)));
    let [changed] = run.raised::<Changed>(ID)[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!((changed.0.values, changed.0.objects), (2, 1));
}

#[test]
fn a_mesh_aligned_on_pitch_alone_faces_the_camera_on_both() {
    let env = env(false);
    let old = vec![
        ("mAlignYawToCamera", flag(false)),
        ("mAlignPitchToCamera", flag(true)),
    ];
    let run = fixed(&env, MESH, old);
    // The new fields take the last places, yaw first.
    let held: Vec<_> = run.object("M").properties.keys().copied().collect();
    assert_eq!(
        held,
        ["AlignYawToCamera", "AlignPitchToCamera"].map(name_hash)
    );
    assert_eq!(run.field("M", "AlignYawToCamera"), Some(&flag(true)));
    assert_eq!(run.field("M", "AlignPitchToCamera"), Some(&flag(true)));
    let outcomes: Vec<Outcome> = run.changes.of(ID).iter().map(|c| c.outcome()).collect();
    // The old yaw, false, is removed; the new yaw is set; the pitch moved.
    assert_eq!(
        outcomes,
        [Outcome::Removed, Outcome::Changed, Outcome::Changed]
    );
}

#[test]
fn a_mesh_that_held_only_false_gets_nothing() {
    let env = env(false);
    let run = fixed(&env, MESH, vec![("mAlignPitchToCamera", flag(false))]);
    // False is the default of the new fields: nothing to set.
    assert!(run.object("M").properties.is_empty());
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.after, After::Removed);
    assert_eq!(run.raised::<Removed>(ID).len(), 1);
    assert!(run.raised::<Changed>(ID).is_empty());
}

#[test]
fn a_new_field_is_of_the_kind_the_target_reads() {
    let env = env(false);
    let run = fixed(&env, ATTACHED, vec![("mAlignYawToCamera", flag(true))]);
    assert_eq!(run.field("M", "AlignYawToCamera"), Some(&flag(true)));
    assert_eq!(
        run.field("M", "AlignPitchToCamera"),
        Some(&V::BitBool(values::BitBool::new(true)))
    );
}

#[test]
fn the_new_field_a_mesh_holds_stays_as_it_is() {
    let env = env(false);
    let fields = vec![
        ("mAlignYawToCamera", flag(true)),
        ("mAlignPitchToCamera", flag(true)),
        ("AlignPitchToCamera", flag(false)),
    ];
    let run = fixed(&env, MESH, fields);
    assert_eq!(run.field("M", "AlignYawToCamera"), Some(&flag(true)));
    assert_eq!(run.field("M", "AlignPitchToCamera"), Some(&flag(false)));
    let [yaw, pitch] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(yaw.outcome(), Outcome::Changed);
    // The old pitch said true where the target reads the mesh's own false.
    assert_eq!(pitch.path, at(MESH, "mAlignPitchToCamera"));
    assert_eq!(pitch.after, After::Lost);
    assert_eq!(run.raised::<Lost>(ID).len(), 1);
}

#[test]
fn an_alignment_the_mods_own_build_did_not_read_is_left() {
    // Made at 150 or later, when every build since read the new fields: the old ones never
    // showed.
    let env = env(true);
    let fields = vec![("mAlignYawToCamera", flag(true))];
    let run = fixed(&env, MESH, fields.clone());
    assert_eq!(run.dating.build(), Some(150));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("M"), &object("M", MESH, fields));
}

#[test]
fn the_fixup_applied_twice_changes_nothing_the_second_time() {
    let mesh = |entry: &str, class: &str, fields| object(entry, class, fields);
    let objects = vec![
        mesh("Yaw", MESH, vec![("mAlignYawToCamera", flag(true))]),
        mesh("Off", MESH, vec![("mAlignPitchToCamera", flag(false))]),
        mesh(
            "Both",
            ATTACHED,
            vec![
                ("mAlignYawToCamera", flag(false)),
                ("mAlignPitchToCamera", flag(true)),
                ("AlignYawToCamera", flag(false)),
            ],
        ),
    ];
    for dated in [false, true] {
        let env = env(dated);
        let once = FixRun::of(&env, &set(), &archive(objects.clone()));
        assert_eq!(once.changes.is_empty(), dated);
        let after = ["Yaw", "Off", "Both"].map(|entry| once.object(entry).clone());
        let twice = once.again(&set());
        assert!(twice.changes.is_empty(), "{:?}", twice.changes);
        let again = ["Yaw", "Off", "Both"].map(|entry| twice.object(entry).clone());
        assert_eq!(after, again);
    }
}

#[test]
fn the_fixup_runs_where_the_target_reads_the_new_fields() {
    // A target of the old layout declares no `AlignYawToCamera`.
    let old_target = Env::builder()
        .game(skins::game_at("1.0.110"))
        .schema(schema())
        .build()
        .expect("an environment");
    let run = fixed(&old_target, MESH, vec![("mAlignYawToCamera", flag(true))]);
    assert!(run.changes.is_empty());
    assert_eq!(run.field("M", "mAlignYawToCamera"), Some(&flag(true)));
}
