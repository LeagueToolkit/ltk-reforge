//! A material's link to a shader the target's shader bin lacks (#ADR-0069): relinked where
//! Riot's materials took another shader (#GAME-CHANGE-0024), and a mod with a link that has no
//! relink gets no package, since the client crashes at load (#GAME-BEHAVIOR-0023).

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyKind as Kind, PropertyValueEnum as V};
use reforge::diag::Diag;
use reforge::env::Env;
use reforge::fixups::layout::ShaderRelink;
use reforge::fixups::{After, Changed, FixupSet};
use reforge::game::{SHADERS_BIN, Schema};
use reforge::model::name_hash;
use reforge::package::{BuildError, ShaderLink, ShaderMissing};
use reforge_testkit::bins::{bin, embedded, link, object};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind};
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

fn embeds(items: Vec<V>) -> V {
    V::Container(values::Container::new(Kind::Embedded, items).expect("a list of embeds"))
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
