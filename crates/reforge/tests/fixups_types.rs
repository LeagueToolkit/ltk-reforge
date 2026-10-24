//! The target build's types: the meta schema first, read at the install's build, else the
//! shape the install's own bins hold a field in; and what the mod's own build read.

use ltk_meta::property::values;
use ltk_meta::{PropertyKind as Kind, PropertyValueEnum as V};
use reforge::diag::Stage;
use reforge::env::Env;
use reforge::fixups::{
    CatalogCapped, FixError, FixInputs, FixupSet, ObjectSet, Rule, TargetTypes, TypeSource,
};
use reforge::game::{Build, FieldType, GameError, MAX_CHAIN, Schema};
use reforge::load::{Dating, LoadOptions};
use reforge::model::{Slot, name_hash};
use reforge_testkit::bins::{bin, embedded, entry, float, linked_bin, object, pointer, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind, pointer as to};
use reforge_testkit::{BrokenGame, FixRun, LoadRun, MemoryHistory, SchemaRows, skins};

const DATED: &str = "assets/characters/test/skins/base/dated.tex";
const SKIN0: &str = "Characters/Test/Skins/Skin0";

/// `Thing.path` is a string up to build 199 and a file from 200. `Thing.gone` leaves at 149.
fn schema() -> Schema {
    SchemaRows::new(300)
        .class(
            "Thing",
            &[
                field("path", kind("String")).then(200, kind("File")),
                field("gone", kind("F32")).until(149),
                field("Child", to("Leaf")),
            ],
        )
        .class("Leaf", &[field("kept", kind("F32"))])
        .build()
}

fn hash(class: &str, field: &str) -> (reforge::model::BinHash, reforge::model::BinHash) {
    (name_hash(class), name_hash(field))
}

#[test]
fn a_type_is_the_schemas_at_the_installs_build() {
    let schema = schema();
    let at = |build: &str| TargetTypes::new(Some(&schema), Build::parse(build).as_ref());
    let (thing, path) = hash("Thing", "path");
    assert_eq!(
        at("16.19.300").kind_of(thing, path),
        Some(Kind::WadChunkLink)
    );
    assert_eq!(at("1.0.150").kind_of(thing, path), Some(Kind::String));
    // An install past the schema is read at the schema's newest build.
    assert_eq!(at("17.1.900").build(), Some(300));
    assert_eq!(
        at("17.1.900").kind_of(thing, path),
        Some(Kind::WadChunkLink)
    );
    // An install that does not say its build is read there too.
    assert_eq!(TargetTypes::new(Some(&schema), None).build(), Some(300));

    let types = at("16.19.300");
    let (_, gone) = hash("Thing", "gone");
    assert!(types.declares(thing, path) && !types.declares(thing, gone));
    assert_eq!(
        types.type_of(thing, path).map(|(_, source)| source),
        Some(TypeSource::Schema)
    );
    assert_eq!(types.has_class(thing), Some(true));
    assert_eq!(types.has_class(name_hash("Unknown")), Some(false));

    let none = TargetTypes::new(None, Build::parse("16.19.300").as_ref());
    assert_eq!((none.build(), none.has_class(thing)), (None, None));
    assert!(!none.declares(thing, path));
}

#[test]
fn a_field_the_schema_does_not_describe_has_the_shape_the_installs_bins_hold() {
    let schema = schema();
    let mut types = TargetTypes::new(Some(&schema), None);
    let (thing, path) = hash("Thing", "path");
    let (leaf, extra) = hash("Leaf", "extra");
    let (_, items) = hash("Leaf", "items");
    assert!(!types.declares(leaf, extra));

    let list = values::Container::new(
        Kind::Embedded,
        vec![embedded("Item", vec![("size", float(1.0))])],
    )
    .unwrap();
    types.witness(&object(
        "Seen",
        "Thing",
        vec![
            // The schema describes it: what a bin holds there does not replace it.
            ("path", text("assets/a.tex")),
            (
                "Child",
                pointer(
                    "Leaf",
                    vec![("extra", float(2.0)), ("items", V::Container(list))],
                ),
            ),
        ],
    ));
    assert_eq!(
        types
            .type_of(thing, path)
            .map(|(t, source)| (t.kind, source)),
        Some((Kind::WadChunkLink, TypeSource::Schema))
    );
    assert_eq!(
        types.type_of(leaf, extra),
        Some((
            FieldType {
                kind: Kind::F32,
                key: None,
                item: None,
                class: None,
                width: None,
            },
            TypeSource::Install
        ))
    );
    assert_eq!(
        types.type_of(leaf, items).map(|(t, _)| t),
        Some(FieldType {
            kind: Kind::Container,
            key: None,
            item: Some(Kind::Embedded),
            class: Some(name_hash("Item")),
            width: None,
        })
    );
    // A struct inside a list is witnessed as its own class.
    assert!(types.witnessed(name_hash("Item"), name_hash("size")));
    assert!(types.witnessed(thing, path) && !types.witnessed(leaf, name_hash("kept")));

    // The first shape seen stays.
    types.witness(&object("Later", "Leaf", vec![("extra", text("x"))]));
    assert_eq!(types.kind_of(leaf, extra), Some(Kind::F32));
}

#[test]
fn the_bins_of_the_mods_characters_skins_are_witnessed() {
    // The game's skin bin links a bin of the character, which holds a class no schema names.
    let shared = "data/characters/test/shared.bin";
    let mut game = skins::game_at("16.19.300");
    game.insert(
        &skins::skin_bin("test", 3),
        linked_bin(
            vec![object("Own", "GameOnly", vec![("inSkin", float(1.0))])],
            &[shared],
        ),
    );
    game.insert(
        shared,
        bin(vec![object(
            "Shared",
            "GameOnly",
            vec![("inLinked", float(1.0))],
        )]),
    );
    game.insert(
        "data/characters/other/skins/skin0.bin",
        bin(vec![object(
            "Other",
            "GameOnly",
            vec![("elsewhere", float(1.0))],
        )]),
    );
    let env = Env::builder().game(game).build().unwrap();
    let run = FixRun::of(&env, &FixupSet::empty(), &skin_archive(vec![]));
    let types = TargetTypes::of(&run.fixed, &env).unwrap();
    let class = name_hash("GameOnly");
    assert!(types.declares(class, name_hash("inSkin")));
    assert!(types.declares(class, name_hash("inLinked")));
    // A character the mod does not touch is not read.
    assert!(!types.declares(class, name_hash("elsewhere")));
    assert_eq!(
        types
            .type_of(class, name_hash("inSkin"))
            .map(|(_, source)| source),
        Some(TypeSource::Install)
    );
    assert!(types.witnessed_len() > 2);
}

#[test]
fn a_skin_whose_bins_link_past_the_cap_is_noted_by_the_fix() {
    // #ADR-0070: the bins past the cap are not witnessed, and the stage says so.
    let mut game = skins::game_at("16.19.300");
    let linked: Vec<String> = (0..MAX_CHAIN + 4)
        .map(|i| format!("data/characters/test/shared/part{i}.bin"))
        .collect();
    let links: Vec<&str> = linked.iter().map(String::as_str).collect();
    game.insert(
        &skins::skin_bin("test", 0),
        linked_bin(vec![entry(SKIN0, &skins::base_skin())], &links),
    );
    for (i, path) in linked.iter().enumerate() {
        let field = format!("part{i}");
        let held = object(
            &format!("Shared/Part{i}"),
            "GameOnly",
            vec![(&field, float(1.0))],
        );
        game.insert(path, bin(vec![held]));
    }
    let env = Env::builder().game(game).build().unwrap();
    let run = FixRun::of(&env, &FixupSet::empty(), &skin_archive(vec![]));
    let class = name_hash("GameOnly");
    let types = TargetTypes::of(&run.fixed, &env).unwrap();
    assert!(types.declares(class, name_hash("part0")));
    assert!(!types.declares(class, name_hash(&format!("part{MAX_CHAIN}"))));
    let capped: Vec<&CatalogCapped> = run.log.of::<CatalogCapped>().collect();
    assert_eq!(capped.len(), 1, "{:#?}", run.messages());
    assert_eq!(
        (&capped[0].slots[..], capped[0].cap),
        (&[Slot(0)][..], MAX_CHAIN)
    );
    assert_eq!(run.fixed.characters[capped[0].character].name, "test");
}

#[test]
fn a_bin_of_the_game_that_does_not_read_ends_the_fix_with_a_fault() {
    // Loaded against a game that reads, fixed against one whose bin of skin 7 does not.
    let reads = Env::builder().game(skins::game_at("16.19.300"));
    let reads = reads.build().unwrap();
    let run = LoadRun::of(&reads, LoadOptions::default(), &skin_archive(vec![]));
    let LoadRun {
        result,
        mut log,
        mut names,
        ..
    } = run;
    let m = result.expect("a mod");

    let broken = BrokenGame::at(skins::game_at("16.19.300"), &skins::skin_bin("test", 7));
    let env = Env::builder().game(broken).build().unwrap();
    // The bin is there and does not read: its shapes are not taken as unknown.
    let types = TargetTypes::of(&m, &env);
    assert!(matches!(types, Err(GameError::Open(_))), "{types:?}");

    let dating = Dating::default();
    let mut inputs = FixInputs {
        env: &env,
        dating: &dating,
        names: &mut names,
    };
    let set = FixupSet::empty();
    let applied = set.apply(m, &ObjectSet::every(), &mut inputs, &mut log.at(Stage::Fix));
    assert!(
        matches!(applied, Err(FixError::Game(GameError::Open(_)))),
        "{:?}",
        applied.err()
    );
    assert!(log.iter().all(|d| d.stage != Stage::Fix));
}

#[test]
fn a_rule_runs_on_what_the_installs_bins_witness_when_there_is_no_schema() {
    // No schema: the game's own emitter shows the layout the build reads.
    let mut game = skins::game_at("16.19.300");
    let filtering = pointer("Filters", vec![("excluded", text("x"))]);
    game.insert(
        &skins::skin_bin("test", 3),
        bin(vec![object(
            "Game",
            "Emitter",
            vec![("Filtering", filtering)],
        )]),
    );
    let env = Env::builder().game(game).build().unwrap();
    let rule = Rule::move_fields("Emitter")
        .into_field("Filtering", "Filters")
        .keep(["excluded", "required"]);
    let set = FixupSet::empty().with(rule);
    let old = object(
        "Old",
        "Emitter",
        vec![("excluded", text("a")), ("required", text("b"))],
    );
    let run = FixRun::of(&env, &set, &skin_archive(vec![old]));
    let child = run.child("Old", "Filtering");
    assert_eq!(
        child.properties.get(&name_hash("excluded")),
        Some(&text("a"))
    );
    // No bin of the game holds `required` in the child: it stays where it is.
    assert_eq!(run.field("Old", "required"), Some(&text("b")));
    assert_eq!(run.changes.len(), 1);
}

fn dated_bytes() -> Vec<u8> {
    vec![7; 5000]
}

/// A mod dated at a build, fixed by one rule that moves `gone` and `path` into `Child`.
fn dated_run(env: &Env) -> FixRun<'_> {
    let rule = Rule::move_fields("Thing")
        .into_field("Child", "Leaf")
        .rename("gone", "kept")
        .id("gather");
    let thing = object("T", "Thing", vec![("gone", float(1.0))]);
    let archive = skin_archive(vec![thing]).file(skins::WAD, DATED, dated_bytes());
    FixRun::of(env, &FixupSet::empty().with(rule), &archive)
}

fn env_dated(first: u16) -> Env {
    let history = MemoryHistory::new(&["1.0.100", "1.0.150", "1.0.250", "16.19.300"]);
    let history = history.with_content(DATED, &dated_bytes(), first..=3);
    Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .history(Box::new(history))
        .build()
        .unwrap()
}

#[test]
fn a_value_the_mods_own_build_read_moves_and_one_it_did_not_stays() {
    // Made at build 100 or later: `gone` was read up to 149, so the author saw it.
    let env = env_dated(0);
    let run = dated_run(&env);
    assert_eq!(run.dating.build(), Some(100));
    assert_eq!(run.changes.len(), 1);
    assert!(run.field("T", "gone").is_none());

    // Made at build 250 or later: no build since declared `gone`.
    let env = env_dated(2);
    let run = dated_run(&env);
    assert_eq!(run.dating.build(), Some(250));
    assert!(run.changes.is_empty());
    assert_eq!(run.field("T", "gone"), Some(&float(1.0)));

    // No history: nothing is known of the mod's build, and the value moves.
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .build()
        .unwrap();
    let run = dated_run(&env);
    assert_eq!(run.dating.build(), None);
    assert_eq!(run.changes.len(), 1);
}

#[test]
fn a_date_before_what_the_schema_describes_calls_nothing_unread() {
    // The schema starts at build 100; a mod made at 50 or later may have been read anyhow.
    let history = MemoryHistory::new(&["1.0.50", "16.19.300"]);
    let history = history.with_content(DATED, &dated_bytes(), 0..=1);
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .history(Box::new(history))
        .build()
        .unwrap();
    let run = dated_run(&env);
    assert_eq!(run.dating.build(), Some(50));
    assert_eq!(run.changes.len(), 1);
    let types = TargetTypes::new(env.schema(), Build::parse("16.19.300").as_ref());
    let (thing, gone) = hash("Thing", "gone");
    assert_eq!(types.types_since(thing, gone, 50), None);
    assert_eq!(types.types_since(thing, gone, 250), Some(Vec::new()));
    assert_eq!(
        types.types_since(thing, gone, 120).map(|t| t.len()),
        Some(1)
    );
}
