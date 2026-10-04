//! The strip: a field the target's class does not declare is removed, as lost when a build
//! since the mod's own declared it and as never shown otherwise.

use ltk_meta::PropertyValueEnum as V;
use reforge::env::Env;
use reforge::fixups::conform::StripUndeclared;
use reforge::fixups::{After, FixupSet, Lost, Removed};
use reforge::game::Schema;
use reforge::model::name_hash;
use reforge_testkit::bins::{bin, field_of, float, object, pointer, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind, list_of, pointer as to};
use reforge_testkit::{FixRun, MemoryHistory, ModArchive, SchemaRows, skins};

const EMITTER: &str = "TestEmitter";
/// Game bytes the history dates, large enough to count.
const DATED: &str = "assets/characters/test/skins/base/dated.tex";

/// `legacyField` was declared until 120 and `renamedField` until 199. `junk` never was.
/// `onBase` is declared by the emitter's base.
fn schema() -> Schema {
    SchemaRows::new(300)
        .class("TestBase", &[field("onBase", kind("F32"))])
        .derived(
            EMITTER,
            &["TestBase"],
            &[
                field("particleName", kind("String")),
                field("legacyField", kind("F32")).until(120),
                field("renamedField", kind("F32")).until(199),
                field("child", to(EMITTER)),
                field("children", list_of(EMITTER)),
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

fn strip() -> FixupSet {
    FixupSet::empty().with(StripUndeclared)
}

fn fields() -> Vec<(&'static str, V)> {
    vec![
        ("particleName", text("p")),
        ("legacyField", float(1.0)),
        ("renamedField", float(2.0)),
        ("junk", float(3.0)),
        ("onBase", float(4.0)),
    ]
}

fn archive() -> ModArchive {
    skin_archive(vec![
        object("E", EMITTER, fields()),
        object("U", "NotInSchema", vec![("junk", float(3.0))]),
    ])
    .file(skins::WAD, DATED, dated_bytes())
}

/// What the strip did, as the field and whether it is lost.
fn stripped(run: &mut FixRun<'_>) -> Vec<(String, bool)> {
    for name in ["legacyField", "renamedField", "junk", "child", "children"] {
        run.names.learn_field(name);
    }
    let speller = run.speller();
    let changes = run.changes.of("strip-undeclared").iter();
    changes
        .map(|change| {
            assert!(matches!(change.after, After::Lost | After::Removed));
            (change.path.spelled(&speller), change.after == After::Lost)
        })
        .collect()
}

#[test]
fn fields_the_target_does_not_declare_are_stripped() {
    let env = env(true);
    let mut run = FixRun::of(&env, &strip(), &archive());
    assert_eq!(run.dating.build(), Some(150));
    let held: Vec<_> = run.object("E").properties.keys().copied().collect();
    // What the class declares stays, a field of its base included.
    assert_eq!(held, ["particleName", "onBase"].map(name_hash));
    // A class the schema does not have is left whole.
    assert_eq!(run.field("U", "junk"), Some(&float(3.0)));

    // Made at 150: `legacyField`, gone at 120, and `junk` never showed. `renamedField`,
    // declared until 199, did.
    assert_eq!(
        stripped(&mut run),
        [
            ("legacyField".to_string(), false),
            ("renamedField".to_string(), true),
            ("junk".to_string(), false),
        ]
    );
    let [removed] = run.raised::<Removed>("strip-undeclared")[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!(removed.0.values, 2);
    let [lost] = run.raised::<Lost>("strip-undeclared")[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!(lost.0.values, 1);
}

#[test]
fn every_field_stripped_from_a_mod_with_no_date_counts_as_lost() {
    let env = env(false);
    let mut run = FixRun::of(&env, &strip(), &archive());
    assert_eq!(run.dating.build(), None);
    assert_eq!(
        stripped(&mut run),
        [
            ("legacyField".to_string(), true),
            ("renamedField".to_string(), true),
            ("junk".to_string(), true),
        ]
    );
    assert!(run.raised::<Removed>("strip-undeclared").is_empty());
}

#[test]
fn a_field_under_a_pointer_or_a_list_is_stripped_and_a_stripped_struct_is_not_entered() {
    let env = env(true);
    let inner = |extra: &str| {
        pointer(
            EMITTER,
            vec![("particleName", text("p")), (extra, float(1.0))],
        )
    };
    let children = ltk_meta::property::values::Container::new(
        ltk_meta::PropertyKind::Struct,
        vec![inner("junk"), inner("onBase")],
    )
    .expect("a list of pointers");
    let emitter = object(
        "E",
        EMITTER,
        vec![
            ("child", inner("renamedField")),
            ("children", V::Container(children)),
            // The struct goes with its field: nothing in it is counted apart.
            ("junk", inner("legacyField")),
        ],
    );
    let archive = skin_archive(vec![emitter]).file(skins::WAD, DATED, dated_bytes());
    let mut run = FixRun::of(&env, &strip(), &archive);
    assert_eq!(
        stripped(&mut run),
        [
            ("junk".to_string(), false),
            ("child.renamedField".to_string(), true),
            ("children[0].junk".to_string(), false),
        ]
    );
    let child = run.field("E", "child").expect("the child");
    assert_eq!(field_of(child, "renamedField"), None);
    assert_eq!(field_of(child, "particleName"), Some(&text("p")));
}

#[test]
fn a_field_a_bin_of_the_install_holds_on_the_class_stays() {
    // The schema does not declare `seen`. The game's own bin holds it on the class at the
    // target build, so the build reads it.
    let mut game = skins::game_at("16.19.300");
    let held = object("G", EMITTER, vec![("seen", text("a"))]);
    game.insert(&skins::skin_bin("test", 5), bin(vec![held]));
    let env = Env::builder()
        .game(game)
        .schema(schema())
        .build()
        .expect("an environment");
    let mine = object(
        "E",
        EMITTER,
        vec![("seen", text("b")), ("junk", float(3.0))],
    );
    let run = FixRun::of(&env, &strip(), &skin_archive(vec![mine]));
    assert_eq!(run.field("E", "seen"), Some(&text("b")));
    assert_eq!(run.field("E", "junk"), None);
}

#[test]
fn nothing_is_stripped_without_a_schema() {
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .build()
        .expect("an environment");
    let run = FixRun::of(&env, &strip(), &archive());
    assert!(run.changes.is_empty());
    assert_eq!(run.object("E"), &object("E", EMITTER, fields()));
}

#[test]
fn the_strip_applied_twice_changes_nothing_the_second_time() {
    for dated in [false, true] {
        let env = env(dated);
        let set = strip();
        let once = FixRun::of(&env, &set, &archive());
        assert_eq!(once.changes.counts().edits(), 3);
        let after = once.object("E").clone();
        let twice = once.again(&set);
        assert!(twice.changes.is_empty(), "{:?}", twice.changes);
        assert_eq!(twice.object("E"), &after);
    }
}
