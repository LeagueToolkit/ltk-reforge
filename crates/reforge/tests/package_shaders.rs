//! A material's link to a shader the target's shader bin lacks (#ADR-0069): relinked where
//! Riot's materials took another shader (#GAME-CHANGE-0024), and a mod with a link that has no
//! relink gets no package, since the client crashes at load (#GAME-BEHAVIOR-0023). A material
//! that asks a game shader for a variant the target's shader cache lacks gets no package either,
//! since the client crashes at its draw (#GAME-BEHAVIOR-0029).

use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::diag::{Diag, Record};
use reforge::env::Env;
use reforge::fixups::layout::ShaderRelink;
use reforge::fixups::{After, Changed, FixupSet};
use reforge::game::{SHADERS_BIN, Schema, ShaderStage};
use reforge::model::name_hash;
use reforge::package::{
    BuildError, ShaderLink, ShaderMissing, ShaderVariant, ShaderVariantMissing,
};
use reforge_testkit::bins::{bin, embedded, link, object};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind};
use reforge_testkit::shaders::{self, Asks, embeds, toc};
use reforge_testkit::{PackRun, SchemaRows, skins};

const RELINK_ID: &str = "material-shader-relink";
const MATERIAL: &str = "Characters/Test/Skins/Skin0/Materials/Glow";
/// Removed at 15.15; Riot's materials took `AlphaBlend_Additive_Scroll_Packed`.
const REMOVED: &str = "Shaders/SkinnedMesh/MultiMask_Panner_Fresnel";
const RELINKED: &str = "Shaders/SkinnedMesh/AlphaBlend_Additive_Scroll_Packed";
/// Removed at 15.14 with every material of Riot's that named it: no relink.
const UNRELINKED: &str = "Shaders/SkinnedMesh/Hologram";
const HELD: &str = "Shaders/SkinnedMesh/Diffuse_Alpha_mult";

fn schema() -> Schema {
    SchemaRows::new(300)
        .class("StaticMaterialPassDef", &[field("shader", kind("Link"))])
        .build()
}

/// The target at build 300, its shader bin holding `RELINKED` and `HELD` when `shaders`.
fn env(shaders: bool) -> Env {
    let mut game = skins::game_at("16.19.300");
    if shaders {
        let defined = |name: &str| object(name, "CustomShaderDef", vec![]);
        game.insert(SHADERS_BIN, bin(vec![defined(RELINKED), defined(HELD)]));
    }
    Env::builder()
        .game(game)
        .schema(schema())
        .build()
        .expect("an environment")
}

/// A material of one technique whose passes link these shaders.
fn material(shaders: &[&str]) -> BinObject {
    let passes = shaders
        .iter()
        .map(|shader| embedded("StaticMaterialPassDef", vec![("shader", link(shader))]))
        .collect();
    let technique = embedded(
        "StaticMaterialTechniqueDef",
        vec![("passes", embeds(passes))],
    );
    object(
        MATERIAL,
        "StaticMaterialDef",
        vec![("techniques", embeds(vec![technique]))],
    )
}

fn relink_only() -> FixupSet {
    FixupSet::empty().with(ShaderRelink)
}

/// The shaders the material's passes link after the run.
fn links(run: &PackRun<'_>) -> Vec<V> {
    let mut found = Vec::new();
    reforge::model::walk::structs(run.fix.object(MATERIAL), |s| {
        if let Some(value) = s.get(name_hash("shader")) {
            found.push(value.clone());
        }
    });
    found
}

#[test]
fn a_material_linking_a_removed_shader_takes_riots_relink() {
    let env = env(true);
    let archive = skin_archive(vec![material(&[REMOVED, HELD])]);
    let run = PackRun::of(&env, &relink_only(), &archive);
    assert_eq!(links(&run), [link(RELINKED), link(HELD)]);
    let [change] = run.fix.changes.of(RELINK_ID) else {
        panic!("{:?}", run.fix.changes);
    };
    let After::Value(after) = &change.after else {
        panic!("{change:?}");
    };
    assert_eq!(after.kind, "Link");
    assert_eq!(run.fix.raised::<Changed>(RELINK_ID).len(), 1);
    // The relinked material loads: the package is built.
    let _ = run.package();
}

#[test]
fn a_material_linking_a_shader_with_no_relink_gets_no_package() {
    let env = env(true);
    let archive = skin_archive(vec![material(&[UNRELINKED, HELD])]);
    let run = PackRun::of(&env, &relink_only(), &archive);
    assert_eq!(links(&run), [link(UNRELINKED), link(HELD)]);
    assert!(run.fix.changes.of(RELINK_ID).is_empty());
    assert!(matches!(run.package, Err(BuildError::Halted(_))));
    let missing = run.only::<ShaderMissing>();
    assert_eq!(
        missing.links,
        [ShaderLink {
            material: name_hash(MATERIAL),
            shader: name_hash(UNRELINKED),
        }]
    );
    assert_eq!(ShaderMissing::CODE.name, "package.shader-missing");
}

#[test]
fn a_game_with_no_shader_bin_judges_no_shader() {
    let env = env(false);
    let archive = skin_archive(vec![material(&[REMOVED, UNRELINKED])]);
    let run = PackRun::of(&env, &relink_only(), &archive);
    assert_eq!(links(&run), [link(REMOVED), link(UNRELINKED)]);
    assert!(run.fix.changes.is_empty());
    let _ = run.package();
}

/// A game shader with programs in the cache, shaped as #GAME-BEHAVIOR-0029's: a feature define,
/// a switch on by default and one off.
const VARIANTS: &str = "Shaders/SkinnedMesh/Test_Variants";
const PIXEL: &str = "ASSETS/Shaders/Generated/Shaders/SkinnedMesh/Test_Variants.ps-dx11";
const VERTEX: &str = "ASSETS/Shaders/Generated/Shaders/SkinnedMesh/Test_Variants.vs-dx11";
const OTHER_PIXEL: &str = "ASSETS/Shaders/Generated/Shaders/SkinnedMesh/Test_Other.ps-dx11";
const OTHER_VERTEX: &str = "ASSETS/Shaders/Generated/Shaders/SkinnedMesh/Test_Other.vs-dx11";
const BASE: &[(&str, &str)] = &[
    ("FEATURE_BLOOM", "1"),
    ("USE_RIM", "1"),
    ("OFF_VALUE_ON_TEXTURE", "1"),
    ("TRANSITION", "1"),
];
/// The variant of `OFF_VALUE_ON_TEXTURE` on, the only one compiled.
const SWITCHED_ON: &str = "FEATURE_BLOOM=1OFF_VALUE_ON_TEXTURE=1USE_RIM=1";
const SWITCHED_ON_TRANSITION: &str = "FEATURE_BLOOM=1OFF_VALUE_ON_TEXTURE=1TRANSITION=1USE_RIM=1";
/// The variant of the shader's defaults: `OFF_VALUE_ON_TEXTURE=0` pairs no base define.
const DEFAULTS: &str = "FEATURE_BLOOM=1USE_RIM=1";
const TECHNIQUE: &str = "normal";
const SWITCH_ON: &[(&str, bool)] = &[("OFF_VALUE_ON_TEXTURE", true)];

/// The target at build 300, its shader bin defining `VARIANTS`, and its shader cache holding
/// these files.
fn variant_env(files: Vec<(&str, Vec<u8>)>) -> Env {
    let mut game = skins::game_at("16.19.300");
    game.insert(SHADERS_BIN, bin(vec![variants_shader()]));
    for (path, bytes) in files {
        game.insert(path, bytes);
    }
    Env::builder()
        .game(game)
        .schema(schema())
        .build()
        .expect("an environment")
}

/// The definition of `VARIANTS`: `USE_RIM` on by default, `OFF_VALUE_ON_TEXTURE` off.
fn variants_shader() -> BinObject {
    shaders::shader(
        VARIANTS,
        &[("FEATURE_BLOOM", "1")],
        &[("USE_RIM", true), ("OFF_VALUE_ON_TEXTURE", false)],
    )
}

/// Both files of `VARIANTS`, of `BASE` and these variants.
fn both(variants: &[&str]) -> Vec<(&'static str, Vec<u8>)> {
    vec![(PIXEL, toc(BASE, variants)), (VERTEX, toc(BASE, variants))]
}

/// A material of one technique of one pass on `VARIANTS`.
fn variant_material(asks: &Asks<'_>) -> BinObject {
    shaders::material(MATERIAL, VARIANTS, TECHNIQUE, asks)
}

fn variant_run<'e>(env: &'e Env, asks: &Asks<'_>) -> PackRun<'e> {
    PackRun::of(
        env,
        &relink_only(),
        &skin_archive(vec![variant_material(asks)]),
    )
}

/// The variant of `VARIANTS` the test material asks for in one file.
fn variant(child: Option<&str>, stage: ShaderStage, filtered: &str) -> ShaderVariant {
    ShaderVariant {
        material: name_hash(MATERIAL),
        shader: name_hash(VARIANTS),
        technique: TECHNIQUE.to_owned(),
        child: child.map(str::to_owned),
        stage,
        filtered: filtered.to_owned(),
    }
}

#[test]
fn a_material_whose_switches_ask_for_a_compiled_variant_is_built() {
    let env = variant_env(both(&[SWITCHED_ON]));
    let asks = Asks {
        switches: SWITCH_ON,
        ..Asks::default()
    };
    let _ = variant_run(&env, &asks).package();
}

#[test]
fn a_switch_left_at_a_default_never_compiled_gets_no_package() {
    let env = variant_env(both(&[SWITCHED_ON]));
    let run = variant_run(&env, &Asks::default());
    assert!(matches!(run.package, Err(BuildError::Halted(_))));
    let missing = run.only::<ShaderVariantMissing>();
    assert_eq!(
        missing.variants,
        [
            variant(None, ShaderStage::Pixel, DEFAULTS),
            variant(None, ShaderStage::Vertex, DEFAULTS),
        ]
    );
    assert_eq!(
        ShaderVariantMissing::CODE.name,
        "package.shader-variant-missing"
    );
    assert_eq!(
        ShaderVariantMissing::RECORD,
        Some(Record::game_behavior(29))
    );
}

#[test]
fn a_child_technique_asking_for_a_variant_the_cache_lacks_gets_no_package() {
    let env = variant_env(both(&[SWITCHED_ON]));
    let asks = Asks {
        switches: SWITCH_ON,
        children: &[("transition", &[("TRANSITION", "1")])],
        ..Asks::default()
    };
    let run = variant_run(&env, &asks);
    assert!(matches!(run.package, Err(BuildError::Halted(_))));
    let missing = run.only::<ShaderVariantMissing>();
    let child = Some("transition");
    assert_eq!(
        missing.variants,
        [
            variant(child, ShaderStage::Pixel, SWITCHED_ON_TRANSITION),
            variant(child, ShaderStage::Vertex, SWITCHED_ON_TRANSITION),
        ]
    );
}

#[test]
fn a_define_that_pairs_no_base_define_is_left_out_of_the_key() {
    let env = variant_env(both(&[SWITCHED_ON]));
    // No base define is named `NUM_BLEND_WEIGHTS`, and `TRANSITION` is paired with 1.
    let asks = Asks {
        switches: SWITCH_ON,
        macros: &[("NUM_BLEND_WEIGHTS", "4"), ("TRANSITION", "0")],
        ..Asks::default()
    };
    let _ = variant_run(&env, &asks).package();
}

#[test]
fn a_pass_macro_is_part_of_the_key() {
    let env = variant_env(both(&[SWITCHED_ON_TRANSITION]));
    let asks = Asks {
        switches: SWITCH_ON,
        pass_macros: &[("TRANSITION", "1")],
        ..Asks::default()
    };
    let _ = variant_run(&env, &asks).package();
}

#[test]
fn a_base_define_listed_with_two_values_pairs_either() {
    let base = [BASE, &[("OFF_VALUE_ON_TEXTURE", "0")]].concat();
    let compiled = "FEATURE_BLOOM=1OFF_VALUE_ON_TEXTURE=0USE_RIM=1";
    let files = vec![
        (PIXEL, toc(&base, &[compiled])),
        (VERTEX, toc(&base, &[compiled])),
    ];
    let env = variant_env(files);
    let _ = variant_run(&env, &Asks::default()).package();
}

#[test]
fn a_variant_the_vertex_file_alone_lacks_gets_no_package() {
    let files = vec![
        (PIXEL, toc(BASE, &[SWITCHED_ON, DEFAULTS])),
        (VERTEX, toc(BASE, &[SWITCHED_ON])),
    ];
    let env = variant_env(files);
    let run = variant_run(&env, &Asks::default());
    assert!(matches!(run.package, Err(BuildError::Halted(_))));
    let missing = run.only::<ShaderVariantMissing>();
    assert_eq!(
        missing.variants,
        [variant(None, ShaderStage::Vertex, DEFAULTS)]
    );
}

#[test]
fn a_shader_with_no_file_in_the_cache_is_not_judged() {
    let files = vec![
        (OTHER_PIXEL, toc(BASE, &[SWITCHED_ON])),
        (OTHER_VERTEX, toc(BASE, &[SWITCHED_ON])),
    ];
    let env = variant_env(files);
    let _ = variant_run(&env, &Asks::default()).package();
}

#[test]
fn a_game_with_no_shader_cache_judges_no_variant() {
    let env = variant_env(Vec::new());
    let _ = variant_run(&env, &Asks::default()).package();
}

#[test]
fn a_shader_the_mod_defines_is_not_judged_against_the_cache() {
    // #GAME-BEHAVIOR-0022: the mod's own definition has no programs, whatever its name.
    let env = variant_env(both(&[SWITCHED_ON]));
    let objects = vec![variant_material(&Asks::default()), variants_shader()];
    let run = PackRun::of(&env, &relink_only(), &skin_archive(objects));
    let _ = run.package();
}
