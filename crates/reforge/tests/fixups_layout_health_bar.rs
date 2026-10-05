//! The health bar style of a champion's skin entry (#GAME-CHANGE-0023): the champion's bar of
//! an older numbering, or no style, as the target numbers the champion's bar.

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyValueEnum as V};
use reforge::env::Env;
use reforge::fixups::layout::HealthBarStyle;
use reforge::fixups::{Changed, FixupSet, Outcome};
use reforge::game::Schema;
use reforge::model::{FieldPath, name_hash};
use reforge_testkit::bins::{bin, embedded, field_of, object, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{embed, field, kind};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const ID: &str = "health-bar-style";
const ENTRY_CLASS: &str = "SkinCharacterDataProperties";
const RECORD: &str = "CharacterHealthBarDataRecord";
/// The champion's skin entry of the base skin.
const ENTRY: &str = "Characters/Test/Skins/Skin0";
/// The sub-character's, in the champion's WAD.
const PET_ENTRY: &str = "Characters/TestPet/Skins/Skin0";

fn schema() -> Schema {
    SchemaRows::new(300)
        .class(ENTRY_CLASS, &[field("healthBarData", embed(RECORD))])
        .class(
            RECORD,
            &[
                field("unitHealthBarStyle", kind("U8")),
                field("attachToBone", kind("String")),
            ],
        )
        .build()
}

/// The target at a version, build 300.
fn env_at(version: &str) -> Env {
    Env::builder()
        .game(skins::game_at(&format!("{version}.300")))
        .schema(schema())
        .build()
        .expect("an environment")
}

fn style(value: u8) -> V {
    V::U8(values::U8::new(value))
}

/// A skin entry whose health bar record holds these fields.
fn entry(name: &str, record: Vec<(&str, V)>) -> BinObject {
    object(
        name,
        ENTRY_CLASS,
        vec![("healthBarData", embedded(RECORD, record))],
    )
}

/// The champion's base skin holding these objects, and the sub-character's holding its own.
fn archive(champion: Vec<BinObject>, pet: Vec<BinObject>) -> ModArchive {
    let archive = skin_archive(champion);
    if pet.is_empty() {
        return archive;
    }
    archive.file(skins::WAD, &skins::skin_bin("testpet", 0), bin(pet))
}

fn fixed<'a>(env: &'a Env, champion: Vec<BinObject>) -> FixRun<'a> {
    let set = FixupSet::empty().with(HealthBarStyle);
    FixRun::of(env, &set, &archive(champion, vec![]))
}

/// The style an entry holds.
fn held(run: &FixRun<'_>, name: &str) -> Option<u8> {
    match field_of(run.field(name, "healthBarData")?, "unitHealthBarStyle")? {
        V::U8(value) => Some(value.value),
        other => panic!("a style of another kind: {other:?}"),
    }
}

fn style_path() -> FieldPath {
    let holder = FieldPath::root().field(name_hash(ENTRY_CLASS), name_hash("healthBarData"));
    holder.field(name_hash(RECORD), name_hash("unitHealthBarStyle"))
}

#[test]
fn a_champions_bar_of_an_older_numbering_is_the_targets() {
    let env = env_at("16.19");
    for old in [0, 9, 10, 11] {
        let run = fixed(
            &env,
            vec![entry(ENTRY, vec![("unitHealthBarStyle", style(old))])],
        );
        assert_eq!(held(&run, ENTRY), Some(12), "from {old}");
        let [change] = run.changes.of(ID) else {
            panic!("{:?}", run.changes);
        };
        assert_eq!(change.path, style_path());
        assert_eq!(change.outcome(), Outcome::Changed);
        assert_eq!(
            change.path.spelled(&run.speller()),
            "healthBarData.unitHealthBarStyle"
        );
        let [changed] = run.raised::<Changed>(ID)[..] else {
            panic!("{:?}", run.messages());
        };
        assert_eq!((changed.0.values, changed.0.objects), (1, 1));
    }
}

#[test]
fn a_champions_record_with_no_style_takes_the_targets_bar() {
    let env = env_at("16.19");
    let run = fixed(
        &env,
        vec![entry(ENTRY, vec![("attachToBone", text("C_Buffbone"))])],
    );
    assert_eq!(held(&run, ENTRY), Some(12));
    let record = run.field(ENTRY, "healthBarData").expect("the record");
    assert_eq!(field_of(record, "attachToBone"), Some(&text("C_Buffbone")));
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.path, style_path());
    assert!(change.before.is_none());
}

#[test]
fn a_champions_entry_with_no_record_takes_one_holding_the_bar_alone() {
    let env = env_at("16.19");
    let run = fixed(&env, vec![object(ENTRY, ENTRY_CLASS, vec![])]);
    let made = embedded(RECORD, vec![("unitHealthBarStyle", style(12))]);
    assert_eq!(run.field(ENTRY, "healthBarData"), Some(&made));
    let [change] = run.changes.of(ID) else {
        panic!("{:?}", run.changes);
    };
    let holder = FieldPath::root().field(name_hash(ENTRY_CLASS), name_hash("healthBarData"));
    assert_eq!(change.path, holder);
    assert!(change.before.is_none());
}

#[test]
fn the_targets_bar_and_a_bar_no_numbering_gives_the_champion_are_left() {
    let env = env_at("16.19");
    for kept in [12, 1, 4, 15] {
        let objects = vec![entry(ENTRY, vec![("unitHealthBarStyle", style(kept))])];
        let run = fixed(&env, objects.clone());
        assert!(run.changes.is_empty(), "{kept}: {:?}", run.changes);
        assert_eq!(run.object(ENTRY), &objects[0]);
    }
}

#[test]
fn a_sub_characters_entry_and_an_object_that_is_no_entry_are_left() {
    let env = env_at("16.19");
    let champion = vec![
        entry(ENTRY, vec![("unitHealthBarStyle", style(9))]),
        entry("Other", vec![("unitHealthBarStyle", style(9))]),
    ];
    let pet = vec![entry(PET_ENTRY, vec![("unitHealthBarStyle", style(4))])];
    let set = FixupSet::empty().with(HealthBarStyle);
    let run = FixRun::of(&env, &set, &archive(champion, pet));
    assert_eq!(held(&run, ENTRY), Some(12));
    assert_eq!(held(&run, "Other"), Some(9));
    assert_eq!(held(&run, PET_ENTRY), Some(4));
    assert_eq!(run.changes.of(ID).len(), 1);
}

#[test]
fn the_bar_is_the_one_of_the_targets_numbering() {
    // At 14.10 the champion's bar is 10: an 11 is of a later numbering, and stays.
    let env = env_at("14.10");
    let run = fixed(
        &env,
        vec![entry(ENTRY, vec![("unitHealthBarStyle", style(9))])],
    );
    assert_eq!(held(&run, ENTRY), Some(10));
    let objects = vec![entry(ENTRY, vec![("unitHealthBarStyle", style(11))])];
    let run = fixed(&env, objects.clone());
    assert!(run.changes.is_empty(), "{:?}", run.changes);
}

#[test]
fn the_value_names_its_numbering_whatever_the_mods_date() {
    // Dated at 15.10, when the champion's bar was 12: the 9 is a pasted entry's.
    let dated = "assets/characters/test/skins/base/dated.tex";
    let bytes = vec![7; 5000];
    let history = MemoryHistory::new(&["1.0.50", "15.10.150", "16.19.300"]);
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .history(Box::new(history.with_content(dated, &bytes, 1..=2)))
        .build()
        .expect("an environment");
    let champion = vec![entry(ENTRY, vec![("unitHealthBarStyle", style(9))])];
    let set = FixupSet::empty().with(HealthBarStyle);
    let archive = archive(champion, vec![]).file(skins::WAD, dated, bytes.clone());
    let run = FixRun::of(&env, &set, &archive);
    assert_eq!(run.dating.build(), Some(150));
    assert_eq!(held(&run, ENTRY), Some(12));
}

#[test]
fn a_target_before_the_first_numbering_changes_nothing() {
    let env = env_at("13.3");
    let objects = vec![object(ENTRY, ENTRY_CLASS, vec![])];
    let run = fixed(&env, objects.clone());
    assert!(run.changes.is_empty(), "{:?}", run.changes);
    assert_eq!(run.object(ENTRY), &objects[0]);
}

#[test]
fn the_fixup_applied_twice_changes_nothing_the_second_time() {
    let env = env_at("16.19");
    let set = FixupSet::empty().with(HealthBarStyle);
    let once = FixRun::of(
        &env,
        &set,
        &archive(vec![object(ENTRY, ENTRY_CLASS, vec![])], vec![]),
    );
    assert_eq!(once.changes.of(ID).len(), 1);
    let after = once.object(ENTRY).clone();
    let twice = once.again(&set);
    assert!(twice.changes.is_empty(), "{:?}", twice.changes);
    assert_eq!(twice.object(ENTRY), &after);
}
