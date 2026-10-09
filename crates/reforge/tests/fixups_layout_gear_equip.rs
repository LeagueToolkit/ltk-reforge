//! The fixup of a contextual rule's gear condition: the condition of the class 15.18 retired
//! becomes the class Riot gave it, its bool renamed and its values kept, or goes where no build
//! since the mod's own read it (#GAME-CHANGE-0027).

use ltk_meta::path::MapKey;
use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyKind, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::layout::GearEquip;
use reforge::fixups::{After, FixupSet};
use reforge::game::Schema;
use reforge::model::{BinHash, FieldPath, name_hash};
use reforge_testkit::bins::{embedded, flag, object, pointer};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind};
use reforge_testkit::{FixRun, MemoryHistory, SchemaRows, skins};

const ID: &str = "contextual-gear-equip";
const ACTIONS: &str = "ContextualActionData";
const SITUATIONS: &str = "mSituations";
const SITUATION: &str = "ContextualSituation";
const RULES: &str = "mRules";
const RULE: &str = "ContextualRule";
const CONDITIONS: &str = "mConditions";
const OLD: &str = "ContextualConditionGearId";
const NEW: &str = "ContextualConditionGearIdEquip";
/// The old class's bool, which no name table spells.
const CHECK: BinHash = BinHash(0x58ec_fcaa);
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";
const TARGET: &str = "16.19.8207193";

/// The target's schema: the new class.
fn schema() -> Schema {
    SchemaRows::new(8_207_193)
        .class(
            NEW,
            &[
                field("CheckNewGearId", kind("Bool")),
                field("CheckPrevGearId", kind("Bool")),
                field("NewGearId", kind("U32")),
                field("PrevGearId", kind("U32")),
            ],
        )
        .build()
}

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

/// The target, with `schema`. With `first_shipped`, a history in which the mod's dated file
/// first shipped at that build.
fn env_with(schema: Schema, first_shipped: Option<&str>) -> Env {
    let mut env = Env::builder().game(skins::game_at(TARGET)).schema(schema);
    if let Some(build) = first_shipped {
        let history = MemoryHistory::new(&["15.17.7085788", build, TARGET]);
        env = env.history(Box::new(history.with_content(DATED, &dated_bytes(), 1..=2)));
    }
    env.build().expect("an environment")
}

fn env(first_shipped: Option<&str>) -> Env {
    env_with(schema(), first_shipped)
}

fn u32_of(value: u32) -> V {
    V::U32(values::U32::new(value))
}

/// A gear condition of the old class: a check of the new gear, of the previous gear, and the
/// two gear ids.
fn old_condition() -> V {
    let mut condition = pointer(
        OLD,
        vec![
            ("CheckPrevGearId", flag(false)),
            ("NewGearId", u32_of(2)),
            ("PrevGearId", u32_of(1)),
        ],
    );
    let V::Struct(fields) = &mut condition else {
        panic!("a pointer is a struct");
    };
    fields.properties.shift_insert(0, CHECK, flag(true));
    condition
}

fn list(item: PropertyKind, items: Vec<V>) -> V {
    let list = values::Container::new(item, items)
        .unwrap_or_else(|error| panic!("a list of {item:?} does not build: {error}"));
    V::Container(list)
}

/// A rule whose conditions are a skin check and the gear condition, with an old gear
/// condition in another field.
fn rule() -> V {
    let skin = pointer("ContextualConditionCharacterSkinID", vec![]);
    embedded(
        RULE,
        vec![
            (
                CONDITIONS,
                list(PropertyKind::Struct, vec![skin, old_condition()]),
            ),
            ("mFallbackCondition", old_condition()),
        ],
    )
}

/// The champion's contextual actions: one situation of one rule.
fn actions() -> BinObject {
    let situation = embedded(
        SITUATION,
        vec![(RULES, list(PropertyKind::Embedded, vec![rule()]))],
    );
    let key = V::Hash(values::Hash::new(name_hash("OnEquip")));
    let map = values::Map::new(
        PropertyKind::Hash,
        PropertyKind::Embedded,
        vec![(key, situation)],
    )
    .unwrap_or_else(|error| panic!("a map of situations does not build: {error}"));
    object("Voice", ACTIONS, vec![(SITUATIONS, V::Map(map))])
}

fn set() -> FixupSet {
    FixupSet::empty().with(GearEquip)
}

fn fixed(env: &Env) -> FixRun<'_> {
    let archive = skin_archive(vec![actions()]).file(skins::WAD, DATED, dated_bytes());
    FixRun::of(env, &set(), &archive)
}

/// The rule's fields, after the run.
fn the_rule<'r>(run: &'r FixRun<'_>) -> &'r values::Struct {
    let Some(V::Map(map)) = run.field("Voice", SITUATIONS) else {
        panic!("the actions hold no map of situations");
    };
    let Some((_, V::Embedded(situation))) = map.entries().first() else {
        panic!("no situation");
    };
    let Some(V::Container(rules)) = situation.0.properties.get(&name_hash(RULES)) else {
        panic!("the situation holds no rules");
    };
    let Some(V::Embedded(rule)) = rules.items().first() else {
        panic!("no rule");
    };
    &rule.0
}

/// The rule's conditions, after the run.
fn conditions<'r>(run: &'r FixRun<'_>) -> Vec<&'r values::Struct> {
    let Some(V::Container(list)) = the_rule(run).properties.get(&name_hash(CONDITIONS)) else {
        panic!("the rule holds no conditions");
    };
    let items = list.items().iter().map(|item| match item {
        V::Struct(condition) => condition,
        other => panic!("a condition of another shape: {other:?}"),
    });
    items.collect()
}

fn at_condition(index: usize) -> FieldPath {
    let key = MapKey::try_from(&V::Hash(values::Hash::new(name_hash("OnEquip")))).expect("a key");
    let situations = FieldPath::root().field(name_hash(ACTIONS), name_hash(SITUATIONS));
    let rules = situations
        .key(key)
        .field(name_hash(SITUATION), name_hash(RULES));
    let conditions = rules.item(0).field(name_hash(RULE), name_hash(CONDITIONS));
    conditions.item(index)
}

#[test]
fn a_gear_condition_takes_the_new_class_with_its_bool_renamed_and_its_values_kept() {
    let env = env(None);
    let run = fixed(&env);
    let [skin, gear] = conditions(&run)[..] else {
        panic!("{:?}", conditions(&run));
    };
    assert_eq!(
        skin.class_hash,
        name_hash("ContextualConditionCharacterSkinID")
    );
    assert_eq!(gear.class_hash, name_hash(NEW));
    let fields: Vec<(BinHash, &V)> = gear.properties.iter().map(|(f, v)| (*f, v)).collect();
    assert_eq!(
        fields,
        [
            (name_hash("CheckNewGearId"), &flag(true)),
            (name_hash("CheckPrevGearId"), &flag(false)),
            (name_hash("NewGearId"), &u32_of(2)),
            (name_hash("PrevGearId"), &u32_of(1)),
        ]
    );
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.path, at_condition(1));
    assert!(matches!(change.after, After::Value(_)));
}

#[test]
fn a_gear_condition_outside_the_rules_conditions_is_left() {
    let env = env(None);
    let run = fixed(&env);
    let Some(V::Struct(fallback)) = the_rule(&run)
        .properties
        .get(&name_hash("mFallbackCondition"))
    else {
        panic!("the rule holds no fallback");
    };
    assert_eq!(fallback.class_hash, name_hash(OLD));
    assert!(fallback.properties.contains_key(&CHECK));
}

#[test]
fn a_gear_condition_no_build_since_the_mods_own_read_is_removed() {
    // #ADR-0065: dated at 15.18, the first build that has no class of that name.
    let env = env(Some("15.18.7095052"));
    let run = fixed(&env);
    assert_eq!(run.dating.build(), Some(7_095_052));
    let classes: Vec<BinHash> = conditions(&run).iter().map(|c| c.class_hash).collect();
    assert_eq!(classes, [name_hash("ContextualConditionCharacterSkinID")]);
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.path, at_condition(1));
    assert_eq!(change.after, After::Removed);
}

#[test]
fn a_target_that_lacks_the_new_class_skips_the_fixup() {
    let schema = SchemaRows::new(8_207_193)
        .class(OLD, &[field("NewGearId", kind("U32"))])
        .build();
    let env = env_with(schema, None);
    let run = fixed(&env);
    assert!(run.changes.of(ID).is_empty(), "{:?}", run.changes);
    assert_eq!(conditions(&run)[1].class_hash, name_hash(OLD));
}

#[test]
fn the_fixup_applied_twice_changes_nothing_the_second_time() {
    let env = env(None);
    let once = fixed(&env);
    let after = once.object("Voice").clone();
    let twice = once.again(&set());
    assert!(twice.changes.is_empty(), "{:?}", twice.changes);
    assert_eq!(twice.object("Voice"), &after);
}
