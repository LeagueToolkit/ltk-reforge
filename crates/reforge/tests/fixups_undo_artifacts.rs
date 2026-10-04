//! The known artifacts: fields that tools write and no game build declared, removed from the
//! classes they are known on and reported apart from every other removal.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::diag::Record;
use reforge::env::Env;
use reforge::fixups::undo::artifacts::{self, TABLE};
use reforge::fixups::{After, BinFixup, FixupSelection, FixupSet, Outcome, Removed, Rule};
use reforge::game::Schema;
use reforge::model::{BinHash, Phase, name_hash};
use reforge_testkit::bins::{embedded, flag, float, object};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{embed, field, kind};
use reforge_testkit::{FixRun, SchemaRows, skins};

const EMITTER: &str = "VfxEmitterDefinitionData";
/// An empty colour curve a shared tool writes on a `ValueColor`.
const CURVE: BinHash = BinHash(0xecbe_c3b3);
/// A colour beside or instead of `constantValue`.
const COLOR: BinHash = BinHash(0x20e6_1d8a);
/// The two flags tools write on an emitter.
const FLAGS: [BinHash; 2] = [BinHash(0x0863_79e1), BinHash(0x82a5_15a5)];

/// `ValueColor` and the emitter as the target declares them, with none of the table's fields.
fn schema() -> Schema {
    SchemaRows::new(300)
        .class("ValueColor", &[field("constantValue", kind("Vec4"))])
        .class(
            EMITTER,
            &[
                field("emitterName", kind("String")),
                field("color", embed("ValueColor")),
            ],
        )
        .build()
}

fn env_of(schema: Schema) -> Env {
    Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema)
        .build()
        .expect("an environment")
}

fn env() -> Env {
    env_of(schema())
}

/// The fixups of the table and no other, so that what is left is what they left.
fn artifacts_only() -> FixupSet {
    artifacts::rules()
        .into_iter()
        .fold(FixupSet::empty(), FixupSet::with)
}

fn curve() -> V {
    V::Struct(values::Struct {
        class_hash: BinHash(0x44d0_38f0),
        properties: Default::default(),
    })
}

fn white() -> V {
    values::Vector4::default().into()
}

/// An object with fields by name, and fields by hash after them.
fn with_hashed(mut object: BinObject, fields: &[(BinHash, V)]) -> BinObject {
    for (hash, value) in fields {
        object.properties.insert(*hash, value.clone());
    }
    object
}

/// A colour holding the curve and a field of no table, and an object of another class
/// holding the curve's hash.
fn colors() -> Vec<BinObject> {
    let color = object(
        "Color",
        "ValueColor",
        vec![("constantValue", white()), ("junk", float(1.0))],
    );
    let other = object("Other", "ValueColor2", vec![]);
    vec![
        with_hashed(color, &[(CURVE, curve())]),
        with_hashed(other, &[(CURVE, float(2.0))]),
    ]
}

#[test]
fn a_field_no_game_build_declared_is_removed_from_the_class_it_is_known_on() {
    let env = env();
    let run = FixRun::of(&env, &artifacts_only(), &skin_archive(colors()));
    let color = run.object("Color");
    assert!(!color.properties.contains_key(&CURVE));
    assert!(color.properties.contains_key(&name_hash("constantValue")));
    // A field of no table is left for the strip of what the build does not declare.
    assert_eq!(run.field("Color", "junk"), Some(&float(1.0)));
    // Only the class the table names loses the field.
    assert_eq!(
        run.object("Other").properties.get(&CURVE),
        Some(&float(2.0))
    );

    let [change] = run.changes.of("known-artifacts-value-color") else {
        panic!("one change: {:?}", run.changes);
    };
    assert_eq!(change.object, run.id("Color"));
    assert_eq!(change.after, After::Removed);
    assert_eq!(change.outcome(), Outcome::Removed);
    assert_eq!(
        change.path.last_field(),
        Some((name_hash("ValueColor"), CURVE))
    );
    assert_eq!(run.changes.len(), 1);
}

#[test]
fn the_removal_is_raised_apart_under_the_rules_id_and_its_record() {
    let env = env();
    let run = FixRun::of(&env, &FixupSet::standard(), &skin_archive(colors()));
    let [removed] = run.raised::<Removed>("known-artifacts-value-color")[..] else {
        panic!("one finding: {:?}", run.messages());
    };
    assert_eq!((removed.0.values, removed.0.objects), (1, 1));
    let raised = run.log.iter().find(|d| {
        d.by.as_deref() == Some("known-artifacts-value-color") && d.code.name == "fixup.removed"
    });
    let raised = raised.expect("the finding");
    assert_eq!(raised.record, Some(Record::mod_artifact(20)));
    assert!(
        run.messages().contains(
            &"fixup.removed known-artifacts-value-color: 1 value in 1 object removed with \
              nothing lost: ValueColor.ecbec3b3 (1)"
                .to_string()
        ),
        "{:?}",
        run.messages()
    );
    // Whatever else the standard set removes, no other fixup counts the artifact.
    let others = run.changes.iter().filter(|c| {
        c.fixup != "known-artifacts-value-color"
            && c.path.last_field() == Some((name_hash("ValueColor"), CURVE))
    });
    assert_eq!(others.count(), 0);
}

#[test]
fn every_row_of_the_table_is_removed_wherever_its_class_is_a_struct() {
    let env = env();
    let color = embedded("ValueColor", vec![("constantValue", white())]);
    let V::Embedded(values::Embedded(mut color)) = color else {
        panic!("an embed");
    };
    color.properties.insert(COLOR, white());
    let emitter = object(
        "Emitter",
        EMITTER,
        vec![
            ("emitterName", reforge_testkit::bins::text("e")),
            ("color", V::Embedded(values::Embedded(color))),
            ("isLocalSpace", flag(false)),
        ],
    );
    let emitter = with_hashed(emitter, &[(FLAGS[0], flag(true)), (FLAGS[1], flag(true))]);
    let system = with_hashed(
        object("System", "VfxSystemDefinitionData", vec![]),
        &[(BinHash(0xc750_ee97), float(0.0))],
    );
    let material = with_hashed(
        object("Material", "StaticMaterialDef", vec![]),
        &[(BinHash(0xf3d3_de85), float(0.0))],
    );
    let archive = skin_archive(vec![emitter, system, material]);
    let run = FixRun::of(&env, &artifacts_only(), &archive);

    let held: Vec<BinHash> = run.object("Emitter").properties.keys().copied().collect();
    assert_eq!(held, [name_hash("emitterName"), name_hash("color")]);
    let Some(V::Embedded(values::Embedded(color))) = run.field("Emitter", "color") else {
        panic!("{:?}", run.object("Emitter"));
    };
    assert_eq!(color.properties.len(), 1, "{color:?}");
    assert!(run.object("System").properties.is_empty());
    assert!(run.object("Material").properties.is_empty());

    let counts: Vec<(String, usize)> = run
        .changes
        .by_fixup()
        .map(|(id, changes)| (id.to_string(), changes.len()))
        .collect();
    let row = |id: &str, n| (id.to_string(), n);
    assert_eq!(
        counts,
        [
            row("known-artifacts-static-material", 1),
            row("known-artifacts-value-color", 1),
            row("known-artifacts-vfx-emitter", 3),
            row("known-artifacts-vfx-system", 1),
        ]
    );
    // The rule teaches the one name the table spells.
    let speller = run.speller();
    let paths: Vec<String> = run
        .changes
        .of("known-artifacts-vfx-emitter")
        .iter()
        .map(|c| c.path.spelled(&speller))
        .collect();
    assert_eq!(paths, ["086379e1", "82a515a5", "isLocalSpace"]);
}

#[test]
fn a_field_the_target_build_declares_is_no_artifact() {
    // A target whose emitter declares `isLocalSpace`: the field is read there.
    let schema = SchemaRows::new(300)
        .class(EMITTER, &[field("isLocalSpace", kind("Bool"))])
        .build();
    let env = env_of(schema);
    let emitter = with_hashed(
        object("Emitter", EMITTER, vec![("isLocalSpace", flag(false))]),
        &[(FLAGS[0], flag(true))],
    );
    let run = FixRun::of(&env, &artifacts_only(), &skin_archive(vec![emitter]));
    assert_eq!(run.field("Emitter", "isLocalSpace"), Some(&flag(false)));
    assert!(!run.object("Emitter").properties.contains_key(&FLAGS[0]));
    assert_eq!(run.changes.len(), 1);
}

#[test]
fn one_class_is_turned_off_alone() {
    let env = env();
    let emitter = with_hashed(
        object("Emitter", EMITTER, vec![]),
        &[(FLAGS[0], flag(true))],
    );
    let mut objects = colors();
    objects.push(emitter);
    let selection = FixupSelection::standard().without("known-artifacts-value-color");
    assert!(selection.unknown().is_empty());
    let set = FixupSet::from(&selection);
    let run = FixRun::of(&env, &set, &skin_archive(objects));
    assert!(run.changes.of("known-artifacts-value-color").is_empty());
    assert_eq!(run.changes.of("known-artifacts-vfx-emitter").len(), 1);
    assert!(!run.object("Emitter").properties.contains_key(&FLAGS[0]));
}

#[test]
fn the_rules_applied_twice_change_nothing_the_second_time() {
    let env = env();
    let set = artifacts_only();
    let once = FixRun::of(&env, &set, &skin_archive(colors()));
    assert_eq!(once.changes.len(), 1);
    let after = [once.object("Color").clone(), once.object("Other").clone()];
    let twice = once.again(&set);
    assert!(twice.changes.is_empty(), "{:?}", twice.changes);
    assert_eq!(
        [twice.object("Color"), twice.object("Other")],
        [&after[0], &after[1]]
    );
}

#[test]
fn the_table_gives_one_rule_a_class_each_data() {
    let rules = artifacts::rules();
    assert_eq!(rules.len(), TABLE.len());
    for (rule, row) in rules.iter().zip(&TABLE) {
        let info = rule.info();
        assert_eq!(info.id, row.id);
        assert_eq!(info.phase, Phase::Undo);
        assert!(info.default);
        assert_eq!(info.one_record(), Some(Record::mod_artifact(20)));
        assert_eq!(rule.class().as_str(), row.class);
        assert_eq!(rule.sources().len(), row.fields.len());
        let written = serde_json::to_string(rule).expect("a rule writes");
        assert_eq!(&serde_json::from_str::<Rule>(&written).unwrap(), rule);
    }
    // The table as ltk-forge holds it: seven fields on four classes.
    let fields: usize = TABLE.iter().map(|row| row.fields.len()).sum();
    assert_eq!(fields, 7);
    // The one field with a name is the hash the mods hold.
    assert_eq!(name_hash("isLocalSpace"), BinHash(0x3040_923e));
    // Every rule of the table is in the standard set, before the layouts.
    let standard = FixupSet::standard();
    let ids = standard.ids();
    let first_layout = ids.iter().position(|id| *id == "vfx-flex-shape");
    for row in &TABLE {
        let at = ids.iter().position(|id| *id == row.id);
        assert!(at.is_some() && at < first_layout, "{} in {ids:?}", row.id);
    }
}
