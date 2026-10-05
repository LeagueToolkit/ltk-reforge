//! The layout fixups that are code, as a set: what each says of itself, the order they run in,
//! and one mod taken through all of them.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::{FixupSet, layout};
use reforge::game::Schema;
use reforge_testkit::bins::{embedded, flag, float, object, pointer, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{embed, field, kind, pointer as to};
use reforge_testkit::{FixRun, SchemaRows, skins};

/// The champion's skin entry of the base skin.
const ENTRY: &str = "Characters/Test/Skins/Skin0";

/// The ids of the fixups of this file, in the order the set runs them.
const IDS: [&str; 7] = [
    "health-bar-style",
    "material-default-technique",
    "vfx-mesh-align",
    "vfx-spawn-shape",
    "vfx-spawn-shape-birth-translation",
    "material-sampler-fields",
    "vfx-emission-surface",
];

#[test]
fn the_layout_fixups_name_their_records_and_builds() {
    let rows: Vec<(String, String, String)> = layout::all()
        .iter()
        .map(|fixup| {
            let info = fixup.info();
            let build = info.phase.build().expect("a layout fixup");
            let record = info.one_record().expect("one record");
            assert!(info.default, "{} is in the standard set", info.id);
            (info.id.to_string(), build.to_string(), record.to_string())
        })
        .collect();
    let row = |id: &str, build: &str, record: &str| (id.into(), build.into(), record.into());
    assert_eq!(
        rows,
        [
            row(IDS[0], "13.4", "#GAME-CHANGE-0023"),
            row(IDS[1], "13.5", "#GAME-CHANGE-0015"),
            row(IDS[2], "13.8", "#GAME-CHANGE-0017"),
            row(IDS[3], "14.1", "#GAME-CHANGE-0006"),
            row(IDS[4], "14.1", "#MOD-ARTIFACT-0019"),
            row(IDS[5], "15.3", "#GAME-CHANGE-0014"),
            row(IDS[6], "15.18", "#GAME-CHANGE-0018"),
        ]
    );
}

#[test]
fn the_standard_set_runs_them_by_build_after_the_table_rules() {
    let set = FixupSet::standard();
    let ids = set.ids();
    let found: Vec<&str> = ids.iter().copied().filter(|id| IDS.contains(id)).collect();
    assert_eq!(found, IDS);
    // 13.1 is the last table rule's build, and 13.4 the first of these.
    let place = |id: &str| ids.iter().position(|held| *held == id).expect("held");
    assert!(place("vfx-texture-mult") < place(IDS[0]));
}

/// One class of each fixup, old up to build 120 and new from 121.
fn schema() -> Schema {
    let vector = || embed("ValueVector3");
    let string = || kind("String");
    let flag = || kind("Bool");
    SchemaRows::new(300)
        .class(
            "VfxEmitterDefinitionData",
            &[
                field("shape", embed("VfxShape")).until(120),
                field("SpawnShape", to("IVfxShape")).from(121),
                field("EmitterPosition", vector()).from(121),
            ],
        )
        .class("VfxShapeLegacy", &[field("emitOffset", vector()).from(121)])
        .class(
            "VfxPrimitiveMesh",
            &[
                field("mAlignYawToCamera", flag()).until(120),
                field("mAlignPitchToCamera", flag()).until(120),
                field("AlignYawToCamera", flag()).from(121),
                field("AlignPitchToCamera", flag()).from(121),
            ],
        )
        .class(
            "VfxEmissionSurfaceData",
            &[
                field("EmissionSurface", to("IVfxEmissionSurface")).from(121),
                field("meshName", string()).until(120),
            ],
        )
        .class(
            "VfxEmissionMeshData",
            &[field("meshName", string()).from(121)],
        )
        .class(
            "StaticMaterialShaderSamplerDef",
            &[
                field("TextureName", string()),
                field("samplerName", string()).until(120),
                field("texturePath", string()).from(121),
            ],
        )
        .class(
            "StaticMaterialDef",
            &[
                field("name", string()),
                field("defaultTechnique", string()).until(120),
            ],
        )
        .class(
            "SkinCharacterDataProperties",
            &[field(
                "healthBarData",
                embed("CharacterHealthBarDataRecord"),
            )],
        )
        .class(
            "CharacterHealthBarDataRecord",
            &[field("unitHealthBarStyle", kind("U8"))],
        )
        .build()
}

fn vector(x: f32) -> V {
    embedded("ValueVector3", vec![("constantValue", float(x))])
}

/// One object of the old layout for each fixup.
fn old_mod() -> Vec<BinObject> {
    let shape = embedded(
        "VfxShape",
        vec![
            ("birthTranslation", vector(1.0)),
            ("emitOffset", vector(2.0)),
        ],
    );
    let misplaced = pointer("VfxShapeLegacy", vec![("birthTranslation", vector(3.0))]);
    let sampler = vec![
        ("samplerName", text("Diffuse_Texture")),
        ("TextureName", text("assets/mod/a/body.tex")),
    ];
    let emitter = "VfxEmitterDefinitionData";
    let bar = embedded(
        "CharacterHealthBarDataRecord",
        vec![("unitHealthBarStyle", V::U8(values::U8::new(9)))],
    );
    vec![
        object(
            ENTRY,
            "SkinCharacterDataProperties",
            vec![("healthBarData", bar)],
        ),
        object("Old", emitter, vec![("shape", shape)]),
        object("Misplaced", emitter, vec![("SpawnShape", misplaced)]),
        object(
            "Yaw",
            "VfxPrimitiveMesh",
            vec![("mAlignYawToCamera", flag(true))],
        ),
        object(
            "Mesh",
            "VfxEmissionSurfaceData",
            vec![
                ("meshName", text("assets/mod/a/prop.skn")),
                ("useAvatarPose", flag(true)),
            ],
        ),
        object("Sampler", "StaticMaterialShaderSamplerDef", sampler),
        object(
            "Glow",
            "StaticMaterialDef",
            vec![
                ("name", text("Glow")),
                ("defaultTechnique", text("glowing")),
            ],
        ),
    ]
}

#[test]
fn one_mod_is_laid_out_by_every_layout_fixup_once() {
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .build()
        .expect("an environment");
    let set = FixupSet::empty()
        .with(layout::EmissionSurface)
        .with(layout::SamplerFields)
        .with(layout::BirthTranslation)
        .with(layout::SpawnShape)
        .with(layout::MeshAlign)
        .with(layout::DefaultTechnique)
        .with(layout::HealthBarStyle);
    let once = FixRun::of(&env, &set, &skin_archive(old_mod()));
    let by_fixup: Vec<&str> = once.changes.by_fixup().map(|(id, _)| id.as_str()).collect();
    assert_eq!(by_fixup, IDS);
    let counts = once.changes.counts();
    // Lost: the technique a material chose.
    assert_eq!((counts.changed, counts.removed, counts.lost), (9, 1, 1));
    assert!(once.field("Old", "SpawnShape").is_some());
    assert!(once.field("Misplaced", "EmitterPosition").is_some());
    assert!(once.field("Yaw", "AlignPitchToCamera").is_some());
    assert!(once.field("Mesh", "EmissionSurface").is_some());
    assert!(once.field("Sampler", "texturePath").is_some());
    assert!(once.field("Glow", "defaultTechnique").is_none());

    let entries = [ENTRY, "Old", "Misplaced", "Yaw", "Mesh", "Sampler", "Glow"];
    let after = entries.map(|entry| once.object(entry).clone());
    let twice = once.again(&set);
    assert!(twice.changes.is_empty(), "{:?}", twice.changes);
    assert_eq!(after, entries.map(|entry| twice.object(entry).clone()));
}
