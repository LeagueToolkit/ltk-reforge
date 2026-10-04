//! The built-in table rules: an emitter's fields gathered into `Filtering`, `Linger`,
//! `FlexShapeDefinition`, `LegacySimple` and `textureMult`, as Riot's own conversions did.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::{After, Changed, FixupSet, Lost, rules};
use reforge::game::Schema;
use reforge::model::name_hash;
use reforge::model::value::as_str;
use reforge_testkit::bins::{embedded, field_of, flag, float, object, pointer, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{FieldRow, Type, embed, field, kind, pointer as to};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const EMITTER: &str = "VfxEmitterDefinitionData";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";

/// A field of the layout up to build 120.
fn old(name: &str, of: Type) -> FieldRow {
    field(name, of).until(120)
}

/// A field of the layout from build 121.
fn new(name: &str, of: Type) -> FieldRow {
    field(name, of).from(121)
}

/// The emitter's old layout up to build 120, and from 121 the five structs with the fields
/// they took.
fn schema() -> Schema {
    let vector = || embed("ValueVector3");
    SchemaRows::new(300)
        .class(
            EMITTER,
            &[
                new("Filtering", to("VfxEmitterFiltering")),
                old("keywordsExcluded", kind("String")),
                old("spectatorPolicy", kind("U8")),
                new("Linger", to("VfxLingerDefinitionData")),
                old("lingerDrag", vector()),
                old("lingerColor", vector()),
                old("useLingerColor", kind("Bool")),
                old("UseLingerScale", kind("Bool")),
                new("FlexShapeDefinition", to("VfxFlexShapeDefinitionData")),
                old("scaleEmitOffsetByBoundObjectSize", kind("F32")),
                old("scaleEmitOffsetByBoundObjectHeight", kind("F32")),
                old("scaleEmitOffsetByBoundObjectRadius", kind("F32")),
                old("flexScaleEmitOffset", to("FlexValueVector3")),
                old("flexBirthTranslation", to("FlexValueVector3")),
                old("scaleBirthScaleByBoundObjectSize", kind("F32")),
                old("scaleBirthScaleByBoundObjectHeight", kind("F32")),
                old("scaleBirthScaleByBoundObjectRadius", kind("F32")),
                new("LegacySimple", to("VfxEmitterLegacySimple")),
                old("birthScale1", vector()),
                old("particleBind", kind("Vec2")),
                old("scaleUpFromOrigin", kind("Bool")),
                new("textureMult", to("VfxTextureMultDefinitionData")),
                old("particleUVScrollRateMult", vector()),
                old("UvRotationMult", vector()),
                old("TextureMultFlipU", kind("Bool")),
            ],
        )
        .class(
            "VfxEmitterFiltering",
            &[
                new("keywordsExcluded", kind("String")),
                new("spectatorPolicy", kind("U8")),
            ],
        )
        .class(
            "VfxLingerDefinitionData",
            &[
                new("KeyedLingerDrag", vector()),
                new("SeparateLingerColor", vector()),
                new("UseSeparateLingerColor", kind("Bool")),
                new("UseLingerScale", kind("Bool")),
            ],
        )
        .class(
            "VfxFlexShapeDefinitionData",
            &[
                new("scaleEmitOffsetByBoundObjectSize", kind("F32")),
                new("scaleEmitOffsetByBoundObjectHeight", kind("F32")),
                new("scaleEmitOffsetByBoundObjectRadius", kind("F32")),
                new("flexScaleEmitOffset", to("FlexValueVector3")),
                new("flexBirthTranslation", to("FlexValueVector3")),
                new("scaleBirthScaleByBoundObjectSize", kind("F32")),
                new("scaleBirthScaleByBoundObjectHeight", kind("F32")),
                new("scaleBirthScaleByBoundObjectRadius", kind("F32")),
            ],
        )
        // The class holds no `scaleUpFromOrigin`: a field the new class does not declare.
        .class(
            "VfxEmitterLegacySimple",
            &[
                new("birthScale", vector()),
                new("particleBind", kind("Vec2")),
            ],
        )
        .class(
            "VfxTextureMultDefinitionData",
            &[
                new("textureMult", kind("String")),
                new("ParticleIntegratedUvScrollMult", vector()),
                new("UvRotationMult", vector()),
                new("TextureMultFilpU", kind("Bool")),
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

fn vector(x: f32) -> V {
    embedded("ValueVector3", vec![("constantValue", float(x))])
}

fn constant(value: Option<&V>) -> Option<f32> {
    match field_of(value?, "constantValue")? {
        V::F32(x) => Some(x.value),
        _ => None,
    }
}

fn emitter(entry: &str, fields: Vec<(&str, V)>) -> BinObject {
    object(entry, EMITTER, fields)
}

fn archive(objects: Vec<BinObject>) -> ModArchive {
    skin_archive(objects).file(skins::WAD, DATED, dated_bytes())
}

/// The built-in table rules alone, so a fixup the standard set gains does not change what a
/// test of them sees.
fn the_rules() -> FixupSet {
    let rules = rules::all().into_iter();
    rules.fold(FixupSet::empty(), FixupSet::with)
}

/// One emitter fixed by the rules, for a mod with no date.
fn fixed<'a>(env: &'a Env, fields: Vec<(&str, V)>) -> FixRun<'a> {
    FixRun::of(env, &the_rules(), &archive(vec![emitter("E", fields)]))
}

fn gone(run: &FixRun<'_>, fields: &[&str]) {
    for name in fields {
        assert!(
            run.field("E", name).is_none(),
            "{name} stays on the emitter"
        );
    }
}

#[test]
fn an_emitters_filters_move_into_filtering() {
    let env = env(false);
    let run = fixed(
        &env,
        vec![
            ("keywordsExcluded", text("spectator")),
            ("spectatorPolicy", values::U8::new(1).into()),
        ],
    );
    let filtering = run.child("E", "Filtering");
    assert_eq!(filtering.class_hash, name_hash("VfxEmitterFiltering"));
    let excluded = filtering.properties.get(&name_hash("keywordsExcluded"));
    assert_eq!(excluded.and_then(as_str), Some("spectator"));
    assert!(
        filtering
            .properties
            .contains_key(&name_hash("spectatorPolicy"))
    );
    gone(&run, &["keywordsExcluded", "spectatorPolicy"]);
    assert_eq!(run.changes.of("vfx-filtering").len(), 2);
    assert_eq!(run.changes.len(), 2);
}

#[test]
fn an_emitters_linger_fields_move_into_linger_under_their_new_names() {
    let env = env(false);
    let run = fixed(
        &env,
        vec![
            ("lingerDrag", vector(2.0)),
            ("lingerColor", vector(8.0)),
            ("useLingerColor", flag(true)),
            ("UseLingerScale", flag(true)),
        ],
    );
    let linger = run.child("E", "Linger");
    assert_eq!(linger.class_hash, name_hash("VfxLingerDefinitionData"));
    let get = |name: &str| linger.properties.get(&name_hash(name));
    assert_eq!(constant(get("KeyedLingerDrag")), Some(2.0));
    assert_eq!(constant(get("SeparateLingerColor")), Some(8.0));
    assert_eq!(get("UseSeparateLingerColor"), Some(&flag(true)));
    assert_eq!(get("UseLingerScale"), Some(&flag(true)));
    gone(
        &run,
        &[
            "lingerDrag",
            "lingerColor",
            "useLingerColor",
            "UseLingerScale",
        ],
    );
    assert_eq!(run.changes.of("vfx-linger").len(), 4);
}

#[test]
fn an_emitters_bound_object_scale_moves_into_the_flex_shape() {
    let env = env(false);
    let run = fixed(
        &env,
        vec![("scaleEmitOffsetByBoundObjectSize", float(0.25))],
    );
    let flex = run.child("E", "FlexShapeDefinition");
    assert_eq!(flex.class_hash, name_hash("VfxFlexShapeDefinitionData"));
    assert_eq!(
        flex.properties
            .get(&name_hash("scaleEmitOffsetByBoundObjectSize")),
        Some(&float(0.25))
    );
    gone(&run, &["scaleEmitOffsetByBoundObjectSize"]);
}

#[test]
fn every_field_riot_moved_into_the_flex_shape_moves() {
    // #GAME-CHANGE-0016: the five fields of 12.6 and the three of 12.21, each under its own
    // name, in the order of the record.
    let env = env(false);
    let flex = |x: f32| pointer("FlexValueVector3", vec![("mValue", vector(x))]);
    let moved = [
        ("scaleEmitOffsetByBoundObjectSize", float(0.25)),
        ("scaleEmitOffsetByBoundObjectHeight", float(0.5)),
        ("scaleEmitOffsetByBoundObjectRadius", float(0.75)),
        ("flexScaleEmitOffset", flex(1.0)),
        ("flexBirthTranslation", flex(2.0)),
        ("scaleBirthScaleByBoundObjectSize", float(1.25)),
        ("scaleBirthScaleByBoundObjectHeight", float(1.5)),
        ("scaleBirthScaleByBoundObjectRadius", float(1.75)),
    ];
    // The emitter holds them in another order than the record's.
    let fields: Vec<(&str, V)> = moved.iter().rev().cloned().collect();
    let run = fixed(&env, fields);
    let held = run.child("E", "FlexShapeDefinition");
    let expected: Vec<_> = moved.iter().map(|(name, _)| name_hash(name)).collect();
    assert_eq!(
        held.properties.keys().copied().collect::<Vec<_>>(),
        expected
    );
    for (name, value) in &moved {
        assert_eq!(held.properties.get(&name_hash(name)), Some(value), "{name}");
    }
    let names: Vec<&str> = moved.iter().map(|(name, _)| *name).collect();
    gone(&run, &names);
    assert_eq!(run.changes.of("vfx-flex-shape").len(), 8);
}

#[test]
fn an_emitters_simple_particle_fields_move_into_legacy_simple() {
    let env = env(false);
    let bind = V::Vector2(values::Vector2::default());
    let run = fixed(
        &env,
        vec![
            ("birthScale1", vector(4.0)),
            ("particleBind", bind.clone()),
            ("scaleUpFromOrigin", flag(true)),
        ],
    );
    let simple = run.child("E", "LegacySimple");
    assert_eq!(simple.class_hash, name_hash("VfxEmitterLegacySimple"));
    let get = |name: &str| simple.properties.get(&name_hash(name));
    // 12.19 dropped the `1` of the name.
    assert_eq!(constant(get("birthScale")), Some(4.0));
    assert_eq!(get("particleBind"), Some(&bind));
    gone(&run, &["birthScale1", "particleBind"]);
    // The new class does not declare it: it stays where it is.
    assert_eq!(run.field("E", "scaleUpFromOrigin"), Some(&flag(true)));
    assert_eq!(get("scaleUpFromOrigin"), None);
}

#[test]
fn an_emitters_second_texture_and_its_fields_move_into_texture_mult() {
    let env = env(false);
    let run = fixed(
        &env,
        vec![
            ("textureMult", text("assets/mod/a/mult.tex")),
            ("particleUVScrollRateMult", vector(5.0)),
            ("TextureMultFlipU", flag(true)),
        ],
    );
    let mult = run.child("E", "textureMult");
    assert_eq!(mult.class_hash, name_hash("VfxTextureMultDefinitionData"));
    let get = |name: &str| mult.properties.get(&name_hash(name));
    assert_eq!(
        get("textureMult").and_then(as_str),
        Some("assets/mod/a/mult.tex")
    );
    assert_eq!(constant(get("ParticleIntegratedUvScrollMult")), Some(5.0));
    // The game's own spelling of the new name.
    assert_eq!(get("TextureMultFilpU"), Some(&flag(true)));
    gone(&run, &["particleUVScrollRateMult", "TextureMultFlipU"]);
}

#[test]
fn one_emitter_is_laid_out_by_every_rule() {
    let env = env(false);
    let run = fixed(
        &env,
        vec![
            ("birthScale1", vector(4.0)),
            ("textureMult", text("assets/mod/a/mult.tex")),
            ("particleUVScrollRateMult", vector(5.0)),
            ("scaleEmitOffsetByBoundObjectSize", float(0.25)),
            ("keywordsExcluded", text("spectator")),
            ("lingerColor", vector(8.0)),
        ],
    );
    // A struct made takes the last place among the emitter's fields, in the order the rules
    // run.
    let held: Vec<_> = run.object("E").properties.keys().copied().collect();
    let expected = [
        "FlexShapeDefinition",
        "Filtering",
        "Linger",
        "LegacySimple",
        "textureMult",
    ];
    assert_eq!(held, expected.map(name_hash));
    assert_eq!(run.changes.len(), 6);
    let by_fixup: Vec<&str> = run.changes.by_fixup().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        by_fixup,
        [
            "vfx-flex-shape",
            "vfx-filtering",
            "vfx-linger",
            "vfx-legacy-simple",
            "vfx-texture-mult"
        ]
    );
}

#[test]
fn a_field_left_beside_a_struct_already_made_joins_it() {
    let env = env(false);
    let made = pointer(
        "VfxTextureMultDefinitionData",
        vec![("textureMult", text("assets/mod/a/mult.tex"))],
    );
    let run = fixed(
        &env,
        vec![("textureMult", made), ("UvRotationMult", vector(6.0))],
    );
    let mult = run.child("E", "textureMult");
    let get = |name: &str| mult.properties.get(&name_hash(name));
    assert_eq!(constant(get("UvRotationMult")), Some(6.0));
    // The struct's own path stays where it is: the field moves only while it holds a string.
    assert_eq!(
        get("textureMult").and_then(as_str),
        Some("assets/mod/a/mult.tex")
    );
    gone(&run, &["UvRotationMult"]);
    assert_eq!(run.changes.len(), 1);
}

#[test]
fn the_value_a_struct_holds_wins_over_the_one_left_on_the_emitter() {
    let env = env(false);
    let made = pointer(
        "VfxEmitterFiltering",
        vec![("keywordsExcluded", text("kept"))],
    );
    let run = fixed(
        &env,
        vec![("Filtering", made), ("keywordsExcluded", text("dropped"))],
    );
    let filtering = run.child("E", "Filtering");
    let excluded = filtering.properties.get(&name_hash("keywordsExcluded"));
    assert_eq!(excluded.and_then(as_str), Some("kept"));
    gone(&run, &["keywordsExcluded"]);
    let [change] = run.changes.of("vfx-filtering") else {
        panic!("one change: {:?}", run.changes);
    };
    assert_eq!(change.after, After::Lost);
    assert_eq!(run.raised::<Lost>("vfx-filtering").len(), 1);
    assert!(run.raised::<Changed>("vfx-filtering").is_empty());
}

#[test]
fn a_layout_the_mods_own_build_did_not_read_is_left() {
    // Made at 150 or later, when every build since read the new layout: the old fields never
    // showed, and moving them would change what the author saw.
    let env = env(true);
    let fields = vec![
        ("birthScale1", vector(4.0)),
        ("keywordsExcluded", text("spectator")),
        ("textureMult", text("assets/mod/a/mult.tex")),
        ("scaleEmitOffsetByBoundObjectSize", float(0.25)),
        ("lingerColor", vector(8.0)),
    ];
    let run = fixed(&env, fields.clone());
    assert_eq!(run.dating.build(), Some(150));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("E"), &emitter("E", fields));
}

#[test]
fn the_rules_applied_twice_change_nothing_the_second_time() {
    let made = |class: &str, fields| pointer(class, fields);
    let objects = vec![
        emitter(
            "Old",
            vec![
                ("birthScale1", vector(4.0)),
                ("particleBind", V::Vector2(values::Vector2::default())),
                ("scaleUpFromOrigin", flag(true)),
                ("textureMult", text("assets/mod/a/mult.tex")),
                ("particleUVScrollRateMult", vector(5.0)),
                ("TextureMultFlipU", flag(true)),
                ("scaleEmitOffsetByBoundObjectSize", float(0.25)),
                ("keywordsExcluded", text("spectator")),
                ("spectatorPolicy", values::U8::new(1).into()),
                ("lingerDrag", vector(2.0)),
                ("lingerColor", vector(8.0)),
                ("useLingerColor", flag(true)),
                ("UseLingerScale", flag(true)),
            ],
        ),
        emitter(
            "Half",
            vec![
                (
                    "textureMult",
                    made(
                        "VfxTextureMultDefinitionData",
                        vec![("textureMult", text("assets/mod/a/mult.tex"))],
                    ),
                ),
                ("UvRotationMult", vector(6.0)),
                (
                    "Filtering",
                    made(
                        "VfxEmitterFiltering",
                        vec![("keywordsExcluded", text("kept"))],
                    ),
                ),
                ("keywordsExcluded", text("dropped")),
            ],
        ),
    ];
    for dated in [false, true] {
        let env = env(dated);
        let set = the_rules();
        let once = FixRun::of(&env, &set, &archive(objects.clone()));
        assert_eq!(once.changes.is_empty(), dated);
        let after = [once.object("Old").clone(), once.object("Half").clone()];
        let twice = once.again(&set);
        assert!(twice.changes.is_empty(), "{:?}", twice.changes);
        assert_eq!(
            [twice.object("Old"), twice.object("Half")],
            [&after[0], &after[1]]
        );
    }
}

#[test]
fn a_rule_runs_where_the_target_reads_the_struct() {
    // A target of the old layout declares no `Filtering`: no rule applies to it.
    let old_target = Env::builder()
        .game(skins::game_at("1.0.110"))
        .schema(schema())
        .build()
        .expect("an environment");
    let run = fixed(&old_target, vec![("keywordsExcluded", text("spectator"))]);
    assert!(run.changes.is_empty());
    assert!(run.field("E", "keywordsExcluded").is_some());
}

#[test]
fn the_built_in_rules_name_their_records_and_builds() {
    let rows: Vec<(String, String, String)> = rules::all()
        .iter()
        .map(|rule| {
            use reforge::fixups::BinFixup as _;
            let info = rule.info();
            let build = info.phase.build().expect("a layout rule");
            let record = info.one_record().expect("one record");
            (info.id.to_string(), build.to_string(), record.to_string())
        })
        .collect();
    let row = |id: &str, build: &str, record: &str| (id.into(), build.into(), record.into());
    assert_eq!(
        rows,
        [
            row("vfx-filtering", "12.17", "#GAME-CHANGE-0002"),
            row("vfx-linger", "12.18", "#GAME-CHANGE-0003"),
            row("vfx-flex-shape", "12.6", "#GAME-CHANGE-0016"),
            row("vfx-legacy-simple", "12.19", "#GAME-CHANGE-0004"),
            row("vfx-texture-mult", "13.1", "#GAME-CHANGE-0005"),
        ]
    );
}
