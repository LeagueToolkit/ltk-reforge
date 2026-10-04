//! The texture-mult strays: an emitter's own simple-particle fields that a tool nested in its
//! `textureMult`, back on the emitter, where the rule that gathers them finds them.

use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::diag::Record;
use reforge::env::Env;
use reforge::fixups::undo::TextureMultStrays;
use reforge::fixups::{After, BinFixup, Changed, FixupSet, Outcome, Removed, rules};
use reforge::game::Schema;
use reforge::model::{Phase, name_hash};
use reforge_testkit::bins::{embedded, field_of, float, object, pointer, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{FieldRow, Type, embed, field, kind, pointer as to};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const EMITTER: &str = "VfxEmitterDefinitionData";
const MULT: &str = "VfxTextureMultDefinitionData";
const ID: &str = "vfx-texture-mult-strays";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";

fn old(name: &str, of: Type) -> FieldRow {
    field(name, of).until(120)
}

fn new(name: &str, of: Type) -> FieldRow {
    field(name, of).from(121)
}

/// The emitter's simple-particle fields on the emitter up to build 120, and from 121 in
/// `LegacySimple`, with `textureMult` a struct. With `mult_binds`, a target whose texture-mult
/// class declares `particleBind`.
fn schema(mult_binds: bool) -> Schema {
    let vector = || embed("ValueVector3");
    let mut mult = vec![
        new("textureMult", kind("String")),
        new("UvRotationMult", vector()),
    ];
    if mult_binds {
        mult.push(new("particleBind", kind("Vec2")));
    }
    SchemaRows::new(300)
        .class(
            EMITTER,
            &[
                new("LegacySimple", to("VfxEmitterLegacySimple")),
                old("birthScale1", vector()),
                old("scale1", vector()),
                old("particleBind", kind("Vec2")),
                new("textureMult", to(MULT)),
                old("UvRotationMult", vector()),
            ],
        )
        .class(
            "VfxEmitterLegacySimple",
            &[
                new("birthScale", vector()),
                new("scale", vector()),
                new("particleBind", kind("Vec2")),
            ],
        )
        .class(MULT, &mult)
        .build()
}

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

fn env_of(schema: Schema, dated: bool) -> Env {
    let mut env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema);
    if dated {
        let history = MemoryHistory::new(&["1.0.50", "1.0.150", "16.19.300"]);
        env = env.history(Box::new(history.with_content(DATED, &dated_bytes(), 1..=2)));
    }
    env.build().expect("an environment")
}

fn env() -> Env {
    env_of(schema(false), false)
}

fn vector(x: f32) -> V {
    embedded("ValueVector3", vec![("constantValue", float(x))])
}

fn bind() -> V {
    ltk_meta::property::values::Vector2::default().into()
}

fn constant(value: Option<&V>) -> Option<f32> {
    match field_of(value?, "constantValue")? {
        V::F32(x) => Some(x.value),
        _ => None,
    }
}

fn mult(fields: Vec<(&str, V)>) -> V {
    pointer(MULT, fields)
}

fn emitter(entry: &str, fields: Vec<(&str, V)>) -> BinObject {
    object(entry, EMITTER, fields)
}

fn archive(objects: Vec<BinObject>) -> ModArchive {
    skin_archive(objects).file(skins::WAD, DATED, dated_bytes())
}

/// The fixup alone, so that what is left on the emitter is what it left there.
fn strays_only() -> FixupSet {
    FixupSet::empty().with(TextureMultStrays::new())
}

/// The fixup and the rule that gathers what it takes back.
fn with_legacy_simple() -> FixupSet {
    strays_only().with(rules::legacy_simple())
}

/// A tool's `textureMult`, wrapped twice, holding the emitter's own `birthScale1`.
fn nested() -> BinObject {
    let inner = mult(vec![
        ("birthScale1", vector(7.0)),
        ("UvRotationMult", vector(9.0)),
    ]);
    emitter(
        "Nested",
        vec![("textureMult", mult(vec![("textureMult", inner)]))],
    )
}

/// The struct in a field of a struct value.
fn inner<'a>(value: &'a V, name: &str) -> &'a V {
    field_of(value, name).unwrap_or_else(|| panic!("no {name} in {value:?}"))
}

#[test]
fn a_nested_field_goes_back_to_the_emitter_and_on_into_legacy_simple() {
    let env = env();
    let run = FixRun::of(&env, &with_legacy_simple(), &archive(vec![nested()]));
    let simple = run.child("Nested", "LegacySimple");
    assert_eq!(
        constant(simple.properties.get(&name_hash("birthScale"))),
        Some(7.0)
    );
    assert!(run.field("Nested", "birthScale1").is_none());
    let outer = run.field("Nested", "textureMult").expect("the struct");
    let nested = inner(outer, "textureMult");
    assert!(field_of(nested, "birthScale1").is_none());
    // A field the struct's class declares stays, for the fixup that unwraps the struct.
    assert_eq!(constant(field_of(nested, "UvRotationMult")), Some(9.0));
    assert_eq!(run.changes.of(ID).len(), 1);
    assert_eq!(run.changes.of("vfx-legacy-simple").len(), 1);
}

#[test]
fn the_move_is_recorded_from_its_place_in_the_struct_to_the_emitter() {
    let env = env();
    let run = FixRun::of(&env, &strays_only(), &archive(vec![nested()]));
    assert_eq!(constant(run.field("Nested", "birthScale1")), Some(7.0));
    let [change] = run.changes.of(ID) else {
        panic!("one change: {:?}", run.changes);
    };
    let speller = run.speller();
    assert_eq!(change.object, run.id("Nested"));
    assert_eq!(
        change.path.spelled(&speller),
        "textureMult.textureMult.birthScale1"
    );
    let After::Moved { to } = &change.after else {
        panic!("{change:?}");
    };
    assert_eq!(to.spelled(&speller), "birthScale1");
    assert_eq!(change.outcome(), Outcome::Changed);
    assert_eq!(
        (to.last_field(), change.path.last_field()),
        (
            Some((name_hash(EMITTER), name_hash("birthScale1"))),
            Some((name_hash(MULT), name_hash("birthScale1")))
        )
    );

    let [changed] = run.raised::<Changed>(ID)[..] else {
        panic!("one finding: {:?}", run.messages());
    };
    assert_eq!((changed.0.values, changed.0.objects), (1, 1));
    let raised = run.log.iter().find(|d| d.by.as_deref() == Some(ID));
    assert_eq!(
        raised.and_then(|d| d.record),
        Some(Record::mod_artifact(17))
    );
    assert_eq!(
        run.messages(),
        [format!(
            "fixup.changed {ID}: 1 value in 1 object kept, in the form the target build reads: \
             {MULT}.birthScale1 (1)"
        )]
    );
}

#[test]
fn every_simple_particle_field_is_taken_from_a_struct_wrapped_once_or_twice() {
    let env = env();
    let once = emitter(
        "Once",
        vec![(
            "textureMult",
            mult(vec![
                ("textureMult", text("assets/mod/a/mult.tex")),
                ("particleBind", bind()),
                ("scale1", vector(2.0)),
                ("birthScale1", vector(3.0)),
            ]),
        )],
    );
    // The struct as a build before 13.1 would hold it, were it an embed.
    let embed = embedded(
        MULT,
        vec![("particleBind", bind()), ("birthScale1", vector(4.0))],
    );
    let embedded = emitter("Embedded", vec![("textureMult", embed)]);
    let run = FixRun::of(&env, &strays_only(), &archive(vec![once, embedded]));

    // In the order the rule of the layout lists them, after the emitter's own fields.
    let held: Vec<_> = run.object("Once").properties.keys().copied().collect();
    let expected = ["textureMult", "birthScale1", "scale1", "particleBind"];
    assert_eq!(held, expected.map(name_hash));
    assert_eq!(constant(run.field("Once", "scale1")), Some(2.0));
    assert_eq!(constant(run.field("Once", "birthScale1")), Some(3.0));
    let left = run.child("Once", "textureMult");
    assert_eq!(left.properties.len(), 1, "{left:?}");

    assert_eq!(constant(run.field("Embedded", "birthScale1")), Some(4.0));
    assert_eq!(run.field("Embedded", "particleBind"), Some(&bind()));
    assert_eq!(run.changes.of(ID).len(), 5);
    let [changed] = run.raised::<Changed>(ID)[..] else {
        panic!("one finding: {:?}", run.messages());
    };
    assert_eq!((changed.0.values, changed.0.objects), (5, 2));
}

#[test]
fn the_emitters_own_value_wins_over_a_nested_copy() {
    let env = env();
    let held = emitter(
        "Held",
        vec![
            ("birthScale1", vector(1.0)),
            ("textureMult", mult(vec![("birthScale1", vector(2.0))])),
        ],
    );
    // The same field at both depths: the outer one is the emitter's, the inner one goes.
    let twice = emitter(
        "Twice",
        vec![(
            "textureMult",
            mult(vec![
                ("birthScale1", vector(5.0)),
                ("textureMult", mult(vec![("birthScale1", vector(6.0))])),
            ]),
        )],
    );
    let run = FixRun::of(&env, &strays_only(), &archive(vec![held, twice]));
    assert_eq!(constant(run.field("Held", "birthScale1")), Some(1.0));
    assert!(run.child("Held", "textureMult").properties.is_empty());
    assert_eq!(constant(run.field("Twice", "birthScale1")), Some(5.0));

    let speller = run.speller();
    let rows: Vec<(String, &After)> = run
        .changes
        .of(ID)
        .iter()
        .map(|c| (c.path.spelled(&speller), &c.after))
        .collect();
    let on_emitter =
        reforge::model::FieldPath::root().field(name_hash(EMITTER), name_hash("birthScale1"));
    let moved = After::Moved { to: on_emitter };
    assert_eq!(
        rows,
        [
            ("textureMult.birthScale1".to_string(), &After::Removed),
            ("textureMult.birthScale1".to_string(), &moved),
            (
                "textureMult.textureMult.birthScale1".to_string(),
                &After::Removed
            ),
        ]
    );
    assert_eq!(run.raised::<Removed>(ID).len(), 1);
    assert_eq!(run.raised::<Changed>(ID).len(), 1);
}

#[test]
fn what_is_no_struct_of_the_class_is_left() {
    let env = env();
    let objects = vec![
        // The old layout: a path, which the rule of the layout gathers.
        emitter(
            "Path",
            vec![
                ("textureMult", text("assets/mod/a/mult.tex")),
                ("birthScale1", vector(1.0)),
            ],
        ),
        // A struct of another class under the field.
        emitter(
            "Other",
            vec![(
                "textureMult",
                pointer("SomethingElse", vec![("birthScale1", vector(2.0))]),
            )],
        ),
        // A struct of the class with none of the fields.
        emitter(
            "Clean",
            vec![(
                "textureMult",
                mult(vec![("textureMult", text("assets/mod/a/mult.tex"))]),
            )],
        ),
        // The class elsewhere than under the emitter's field.
        object(
            "Elsewhere",
            "VfxSystemDefinitionData",
            vec![("textureMult", mult(vec![("birthScale1", vector(3.0))]))],
        ),
    ];
    let before = objects.clone();
    let run = FixRun::of(&env, &strays_only(), &archive(objects));
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    for object in &before {
        let entry = run
            .fixed
            .objects
            .values()
            .find(|o| o.entry == object.path_hash);
        assert_eq!(entry.map(|o| &o.value), Some(object));
    }
}

#[test]
fn a_field_the_structs_class_declares_is_no_stray() {
    // A target whose texture-mult class reads `particleBind`: the field stays in the struct.
    let env = env_of(schema(true), false);
    let nested = emitter(
        "E",
        vec![(
            "textureMult",
            mult(vec![("particleBind", bind()), ("birthScale1", vector(1.0))]),
        )],
    );
    let run = FixRun::of(&env, &strays_only(), &archive(vec![nested]));
    assert!(run.field("E", "particleBind").is_none());
    assert_eq!(constant(run.field("E", "birthScale1")), Some(1.0));
    let left = run.child("E", "textureMult");
    assert_eq!(
        left.properties.get(&name_hash("particleBind")),
        Some(&bind())
    );
    assert_eq!(run.changes.of(ID).len(), 1);
}

#[test]
fn a_mod_of_any_date_gets_its_fields_back_on_the_emitter() {
    // Made at build 150 or later: the rule of the layout leaves the emitter as that build
    // read it, and the strays are on the emitter all the same.
    let env = env_of(schema(false), true);
    let run = FixRun::of(&env, &with_legacy_simple(), &archive(vec![nested()]));
    assert_eq!(run.dating.build(), Some(150));
    assert_eq!(constant(run.field("Nested", "birthScale1")), Some(7.0));
    assert!(run.field("Nested", "LegacySimple").is_none());
    assert_eq!(run.changes.of(ID).len(), 1);
    assert!(run.changes.of("vfx-legacy-simple").is_empty());
}

#[test]
fn the_fixup_applied_twice_changes_nothing_the_second_time() {
    let held = emitter(
        "Held",
        vec![
            ("birthScale1", vector(1.0)),
            (
                "textureMult",
                mult(vec![("birthScale1", vector(2.0)), ("scale1", vector(3.0))]),
            ),
        ],
    );
    for (set, dated) in [
        (strays_only(), false),
        (with_legacy_simple(), false),
        (FixupSet::standard(), false),
        (FixupSet::standard(), true),
    ] {
        let env = env_of(schema(false), dated);
        let once = FixRun::of(&env, &set, &archive(vec![nested(), held.clone()]));
        assert_eq!(once.changes.of(ID).len(), 3);
        let after = [once.object("Nested").clone(), once.object("Held").clone()];
        let twice = once.again(&set);
        assert!(twice.changes.of(ID).is_empty(), "{:?}", twice.changes);
        assert!(twice.changes.of("vfx-legacy-simple").is_empty());
        assert_eq!(
            [twice.object("Nested"), twice.object("Held")],
            [&after[0], &after[1]]
        );
    }
}

#[test]
fn the_fixup_runs_before_every_layout_on_the_fields_of_the_legacy_simple_rule() {
    let fixup = TextureMultStrays::new();
    let info = fixup.info();
    assert_eq!(info.id, ID);
    assert_eq!(info.phase, Phase::Undo);
    assert!(info.default);
    assert_eq!(info.one_record(), Some(Record::mod_artifact(17)));

    let standard = FixupSet::standard();
    let ids = standard.ids();
    let at = |id: &str| ids.iter().position(|held| *held == id);
    assert!(at(ID).is_some());
    for layout in ["vfx-flex-shape", "vfx-legacy-simple", "vfx-texture-mult"] {
        assert!(at(ID) < at(layout), "{ID} and {layout} in {ids:?}");
    }

    // A target whose texture-mult class declares every field of the rule has no stray.
    let all: Vec<FieldRow> = rules::legacy_simple()
        .sources()
        .iter()
        .map(|name| field(name.as_str(), kind("F32")))
        .collect();
    assert_eq!(all.len(), 11);
    let declared = SchemaRows::new(300).class(MULT, &all).build();
    let env = env_of(declared, false);
    let run = FixRun::of(&env, &strays_only(), &archive(vec![nested()]));
    assert!(run.changes.is_empty());
    assert!(run.field("Nested", "birthScale1").is_none());
}
