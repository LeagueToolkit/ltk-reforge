//! A material asking a game shader for a variant the target's shader cache lacks, moved to the
//! nearest variant the cache holds (#ADR-0092): one switch flipped or one macro set, where
//! that one define makes every variant the material asks for a compiled one; left otherwise.

use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::conform::ShaderVariantNearest;
use reforge::fixups::{After, Changed, FixupSet, Outcome};
use reforge::game::{SHADERS_BIN, Schema};
use reforge::model::value::{as_str, field_of, items};
use reforge::model::{Phase, name_hash};
use reforge_testkit::bins::{bin, object};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, list2, map};
use reforge_testkit::shaders::{Asks, material, shader, toc};
use reforge_testkit::{FixRun, PackRun, SchemaRows, skins};

const ID: &str = "shader-variant";
const MATERIAL: &str = "Characters/Test/Skins/Skin0/Materials/Glow";
const TECHNIQUE: &str = "normal";
/// A game shader shaped as #GAME-BEHAVIOR-0029's `Leona_Texture_Transition`: a feature define,
/// a switch on by default and one off.
const VARIANTS: &str = "Shaders/SkinnedMesh/Test_Variants";
const PIXEL: &str = "ASSETS/Shaders/Generated/Shaders/SkinnedMesh/Test_Variants.ps-dx11";
const VERTEX: &str = "ASSETS/Shaders/Generated/Shaders/SkinnedMesh/Test_Variants.vs-dx11";
const BASE: &[(&str, &str)] = &[
    ("FEATURE_BLOOM", "1"),
    ("USE_RIM", "1"),
    ("OFF_VALUE_ON_TEXTURE", "1"),
    ("TRANSITION", "1"),
    ("SKINNED_MATERIAL_ADDITIVE", "1"),
];
/// The variant of `OFF_VALUE_ON_TEXTURE` on.
const SWITCHED_ON: &str = "FEATURE_BLOOM=1OFF_VALUE_ON_TEXTURE=1USE_RIM=1";
const SWITCHED_ON_TRANSITION: &str = "FEATURE_BLOOM=1OFF_VALUE_ON_TEXTURE=1TRANSITION=1USE_RIM=1";
/// The variant of the shader's defaults.
const DEFAULTS: &str = "FEATURE_BLOOM=1USE_RIM=1";
/// The variant of the defaults with a macro no switch sets.
const ADDITIVE: &str = "FEATURE_BLOOM=1SKINNED_MATERIAL_ADDITIVE=1USE_RIM=1";
/// The variant of the switch on and the macro: two defines from the defaults.
const SWITCHED_ON_ADDITIVE: &str =
    "FEATURE_BLOOM=1OFF_VALUE_ON_TEXTURE=1SKINNED_MATERIAL_ADDITIVE=1USE_RIM=1";

fn schema() -> Schema {
    SchemaRows::new(300)
        .class(
            "StaticMaterialDef",
            &[
                field("switches", list2("Embed")),
                field("shaderMacros", map("String", "String")),
            ],
        )
        .build()
}

/// The target at build 300, its shader bin defining `VARIANTS` and its cache holding these
/// variants in both files.
fn env(variants: &[&str]) -> Env {
    let mut game = skins::game_at("16.19.300");
    let defined = shader(
        VARIANTS,
        &[("FEATURE_BLOOM", "1")],
        &[("USE_RIM", true), ("OFF_VALUE_ON_TEXTURE", false)],
    );
    game.insert(SHADERS_BIN, bin(vec![defined]));
    game.insert(PIXEL, toc(BASE, variants));
    game.insert(VERTEX, toc(BASE, variants));
    Env::builder()
        .game(game)
        .schema(schema())
        .build()
        .expect("an environment")
}

fn set() -> FixupSet {
    FixupSet::empty().with(ShaderVariantNearest)
}

fn asking(asks: &Asks<'_>) -> BinObject {
    material(MATERIAL, VARIANTS, TECHNIQUE, asks)
}

/// The material's switches after the run, each by name and whether it is on.
fn switches(run: &FixRun<'_>) -> Vec<(String, bool)> {
    let switches = run.field(MATERIAL, "switches").map_or(&[][..], items);
    switches
        .iter()
        .map(|switch| {
            let name = field_of(switch, "name")
                .and_then(as_str)
                .unwrap_or_default();
            let on = match field_of(switch, "on") {
                Some(V::Bool(on)) => on.value,
                _ => true,
            };
            (name.to_owned(), on)
        })
        .collect()
}

/// The material's macros after the run.
fn macros(run: &FixRun<'_>) -> Vec<(String, String)> {
    let Some(V::Map(map)) = run.field(MATERIAL, "shaderMacros") else {
        return Vec::new();
    };
    map.entries()
        .iter()
        .filter_map(|(name, value)| Some((as_str(name)?.to_owned(), as_str(value)?.to_owned())))
        .collect()
}

#[test]
fn a_switch_left_at_a_default_never_compiled_is_set_to_the_compiled_value() {
    let env = env(&[SWITCHED_ON]);
    let archive = skin_archive(vec![asking(&Asks::default())]);
    let run = PackRun::of(&env, &set(), &archive);
    assert_eq!(
        switches(&run.fix),
        [("OFF_VALUE_ON_TEXTURE".to_owned(), true)]
    );
    let [change] = run.fix.changes.of(ID) else {
        panic!("{:?}", run.fix.changes);
    };
    assert_eq!(change.outcome(), Outcome::Changed);
    assert!(change.before.is_none(), "{change:?}");
    let After::Value(after) = &change.after else {
        panic!("{change:?}");
    };
    assert_eq!(after.kind, "Embed");
    assert_eq!(change.path.spelled(&run.fix.speller()), "switches[0]");
    assert_eq!(run.fix.raised::<Changed>(ID).len(), 1);
    // The material asks for the compiled variant: the package is built.
    let _ = run.package();
}

#[test]
fn a_switch_the_material_sets_to_a_value_never_compiled_is_flipped() {
    let env = env(&[SWITCHED_ON]);
    let asks = Asks {
        switches: &[("OFF_VALUE_ON_TEXTURE", false)],
        ..Asks::default()
    };
    let archive = skin_archive(vec![asking(&asks)]);
    let run = PackRun::of(&env, &set(), &archive);
    assert_eq!(
        switches(&run.fix),
        [("OFF_VALUE_ON_TEXTURE".to_owned(), true)]
    );
    let [change] = run.fix.changes.of(ID) else {
        panic!("{:?}", run.fix.changes);
    };
    assert_eq!(change.path.spelled(&run.fix.speller()), "switches[0].on");
    assert_eq!(
        change.before.as_ref().map(|before| &*before.text),
        Some("false")
    );
    let _ = run.package();
}

#[test]
fn a_child_techniques_variant_is_asked_for_too() {
    let env = env(&[SWITCHED_ON, SWITCHED_ON_TRANSITION]);
    let asks = Asks {
        children: &[("transition", &[("TRANSITION", "1")])],
        ..Asks::default()
    };
    let archive = skin_archive(vec![asking(&asks)]);
    let run = PackRun::of(&env, &set(), &archive);
    assert_eq!(
        switches(&run.fix),
        [("OFF_VALUE_ON_TEXTURE".to_owned(), true)]
    );
    let _ = run.package();
}

#[test]
fn a_define_that_is_no_switch_of_the_shader_is_set_as_a_macro() {
    let env = env(&[ADDITIVE]);
    let archive = skin_archive(vec![asking(&Asks::default())]);
    let run = PackRun::of(&env, &set(), &archive);
    assert!(switches(&run.fix).is_empty());
    assert_eq!(
        macros(&run.fix),
        [("SKINNED_MATERIAL_ADDITIVE".to_owned(), "1".to_owned())]
    );
    let [change] = run.fix.changes.of(ID) else {
        panic!("{:?}", run.fix.changes);
    };
    assert_eq!(
        change.path.spelled(&run.fix.speller()),
        "shaderMacros{\"SKINNED_MATERIAL_ADDITIVE\"}"
    );
    let _ = run.package();
}

#[test]
fn a_macro_the_material_sets_to_a_value_never_compiled_takes_the_compiled_one() {
    let env = env(&[ADDITIVE]);
    let asks = Asks {
        macros: &[("SKINNED_MATERIAL_ADDITIVE", "0")],
        ..Asks::default()
    };
    let archive = skin_archive(vec![asking(&asks)]);
    let run = PackRun::of(&env, &set(), &archive);
    assert_eq!(
        macros(&run.fix),
        [("SKINNED_MATERIAL_ADDITIVE".to_owned(), "1".to_owned())]
    );
    let [change] = run.fix.changes.of(ID) else {
        panic!("{:?}", run.fix.changes);
    };
    assert_eq!(
        change.before.as_ref().map(|before| &*before.text),
        Some("\"0\"")
    );
    let _ = run.package();
}

#[test]
fn a_material_two_defines_from_every_compiled_variant_is_left() {
    let env = env(&[SWITCHED_ON_ADDITIVE]);
    let archive = skin_archive(vec![asking(&Asks::default())]);
    let run = PackRun::of(&env, &set(), &archive);
    assert!(run.fix.changes.is_empty(), "{:?}", run.fix.changes);
    assert!(run.package.is_err());
}

#[test]
fn a_material_asking_for_compiled_variants_is_left() {
    let env = env(&[DEFAULTS]);
    let archive = skin_archive(vec![asking(&Asks::default())]);
    let run = PackRun::of(&env, &set(), &archive);
    assert!(run.fix.changes.is_empty(), "{:?}", run.fix.changes);
    let _ = run.package();
}

#[test]
fn a_material_with_no_switches_field_gets_one_of_the_targets_kind() {
    let env = env(&[SWITCHED_ON]);
    let mut material = asking(&Asks::default());
    material.properties.shift_remove(&name_hash("switches"));
    let archive = skin_archive(vec![material]);
    let run = PackRun::of(&env, &set(), &archive);
    assert!(matches!(
        run.fix.field(MATERIAL, "switches"),
        Some(V::UnorderedContainer(_))
    ));
    assert_eq!(
        switches(&run.fix),
        [("OFF_VALUE_ON_TEXTURE".to_owned(), true)]
    );
    let _ = run.package();
}

#[test]
fn a_game_with_no_shader_bin_moves_no_material() {
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .build()
        .expect("an environment");
    let archive = skin_archive(vec![asking(&Asks::default())]);
    let run = FixRun::of(&env, &set(), &archive);
    assert!(run.changes.is_empty(), "{:?}", run.changes);
}

#[test]
fn applied_twice_it_changes_nothing_the_second_time() {
    let env = env(&[SWITCHED_ON]);
    let archive = skin_archive(vec![asking(&Asks::default())]);
    let run = FixRun::of(&env, &set(), &archive);
    assert_eq!(run.changes.of(ID).len(), 1);
    let again = run.again(&set());
    assert!(again.changes.is_empty(), "{:?}", again.changes);
}

#[test]
fn the_fixup_runs_in_the_conform_phase_after_retype() {
    let set = FixupSet::standard();
    let ids = set.ids();
    let at = |id: &str| ids.iter().position(|held| *held == id).expect("in the set");
    assert!(at("retype") < at(ID));
    assert!(at(ID) < at("strip-undeclared"));
    let info = set.infos()[at(ID)];
    assert_eq!(info.phase, Phase::Conform);
    assert_eq!(
        info.one_record().map(|r| r.to_string()),
        Some("#GAME-BEHAVIOR-0029".into())
    );
}

#[test]
fn a_shader_the_bin_lacks_is_not_judged() {
    let env = env(&[SWITCHED_ON]);
    let asks = Asks::default();
    let elsewhere = material(MATERIAL, "Shaders/SkinnedMesh/Gone", TECHNIQUE, &asks);
    let archive = skin_archive(vec![elsewhere, object("Other", "Unrelated", vec![])]);
    let run = FixRun::of(&env, &set(), &archive);
    assert!(run.changes.is_empty(), "{:?}", run.changes);
}
