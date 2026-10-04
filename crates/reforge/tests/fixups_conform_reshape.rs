//! Reshape: a value wrapped in the struct the build reads it in, and a struct a tool nested
//! in itself taken out.

use ltk_meta::PropertyValueEnum as V;
use reforge::env::Env;
use reforge::fixups::conform::{Hoist, Retype, StripUndeclared, Wrap};
use reforge::fixups::{BinFixup, Change, FixupSet, rules};
use reforge::game::Schema;
use reforge::model::{Phase, Record, name_hash};
use reforge_testkit::bins::{embedded, float, object, pointer, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{embed, field, kind, pointer as to};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const EMITTER: &str = "TestEmitter";
const MULT: &str = "TestMult";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";
const TEXTURE: &str = "assets/mod/a/mult.tex";

/// The emitter's `textureMult` has been a pointer to a `TestMult` since 100, the schema's
/// first build. `TestMult` holds the texture in its own `textureMult` beside `texDivMult`,
/// which the emitter does not declare. `asEmbed` is the same with an embed.
fn schema() -> Schema {
    SchemaRows::new(300)
        .class(
            EMITTER,
            &[
                field("textureMult", to(MULT)),
                field("kept", kind("F32")),
                field("asEmbed", embed("TestEmbedded")),
            ],
        )
        .class(
            MULT,
            &[
                field("textureMult", kind("String")),
                field("texDivMult", kind("F32")),
                field("kept", kind("F32")),
            ],
        )
        .class("TestEmbedded", &[field("asEmbed", kind("F32"))])
        .build()
}

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

/// The target at build 300. With `dated`, a history in which the mod's dated file first
/// shipped at build 150.
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

fn env(dated: bool) -> Env {
    env_of(schema(), dated)
}

fn mult(fields: Vec<(&str, V)>) -> V {
    pointer(MULT, fields)
}

/// The old layout: the texture's path and its `texDivMult` on the emitter.
fn flat() -> Vec<(&'static str, V)> {
    vec![
        ("textureMult", text(TEXTURE)),
        ("texDivMult", float(2.0)),
        ("kept", float(3.0)),
    ]
}

/// A `TestMult` wrapped in another.
fn twice() -> V {
    let inner = mult(vec![
        ("textureMult", text(TEXTURE)),
        ("texDivMult", float(4.0)),
    ]);
    mult(vec![("textureMult", inner)])
}

fn archive(fields: Vec<(&str, V)>) -> ModArchive {
    skin_archive(vec![object("E", EMITTER, fields)]).file(skins::WAD, DATED, dated_bytes())
}

/// One emitter fixed by a set. The run's names learn the fields of the test schema, so that
/// a path of a change spells them.
fn fixed<'a>(env: &'a Env, set: &FixupSet, fields: Vec<(&str, V)>) -> FixRun<'a> {
    let mut run = FixRun::of(env, set, &archive(fields));
    for name in [
        "textureMult",
        "texDivMult",
        "kept",
        "asEmbed",
        "UvRotationMult",
    ] {
        run.names.learn_field(name);
    }
    run
}

fn wrap() -> FixupSet {
    FixupSet::empty().with(Wrap)
}

fn hoist() -> FixupSet {
    FixupSet::empty().with(Hoist)
}

/// What a run did, as where the value was, its kind and what is there after.
fn rows(changes: &[Change], run: &FixRun<'_>) -> Vec<(String, &'static str, String)> {
    use reforge::fixups::After;
    let speller = run.speller();
    changes
        .iter()
        .map(|change| {
            let before = change.before.as_ref().map_or("-", |before| before.kind);
            let after = match &change.after {
                After::Value(after) => format!("{} {}", after.kind, after.text),
                After::Moved { to } => format!("at {}", to.spelled(&speller)),
                After::Removed => "removed".to_string(),
                After::Lost => "lost".to_string(),
                After::Unconvertible { wants } => format!("left for {wants}"),
            };
            (change.path.spelled(&speller), before, after)
        })
        .collect()
}

#[test]
fn a_flat_value_is_wrapped_with_the_fields_its_struct_declares() {
    let env = env(false);
    let run = fixed(&env, &wrap(), flat());
    let made = run.child("E", "textureMult");
    assert_eq!(made.class_hash, name_hash(MULT));
    let held: Vec<_> = made.properties.keys().copied().collect();
    assert_eq!(held, ["textureMult", "texDivMult"].map(name_hash));
    assert_eq!(
        made.properties.get(&name_hash("textureMult")),
        Some(&text(TEXTURE))
    );
    assert_eq!(
        made.properties.get(&name_hash("texDivMult")),
        Some(&float(2.0))
    );
    // `texDivMult` moved. `kept`, which the emitter declares too, stays.
    assert_eq!(run.field("E", "texDivMult"), None);
    assert_eq!(run.field("E", "kept"), Some(&float(3.0)));
    // The struct takes the place the value had.
    let held: Vec<_> = run.object("E").properties.keys().copied().collect();
    assert_eq!(held, ["textureMult", "kept"].map(name_hash));
    assert_eq!(
        rows(run.changes.of("reshape-wrap"), &run),
        [
            (
                "textureMult".into(),
                "String",
                "at textureMult.textureMult".into()
            ),
            (
                "texDivMult".into(),
                "F32",
                "at textureMult.texDivMult".into()
            ),
        ]
    );
}

#[test]
fn a_value_is_wrapped_in_an_embed_where_the_build_reads_one() {
    let env = env(false);
    let run = fixed(&env, &wrap(), vec![("asEmbed", float(5.0))]);
    assert_eq!(
        run.field("E", "asEmbed"),
        Some(&embedded("TestEmbedded", vec![("asEmbed", float(5.0))]))
    );
}

#[test]
fn a_struct_or_a_value_of_a_struct_with_no_such_field_is_not_wrapped() {
    let env = env(false);
    // `kept` is a float on the emitter, and `textureMult` holds its struct already.
    let fields = vec![("textureMult", mult(vec![])), ("kept", float(3.0))];
    let run = fixed(&env, &wrap(), fields.clone());
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object("E"), &object("E", EMITTER, fields));
}

#[test]
fn a_struct_nested_in_itself_is_taken_out() {
    let env = env(false);
    let run = fixed(&env, &hoist(), vec![("textureMult", twice())]);
    assert_eq!(
        run.field("E", "textureMult"),
        Some(&mult(vec![
            ("textureMult", text(TEXTURE)),
            ("texDivMult", float(4.0))
        ]))
    );
    assert_eq!(
        rows(run.changes.of("reshape-hoist"), &run),
        [
            (
                "textureMult.textureMult.texDivMult".into(),
                "F32",
                "at textureMult.texDivMult".into()
            ),
            (
                "textureMult.textureMult".into(),
                "Pointer",
                format!("String {TEXTURE:?}")
            ),
        ]
    );
}

#[test]
fn the_outer_struct_keeps_its_own_value_where_both_hold_one() {
    let env = env(false);
    let inner = mult(vec![
        ("textureMult", text(TEXTURE)),
        ("texDivMult", float(4.0)),
        ("kept", float(5.0)),
    ]);
    let outer = mult(vec![("textureMult", inner), ("texDivMult", float(1.0))]);
    let run = fixed(&env, &hoist(), vec![("textureMult", outer)]);
    assert_eq!(
        run.field("E", "textureMult"),
        Some(&mult(vec![
            ("textureMult", text(TEXTURE)),
            ("texDivMult", float(1.0)),
            ("kept", float(5.0)),
        ]))
    );
    assert_eq!(
        rows(run.changes.of("reshape-hoist"), &run),
        [
            (
                "textureMult.textureMult.texDivMult".into(),
                "F32",
                "removed".into()
            ),
            (
                "textureMult.textureMult.kept".into(),
                "F32",
                "at textureMult.kept".into()
            ),
            (
                "textureMult.textureMult".into(),
                "Pointer",
                format!("String {TEXTURE:?}")
            ),
        ]
    );
}

#[test]
fn a_struct_nested_three_deep_is_taken_out_to_the_value() {
    let env = env(false);
    let deepest = mult(vec![("textureMult", text(TEXTURE)), ("kept", float(9.0))]);
    let middle = mult(vec![("textureMult", deepest), ("texDivMult", float(4.0))]);
    let run = fixed(
        &env,
        &hoist(),
        vec![("textureMult", mult(vec![("textureMult", middle)]))],
    );
    assert_eq!(
        run.field("E", "textureMult"),
        Some(&mult(vec![
            ("textureMult", text(TEXTURE)),
            ("texDivMult", float(4.0)),
            ("kept", float(9.0)),
        ]))
    );
    let rows = rows(run.changes.of("reshape-hoist"), &run);
    assert_eq!(
        rows[1],
        (
            "textureMult.textureMult.textureMult.kept".into(),
            "F32",
            "at textureMult.kept".into()
        )
    );
    assert_eq!(rows.len(), 3);
}

#[test]
fn a_nested_struct_with_no_value_of_its_own_leaves_only_its_fields() {
    let env = env(false);
    let inner = mult(vec![("texDivMult", float(4.0))]);
    let run = fixed(
        &env,
        &hoist(),
        vec![("textureMult", mult(vec![("textureMult", inner)]))],
    );
    assert_eq!(
        run.field("E", "textureMult"),
        Some(&mult(vec![("texDivMult", float(4.0))]))
    );
    assert_eq!(
        rows(run.changes.of("reshape-hoist"), &run),
        [
            (
                "textureMult.textureMult.texDivMult".into(),
                "F32",
                "at textureMult.texDivMult".into()
            ),
            (
                "textureMult.textureMult".into(),
                "Pointer",
                "removed".into()
            ),
        ]
    );
}

#[test]
fn a_shape_the_mods_own_build_did_not_read_is_not_revived() {
    // Made at 150, when the emitter already read a pointer and the struct's own
    // `textureMult` a string: neither old shape showed, and retype removes both.
    let env = env(true);
    let set = FixupSet::empty().with(Hoist).with(Wrap).with(Retype);
    let run = fixed(&env, &set, flat());
    assert_eq!(run.dating.build(), Some(150));
    assert_eq!(run.field("E", "textureMult"), None);
    assert_eq!(run.field("E", "texDivMult"), Some(&float(2.0)));
    assert!(run.changes.of("reshape-wrap").is_empty());

    let run = fixed(&env, &set, vec![("textureMult", twice())]);
    assert_eq!(run.field("E", "textureMult"), Some(&mult(vec![])));
    assert!(run.changes.of("reshape-hoist").is_empty());
    assert_eq!(
        rows(run.changes.of("retype"), &run),
        [(
            "textureMult.textureMult".into(),
            "Pointer",
            "removed".into()
        )]
    );
}

#[test]
fn a_field_a_layout_rule_joins_to_the_outer_struct_is_kept_over_the_inner_one() {
    // The texture-mult rule moves an emitter's `UvRotationMult` into the struct in
    // `textureMult`, here the outer of two. The hoist runs after it, so the emitter's
    // value is the one kept.
    let emitter = "VfxEmitterDefinitionData";
    let class = "VfxTextureMultDefinitionData";
    let schema = SchemaRows::new(300)
        .class(
            emitter,
            &[
                field("textureMult", to(class)).from(121),
                field("UvRotationMult", kind("F32")).until(120),
            ],
        )
        .class(
            class,
            &[
                field("textureMult", kind("String")).from(121),
                field("UvRotationMult", kind("F32")).from(121),
            ],
        )
        .build();
    let env = env_of(schema, false);
    let set = FixupSet::empty()
        .with(Hoist)
        .with(Wrap)
        .with(rules::texture_mult());
    assert_eq!(
        set.ids(),
        ["vfx-texture-mult", "reshape-hoist", "reshape-wrap"]
    );
    let inner = pointer(
        class,
        vec![
            ("textureMult", text(TEXTURE)),
            ("UvRotationMult", float(4.0)),
        ],
    );
    let fields = vec![
        ("textureMult", pointer(class, vec![("textureMult", inner)])),
        ("UvRotationMult", float(6.0)),
    ];
    let object = object("E", emitter, fields);
    let run = FixRun::of(&env, &set, &skin_archive(vec![object]));
    assert_eq!(
        run.field("E", "textureMult"),
        Some(&pointer(
            class,
            vec![
                ("textureMult", text(TEXTURE)),
                ("UvRotationMult", float(6.0))
            ]
        ))
    );
    assert_eq!(run.field("E", "UvRotationMult"), None);
}

#[test]
fn reshape_applied_twice_changes_nothing_the_second_time() {
    let mut fields = flat();
    fields.push(("asEmbed", float(5.0)));
    let nested = vec![("textureMult", twice()), ("kept", float(3.0))];
    for dated in [false, true] {
        let env = env(dated);
        let set = FixupSet::empty().with(Hoist).with(Wrap);
        for fields in [fields.clone(), nested.clone()] {
            let once = FixRun::of(&env, &set, &archive(fields));
            assert_eq!(once.changes.is_empty(), dated);
            let after = once.object("E").clone();
            let again = once.again(&set);
            assert!(again.changes.is_empty(), "{:?}", again.changes);
            assert_eq!(again.object("E"), &after);
        }
    }
}

#[test]
fn the_conforming_fixups_run_after_every_layout_and_strip_last() {
    let set = FixupSet::standard();
    let mine = [
        "reshape-hoist",
        "reshape-wrap",
        "retype",
        "strip-undeclared",
    ];
    let ids = set.ids();
    let found: Vec<&str> = ids.iter().copied().filter(|id| mine.contains(id)).collect();
    assert_eq!(found, mine);
    // Reshape comes before retype, which types what it moved, and nothing of an earlier
    // phase comes after either.
    let at = |id: &str| ids.iter().position(|held| *held == id).expect("in the set");
    let infos = set.infos();
    let before_conform = |phase: &Phase| *phase < Phase::Conform;
    assert!(
        infos[at("reshape-hoist")..]
            .iter()
            .all(|info| !before_conform(&info.phase))
    );
    assert!(
        infos[..at("reshape-hoist")]
            .iter()
            .all(|info| info.phase <= Phase::Conform)
    );

    let record = |fixup: &dyn BinFixup| fixup.info().one_record();
    assert_eq!(record(&Hoist), Some(Record::mod_artifact(18)));
    assert_eq!(record(&Wrap), Some(Record::game_change(5)));
    assert_eq!(record(&StripUndeclared), Some(Record::game_behavior(2)));
    assert_eq!(Hoist.info().phase, Phase::Conform);
    assert_eq!(Wrap.info().phase, Phase::Conform);
    assert_eq!(Retype.info().phase, Phase::Conform);
    assert_eq!(StripUndeclared.info().phase, Phase::Strip);
}
