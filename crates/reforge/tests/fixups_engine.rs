//! The fixup engine: the set and its order, the choice of fixups as data, rules as data, the
//! record of every change, and the contract every fixup is held to.

use ltk_meta::{BinObject, PropertyKind as Kind, PropertyValueEnum as V};
use reforge::diag::{Cancel, Cancelled, Record, Severity, Stage};
use reforge::env::Env;
use reforge::fixups::{
    After, BinFixup, Changed, Classes, FixCx, FixError, FixInputs, FixupSelection, FixupSet,
    ObjectSet, Outcome, Removed, Rule, SelectionError, TargetTypes, Unconvertible, rules,
};
use reforge::game::Schema;
use reforge::history::HistoryError;
use reforge::load::{Dating, LoadOptions};
use reforge::model::entity::UnitId;
use reforge::model::{Dump, DumpCx, FixupId, FixupInfo, Phase, StructMut, Version, name_hash};
use reforge::report::Report;
use reforge_testkit::bins::{
    Skin, entry, flag, float, linked_bin, object, pointer, text, with_link,
};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{embed, field, kind, pointer as to};
use reforge_testkit::{FixRun, LoadRun, ModArchive, SchemaRows, skins};
use serde_json::json;

const EMITTER: &str = "VfxEmitterDefinitionData";

/// A target that reads an emitter's filters in `Filtering`, its second texture in
/// `textureMult`, a material's sampler under `textureName`, and nothing called `junk`.
fn schema() -> Schema {
    SchemaRows::new(300)
        .class(
            EMITTER,
            &[
                field("Filtering", to("VfxEmitterFiltering")).from(121),
                field("keywordsExcluded", kind("String")).until(120),
                field("textureMult", to("VfxTextureMultDefinitionData")).from(121),
                field("birthScale", embed("ValueVector3")),
            ],
        )
        .class(
            "VfxEmitterFiltering",
            &[field("keywordsExcluded", kind("String")).from(121)],
        )
        .class(
            "VfxTextureMultDefinitionData",
            &[field("textureMult", kind("String")).from(121)],
        )
        .class(
            "Material",
            &[
                field("samplerName", kind("String")).until(120),
                field("textureName", kind("String")).from(121),
                field("count", kind("U8")),
            ],
        )
        .build()
}

fn env() -> Env {
    Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema())
        .build()
        .expect("an environment")
}

fn emitter(entry: &str, fields: Vec<(&str, V)>) -> BinObject {
    object(entry, EMITTER, fields)
}

/// Two old emitters and a material, in one skin bin.
fn old_mod() -> ModArchive {
    skin_archive(vec![
        emitter(
            "A",
            vec![
                ("keywordsExcluded", text("spectator")),
                ("textureMult", text("assets/mod/a/mult.tex")),
            ],
        ),
        emitter("B", vec![("keywordsExcluded", text("replay"))]),
        object(
            "M",
            "Material",
            vec![
                ("samplerName", text("Diffuse")),
                ("junk", float(1.0)),
                ("count", float(2.0)),
            ],
        ),
    ])
}

/// A selection with every built-in fixup on the bins turned off but the table rules. A test
/// of the engine runs the rules, so a fixup the standard set gains does not change what it
/// sees.
fn only_rules(selection: FixupSelection) -> FixupSelection {
    let rules: Vec<FixupId> = rules::all()
        .iter()
        .map(|rule| rule.info().id.clone())
        .collect();
    let standard = FixupSet::standard();
    let built_in = standard.bin_fixups().iter();
    built_in
        .map(|fixup| fixup.info().id.clone())
        .filter(|id| !rules.contains(id))
        .fold(selection, FixupSelection::without)
}

/// The table rules, as a selection gives them.
fn rules_set() -> FixupSet {
    FixupSet::from(&only_rules(FixupSelection::standard()))
}

#[test]
fn the_standard_set_runs_by_phase_then_by_build_then_by_id() {
    // The file fixups and the table rules, in the order they keep among every other fixup.
    let pinned = [
        "legacy-formats",
        "moved-assets",
        "vfx-flex-shape",
        "vfx-filtering",
        "vfx-linger",
        "vfx-legacy-simple",
        "vfx-texture-mult",
    ];
    let set = FixupSet::standard();
    let found: Vec<&str> = set
        .ids()
        .into_iter()
        .filter(|id| pinned.contains(id))
        .collect();
    assert_eq!(found, pinned);
}

#[test]
fn the_catalog_lists_the_fixups_on_files_and_on_bins() {
    let catalog = FixupSet::catalog();
    let ids: Vec<&str> = catalog.iter().map(|info| info.id.as_str()).collect();
    assert_eq!(ids, FixupSet::standard().ids());
    let info = |id: &str| catalog.iter().find(|info| info.id == id).expect("listed");
    assert_eq!(info("legacy-formats").phase, Phase::Files);
    assert_eq!(
        info("legacy-formats").records.as_ref(),
        [Record::game_change(7), Record::game_change(8)]
    );
    let filtering = info("vfx-filtering");
    assert_eq!(
        filtering.phase,
        Phase::Layout {
            build: Version::new(12, 17)
        }
    );
    assert!(filtering.default);
    assert_eq!(filtering.one_record(), Some(Record::game_change(2)));
    // A description is data: it writes and reads back.
    let json = serde_json::to_value(filtering).unwrap();
    assert_eq!(
        json,
        json!({
            "id": "vfx-filtering",
            "title": "An emitter's filters, in Filtering",
            "records": [{ "series": "game-change", "number": 2 }],
            "phase": { "name": "layout", "build": "12.17" },
            "default": true,
        })
    );
    let back: FixupInfo = serde_json::from_value(json).unwrap();
    assert_eq!(&back, filtering);
}

#[test]
fn a_rule_from_json_equals_the_rule_from_its_builder() {
    let json = json!({
        "id": "vfx-filtering",
        "title": "An emitter's filters, in Filtering",
        "records": [{ "series": "game-change", "number": 2 }],
        "phase": { "name": "layout", "build": "12.17" },
        "class": "VfxEmitterDefinitionData",
        "move": {
            "into": { "field": "Filtering", "class": "VfxEmitterFiltering" },
            "fields": ["keywordsExcluded", "keywordsRequired", "spectatorPolicy", "censorPolicy"],
        },
    });
    let read: Rule = serde_json::from_value(json).unwrap();
    assert_eq!(read, rules::filtering());

    // Every built-in rule writes and reads back as itself.
    for rule in rules::all() {
        let written = serde_json::to_string(&rule).unwrap();
        assert_eq!(serde_json::from_str::<Rule>(&written).unwrap(), rule);
    }

    // A field that takes a new name is written with both.
    let linger = serde_json::to_value(rules::linger()).unwrap();
    assert_eq!(
        linger["move"]["fields"][0],
        json!({ "from": "lingerDrag", "to": "KeyedLingerDrag" })
    );
    assert_eq!(linger["move"]["fields"][9], json!("UseLingerScale"));
    assert_eq!(linger["default"], json!(true));
}

#[test]
fn a_rule_states_only_what_differs_from_the_usual() {
    // No title, no record, no phase: a rule a person writes by hand takes a tool's damage out.
    let read: Rule = serde_json::from_value(json!({
        "id": "no-junk",
        "class": "Material",
        "remove": { "fields": ["junk", "0x1e6ba0c4"] },
    }))
    .unwrap();
    let built = Rule::remove_fields("Material", ["junk", "0x1e6ba0c4"]).id("no-junk");
    assert_eq!(read, built);
    assert_eq!(read.info().phase, Phase::Undo);
    assert_eq!(read.info().title, "Material: fields removed");
    assert!(read.info().default && read.info().records.is_empty());

    let renamed = Rule::rename_fields("Material").rename("samplerName", "textureName");
    assert_eq!(renamed.info().id, "rename-material");
    let moved = Rule::move_fields("A").into_field("Child", "B");
    assert_eq!(moved.info().id, "move-a-child");
    assert!(serde_json::from_value::<Rule>(json!({ "id": "x", "class": "A" })).is_err());
}

#[test]
fn a_selection_round_trips() {
    let selection = FixupSelection::standard()
        .without("vfx-linger")
        .without("legacy-formats")
        .with("an-optional-fixup")
        .rule(Rule::remove_fields("Material", ["junk"]).id("no-junk"))
        .rule(
            Rule::rename_fields("Material")
                .rename("samplerName", "textureName")
                .at_build(Version::new(16, 13))
                .record(Record::mod_artifact(3))
                .off_by_default(),
        );
    let json = selection.to_json();
    assert_eq!(FixupSelection::from_json(&json).unwrap(), selection);

    let dir = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(dir.path().join("fixups.json")).unwrap();
    selection.save(&path).unwrap();
    assert_eq!(FixupSelection::open(&path).unwrap(), selection);

    // The standard selection is the empty one, and an unknown key is refused.
    assert_eq!(
        FixupSelection::from_json("{}").unwrap(),
        FixupSelection::standard()
    );
    assert!(FixupSelection::from_json(r#"{"of": []}"#).is_err());
    assert_eq!(selection.unknown(), [&FixupId::from("an-optional-fixup")]);
}

#[test]
fn a_selection_that_does_not_load_says_whether_the_file_or_its_text_failed() {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();

    let missing = FixupSelection::open(&root.join("no-such.json"));
    assert!(matches!(missing, Err(SelectionError::Io(_))), "{missing:?}");

    let path = root.join("fixups.json");
    fs_err::write(&path, r#"{"off": "vfx-linger"}"#).unwrap();
    let wrong = FixupSelection::open(&path);
    assert!(matches!(wrong, Err(SelectionError::Json(_))), "{wrong:?}");
    let not_json = FixupSelection::from_json("off: vfx-linger");
    assert!(matches!(not_json, Err(SelectionError::Json(_))));

    // A selection saved where no directory is does not write.
    let nowhere = root.join("no-such-directory").join("fixups.json");
    let saved = FixupSelection::standard().save(&nowhere);
    assert!(matches!(saved, Err(SelectionError::Io(_))), "{saved:?}");
}

#[test]
fn a_selection_gives_the_set() {
    let selection = FixupSelection::standard()
        .without("vfx-linger")
        .without("legacy-formats")
        .rule(Rule::remove_fields("Material", ["junk"]).id("no-junk"))
        .rule(
            Rule::rename_fields("Material")
                .rename("samplerName", "textureName")
                .off_by_default(),
        );
    let set = FixupSet::from(&selection);
    // Among the file fixups, the table rules and the rule added, whatever else the set holds.
    let pinned = [
        "moved-assets",
        // A rule with no build takes a tool's damage out, before the layouts.
        "no-junk",
        "vfx-flex-shape",
        "vfx-filtering",
        "vfx-legacy-simple",
        "vfx-texture-mult",
    ];
    let found: Vec<&str> = set
        .ids()
        .into_iter()
        .filter(|id| pinned.contains(id))
        .collect();
    assert_eq!(found, pinned);
    assert!(!set.ids().contains(&"vfx-linger") && !set.ids().contains(&"legacy-formats"));
    let on = FixupSet::from(&selection.with("rename-material"));
    assert!(on.ids().contains(&"rename-material"));
    let files: Vec<String> = set
        .file_fixups()
        .iter()
        .map(|f| f.id().to_string())
        .collect();
    assert_eq!(files, ["moved-assets", "bank-id"]);
}

#[test]
fn a_fixup_turned_off_changes_nothing_and_is_absent_from_the_changes() {
    let env = env();
    let all = FixRun::of(&env, &rules_set(), &old_mod());
    assert_eq!(all.changes.of("vfx-filtering").len(), 2);
    assert_eq!(all.changes.of("vfx-texture-mult").len(), 1);

    let selection = only_rules(FixupSelection::standard());
    let set = FixupSet::from(&selection.without("vfx-filtering"));
    let run = FixRun::of(&env, &set, &old_mod());
    assert!(run.changes.of("vfx-filtering").is_empty());
    assert!(run.changes.iter().all(|c| c.fixup != "vfx-filtering"));
    assert_eq!(run.field("A", "keywordsExcluded"), Some(&text("spectator")));
    assert!(run.field("A", "Filtering").is_none());
    assert!(
        run.log
            .iter()
            .all(|d| d.by.as_deref() != Some("vfx-filtering"))
    );
    // The others still run.
    assert_eq!(run.changes.of("vfx-texture-mult").len(), 1);
}

#[test]
fn the_rules_applied_twice_change_nothing_the_second_time() {
    let env = env();
    let set = rules_set();
    let once = FixRun::of(&env, &set, &old_mod());
    assert_eq!(once.changes.counts().edits(), 3);
    let after: Vec<BinObject> = ["A", "B", "M"].map(|e| once.object(e).clone()).to_vec();
    let twice = once.again(&set);
    assert!(twice.changes.is_empty(), "{:?}", twice.changes);
    let again: Vec<BinObject> = ["A", "B", "M"].map(|e| twice.object(e).clone()).to_vec();
    assert_eq!(after, again);
}

#[test]
fn the_standard_set_applied_twice_changes_nothing_the_second_time() {
    let env = env();
    let set = FixupSet::standard();
    let once = FixRun::of(&env, &set, &old_mod());
    // The three values the rules move, at the least.
    assert!(once.changes.counts().edits() >= 3);
    let left = once.changes.counts().unconvertible;
    let after: Vec<BinObject> = ["A", "B", "M"].map(|e| once.object(e).clone()).to_vec();
    let twice = once.again(&set);
    // What has no conversion is said again, and nothing else changes.
    assert_eq!(twice.changes.counts().edits(), 0, "{:?}", twice.changes);
    assert_eq!(twice.changes.counts().unconvertible, left);
    let again: Vec<BinObject> = ["A", "B", "M"].map(|e| twice.object(e).clone()).to_vec();
    assert_eq!(after, again);
}

#[test]
fn a_rule_a_person_adds_runs() {
    let env = env();
    let selection = FixupSelection::from_json(
        r#"{
          "rules": [
            { "id": "no-junk", "class": "Material", "remove": { "fields": ["junk"] } },
            { "id": "sampler-name", "class": "Material",
              "phase": { "name": "layout", "build": "16.13" },
              "rename": { "fields": [{ "from": "samplerName", "to": "textureName" }] } }
          ]
        }"#,
    )
    .unwrap();
    let set = FixupSet::from(&only_rules(selection));
    let run = FixRun::of(&env, &set, &old_mod());
    assert_eq!(run.field("M", "junk"), None);
    assert_eq!(run.field("M", "samplerName"), None);
    assert_eq!(run.field("M", "textureName"), Some(&text("Diffuse")));

    let [removed] = run.changes.of("no-junk") else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(removed.after, After::Removed);
    assert_eq!(removed.outcome(), Outcome::Removed);
    let [renamed] = run.changes.of("sampler-name") else {
        panic!("{:?}", run.changes);
    };
    let material = name_hash("Material");
    let to = reforge::model::FieldPath::root().field(material, name_hash("textureName"));
    assert_eq!(renamed.after, After::Moved { to });
    // The fixups share their codes: a rule a person adds raises under its own id.
    assert_eq!(run.raised::<Removed>("no-junk").len(), 1);
    assert_eq!(run.raised::<Changed>("sampler-name").len(), 1);

    // A rule is held to the contract: a second run changes nothing.
    assert!(run.again(&set).changes.is_empty());
}

#[test]
fn a_rule_leaves_what_the_target_reads_where_it_is() {
    let env = env();
    // The target declares `count` on the material and `birthScale` on the emitter.
    let set = FixupSet::empty()
        .with(Rule::remove_fields("Material", ["count"]).id("remove-read"))
        .with(
            Rule::rename_fields(EMITTER)
                .rename("birthScale", "keywordsExcluded")
                .id("rename-to-old"),
        );
    let archive = skin_archive(vec![
        object("M", "Material", vec![("count", float(2.0))]),
        emitter("E", vec![("birthScale", float(1.0))]),
    ]);
    let run = FixRun::of(&env, &set, &archive);
    assert!(run.changes.is_empty());
    assert_eq!(run.field("M", "count"), Some(&float(2.0)));
    assert_eq!(run.field("E", "birthScale"), Some(&float(1.0)));
}

#[test]
fn no_rule_changes_a_struct_twice() {
    let schema = SchemaRows::new(300)
        .class(
            "Node",
            &[
                field("a", kind("F32")).until(120),
                field("b", kind("F32")),
                field("c", kind("F32")),
                field("Child", to("Node")),
                field("Other", to("Leaf")),
            ],
        )
        .class("Leaf", &[field("a", kind("F32")), field("b", kind("F32"))])
        .build();
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema)
        .build()
        .unwrap();
    let set = FixupSet::empty()
        // A swap and a chain: each row applies only from a name the target does not read.
        .with(
            Rule::rename_fields("Node")
                .rename("a", "b")
                .rename("b", "a")
                .rename("b", "c")
                .id("swap"),
        )
        // A struct moved into a struct of its own class would be moved again inside it.
        .with(
            Rule::move_fields("Node")
                .into_field("Child", "Node")
                .keep(["b"])
                .id("nest"),
        )
        // The same field in two rows: the first takes it.
        .with(
            Rule::move_fields("Node")
                .into_field("Other", "Leaf")
                .rename("c", "b")
                .rename("c", "a")
                .id("twice"),
        );
    let node = || vec![("a", float(1.0)), ("c", float(3.0))];
    let archive = skin_archive(vec![object("N", "Node", node())]);
    let once = FixRun::of(&env, &set, &archive);
    assert!(!once.changes.is_empty());
    let after = once.object("N").clone();
    let twice = once.again(&set);
    assert!(twice.changes.is_empty(), "{:?}", twice.changes);
    assert_eq!(twice.object("N"), &after);
    // `a` went to `b`, and `nest` made no child of the struct's own class.
    assert_eq!(twice.field("N", "b"), Some(&float(1.0)));
    assert!(twice.field("N", "Child").is_none());
    let other = twice.child("N", "Other");
    assert_eq!(other.properties.get(&name_hash("b")), Some(&float(3.0)));
}

#[test]
fn a_change_holds_the_object_the_path_and_the_value_before_and_after() {
    let env = env();
    let system = object(
        "S",
        "VfxSystemDefinitionData",
        vec![(
            "emitters",
            V::Container(
                ltk_meta::property::values::Container::new(
                    Kind::Struct,
                    vec![
                        pointer(EMITTER, vec![("emitterName", text("first"))]),
                        pointer(EMITTER, vec![("keywordsExcluded", text("spectator"))]),
                    ],
                )
                .unwrap(),
            ),
        )],
    );
    let mut run = FixRun::of(&env, &rules_set(), &skin_archive(vec![system]));
    run.names.learn_field("emitters");
    let [change] = run.changes.of("vfx-filtering") else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(change.fixup, "vfx-filtering");
    assert_eq!(change.object, run.id("S"));
    let speller = run.speller();
    assert_eq!(
        change.path.spelled(&speller),
        "emitters[1].keywordsExcluded"
    );
    let before = change.before.as_ref().expect("a value before");
    assert_eq!(
        (before.kind, before.text.as_str()),
        ("String", "\"spectator\"")
    );
    let After::Moved { to } = &change.after else {
        panic!("{change:?}");
    };
    assert_eq!(
        to.spelled(&speller),
        "emitters[1].Filtering.keywordsExcluded"
    );
    assert_eq!(change.outcome(), Outcome::Changed);

    // A change is data: hashes in the hash form, the object by its id.
    let emitters = format!("{:08x}", name_hash("emitters").0);
    let filtering = format!("{:08x}", name_hash("Filtering").0);
    let excluded = format!("{:08x}", name_hash("keywordsExcluded").0);
    assert_eq!(
        serde_json::to_value(change).unwrap(),
        json!({
            "fixup": "vfx-filtering",
            "object": run.id("S").to_string(),
            "path": format!("{emitters}[1].{excluded}"),
            "before": { "kind": "String", "text": "\"spectator\"" },
            "after": { "as": "moved", "to": format!("{emitters}[1].{filtering}.{excluded}") },
        })
    );
    assert!(run.changes.edits(run.id("S")));
    assert_eq!(run.changes.edited().len(), 1);
}

#[test]
fn changes_come_fixup_by_fixup_then_object_by_object_then_in_walk_order() {
    let env = env();
    let run = FixRun::of(&env, &rules_set(), &old_mod());
    let speller = run.speller();
    let rows: Vec<(String, String, String)> = run
        .changes
        .iter()
        .map(|c| {
            (
                c.fixup.to_string(),
                c.object.to_string(),
                c.path.spelled(&speller),
            )
        })
        .collect();
    let (a, b) = (run.id("A").to_string(), run.id("B").to_string());
    let row = |fixup: &str, object: &str, path: &str| (fixup.into(), object.into(), path.into());
    assert_eq!(
        rows,
        [
            row("vfx-filtering", &a, "keywordsExcluded"),
            row("vfx-filtering", &b, "keywordsExcluded"),
            row("vfx-texture-mult", &a, "textureMult"),
        ]
    );
    let dump = run.changes.dump(&DumpCx::new(&speller));
    let expected = format!(
        "fixup vfx-filtering changed=2 removed=0 lost=0 unconvertible=0\n     \
         {a} keywordsExcluded: \"spectator\" -> at Filtering.keywordsExcluded\n     \
         {b} keywordsExcluded: \"replay\" -> at Filtering.keywordsExcluded\n\
         fixup vfx-texture-mult changed=1 removed=0 lost=0 unconvertible=0\n     \
         {a} textureMult: \"assets/mod/a/mult.tex\" -> at textureMult.textureMult\n"
    );
    assert_eq!(dump, expected);
}

#[test]
fn what_a_fixup_did_is_raised_once_with_its_id_and_its_record() {
    let env = env();
    let run = FixRun::of(&env, &rules_set(), &old_mod());
    let of_fix: Vec<_> = run.log.iter().filter(|d| d.stage == Stage::Fix).collect();
    let rows: Vec<(&str, Option<&str>, Option<Record>)> = of_fix
        .iter()
        .map(|d| (d.code.name, d.by.as_deref(), d.record))
        .collect();
    assert_eq!(
        rows,
        [
            (
                "fixup.changed",
                Some("vfx-filtering"),
                Some(Record::game_change(2))
            ),
            (
                "fixup.changed",
                Some("vfx-texture-mult"),
                Some(Record::game_change(5))
            ),
        ]
    );
    assert!(of_fix.iter().all(|d| d.severity() == Severity::Note));
    let [changed] = run.raised::<Changed>("vfx-filtering")[..] else {
        panic!("{:?}", run.messages());
    };
    assert_eq!((changed.0.values, changed.0.objects), (2, 2));
    assert_eq!(
        run.messages()[0],
        "fixup.changed vfx-filtering: 2 values in 2 objects kept, in the form the target build \
         reads: VfxEmitterDefinitionData.keywordsExcluded (2)"
    );
    let data = of_fix[0].data();
    assert_eq!(
        data,
        json!({
            "values": 2,
            "objects": 2,
            "fields": [{
                "class": format!("{:08x}", name_hash(EMITTER).0),
                "field": format!("{:08x}", name_hash("keywordsExcluded").0),
                "values": 2,
            }],
        })
    );
}

/// A fixup written in code: it narrows a material's `count` to the kind the target reads,
/// and records the ones it cannot.
struct NarrowCount {
    info: FixupInfo,
}

impl NarrowCount {
    fn new() -> Self {
        Self {
            info: FixupInfo::built_in(
                "narrow-count",
                "A count as the build reads it",
                &[],
                Phase::Conform,
            ),
        }
    }
}

impl BinFixup for NarrowCount {
    fn info(&self) -> &FixupInfo {
        &self.info
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        types.kind_of(name_hash("Material"), name_hash("count")) == Some(Kind::U8)
    }

    fn classes(&self) -> Classes {
        Classes::Every
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let count = name_hash("count");
        let Some(wants) = cx.type_of(s.class(), count).map(|t| t.kind) else {
            return;
        };
        let path = s.path_of(count);
        match s.fields_mut().get_mut(&count) {
            Some(value @ V::U32(_)) => {
                let before = value.clone();
                *value = ltk_meta::property::values::U8::new(7).into();
                cx.converted(path, &before, value);
            }
            Some(value) if value.kind() != wants => cx.unconvertible(path, value, wants),
            _ => {}
        }
    }
}

#[test]
fn a_fixup_written_in_code_takes_its_place_by_its_phase() {
    let env = env();
    let set = rules_set().with(NarrowCount::new());
    assert_eq!(set.ids().last(), Some(&"narrow-count"));
    let archive = skin_archive(vec![
        object("M", "Material", vec![("count", float(2.0))]),
        object(
            "N",
            "Material",
            vec![("count", ltk_meta::property::values::U32::new(7).into())],
        ),
    ]);
    let run = FixRun::of(&env, &set, &archive);
    let [left, converted] = run.changes.of("narrow-count") else {
        panic!("{:?}", run.changes);
    };
    assert_eq!(left.after, After::Unconvertible { wants: "U8" });
    assert_eq!(left.outcome(), Outcome::Unconvertible);
    assert!(!left.edits() && !run.changes.edits(run.id("M")));
    assert_eq!(run.field("M", "count"), Some(&float(2.0)));
    let After::Value(after) = &converted.after else {
        panic!("{converted:?}");
    };
    assert_eq!((after.kind, after.text.as_str()), ("U8", "7"));
    assert_eq!(run.raised::<Unconvertible>("narrow-count").len(), 1);
    assert_eq!(run.raised::<Changed>("narrow-count").len(), 1);
    let warning = run
        .log
        .iter()
        .find(|d| d.code.name == "fixup.unconvertible");
    assert_eq!(warning.map(|d| d.severity()), Some(Severity::Warning));

    // What has no conversion is said again; nothing else changes.
    let twice = run.again(&set);
    assert_eq!(twice.changes.counts().edits(), 0);
    assert_eq!(twice.changes.counts().unconvertible, 1);
}

/// A fixup whose input does not answer.
struct Faulty {
    info: FixupInfo,
}

impl BinFixup for Faulty {
    fn info(&self) -> &FixupInfo {
        &self.info
    }

    fn applies(&self, _: &TargetTypes<'_>) -> bool {
        true
    }

    fn classes(&self) -> Classes {
        Classes::named(&["Material"])
    }

    fn fix(&self, cx: &mut FixCx<'_>, _: StructMut<'_>) {
        cx.fault(HistoryError::UnknownFamily("none".into()));
    }
}

#[test]
fn a_cancel_ends_fix_before_the_first_fixup_over_a_bin_with_no_fixed_mod() {
    // #ADR-0097: a step is one fixup over the objects of one bin.
    let env = env();
    let LoadRun {
        result,
        mut log,
        mut names,
        ..
    } = LoadRun::of(&env, LoadOptions::default(), &old_mod());
    let m = result.expect("a mod");
    let dating = Dating::default();
    let mut inputs = FixInputs {
        env: &env,
        dating: &dating,
        names: &mut names,
    };
    let cancel = Cancel::new();
    cancel.cancel();
    log.cancel_on(cancel);
    let set = FixupSet::standard();
    let applied = set.apply(m, &ObjectSet::every(), &mut inputs, &mut log.at(Stage::Fix));
    assert!(
        matches!(applied, Err(FixError::Cancelled(Cancelled(Stage::Fix)))),
        "the stage ends at its first step"
    );
    assert!(log.iter().all(|d| d.stage != Stage::Fix));
}

#[test]
fn a_fault_a_fixup_reports_ends_the_run_with_no_fixed_mod() {
    let env = env();
    let info = FixupInfo::built_in("faulty", "An input that does not answer", &[], Phase::Undo);
    let set = FixupSet::standard().with(Faulty { info });
    let run = LoadRun::of(&env, LoadOptions::default(), &old_mod());
    let LoadRun {
        result,
        mut log,
        mut names,
        ..
    } = run;
    let m = result.expect("a mod");
    let dating = Dating::default();
    let mut inputs = FixInputs {
        env: &env,
        dating: &dating,
        names: &mut names,
    };
    let applied = set.apply(m, &ObjectSet::every(), &mut inputs, &mut log.at(Stage::Fix));
    assert!(matches!(applied, Err(FixError::History(_))));
    assert!(log.iter().all(|d| d.stage != Stage::Fix));
}

#[test]
fn a_rule_with_the_id_of_a_built_in_fixup_takes_its_place() {
    let env = env();
    let narrower = Rule::move_fields(EMITTER)
        .into_field("Filtering", "VfxEmitterFiltering")
        .keep(["spectatorPolicy"])
        .id("vfx-filtering")
        .at_build(Version::new(12, 17));
    let set = FixupSet::from(&FixupSelection::standard().rule(narrower.clone()));
    assert_eq!(set.ids(), FixupSet::standard().ids());
    let set = FixupSet::from(&only_rules(FixupSelection::standard()).rule(narrower));
    assert_eq!(set.ids(), rules_set().ids());
    let run = FixRun::of(&env, &set, &old_mod());
    assert!(run.changes.of("vfx-filtering").is_empty());
    assert_eq!(run.field("A", "keywordsExcluded"), Some(&text("spectator")));
}

fn base_skin() -> Skin {
    skins::base_skin()
}

/// A skin bin whose entry names the emitter `Reached`, and a root bin the skin bin links with
/// the emitter `Spell` in it.
fn linked_mod() -> ModArchive {
    let root = "data/characters/test/test.bin";
    let old = |name: &str| emitter(name, vec![("keywordsExcluded", text("spectator"))]);
    let skin_entry = with_link(
        entry("Characters/Test/Skins/Skin0", &base_skin()),
        "idleParticlesEffects",
        "Reached",
    );
    ModArchive::new()
        .file(
            skins::WAD,
            &skins::skin_bin("test", 0),
            linked_bin(vec![skin_entry, old("Reached")], &[root]),
        )
        .file(skins::WAD, root, linked_bin(vec![old("Spell")], &[]))
}

#[test]
fn every_loaded_object_is_fixed_and_a_caller_may_narrow_the_set() {
    let env = env();
    let set = rules_set();
    let every = FixRun::of(&env, &set, &linked_mod());
    assert_eq!(every.changes.of("vfx-filtering").len(), 2);
    assert!(every.field("Spell", "Filtering").is_some());

    // What the units' entries reach: the port's set.
    let reached = |m: &reforge::model::Mod| {
        let units: Vec<UnitId> = m.units.ids().collect();
        ObjectSet::reached(m, &units)
    };
    let narrowed = FixRun::of_objects(&env, &set, &linked_mod(), &reached);
    let [change] = narrowed.changes.of("vfx-filtering") else {
        panic!("{:?}", narrowed.changes);
    };
    assert_eq!(change.object, narrowed.id("Reached"));
    assert!(narrowed.field("Reached", "Filtering").is_some());
    assert_eq!(
        narrowed.field("Spell", "keywordsExcluded"),
        Some(&text("spectator"))
    );
    let set_of = reached(&narrowed.fixed);
    assert!(set_of.contains(narrowed.id("Reached")));
    assert!(!set_of.contains(narrowed.id("Spell")));
    assert!(ObjectSet::every().contains(narrowed.id("Spell")));
}

#[test]
fn the_report_lists_every_change_with_whether_an_entry_reaches_its_object() {
    let env = env();
    let run = FixRun::of(&env, &rules_set(), &linked_mod());
    let speller = run.speller();
    let report = Report::builder()
        .module(&run.fixed)
        .changes(&run.changes, &run.fixed, &speller)
        .build();
    let json: serde_json::Value = serde_json::from_str(&report.to_json()).unwrap();
    let reached = run.id("Reached");
    let spell = run.id("Spell");
    assert_eq!(
        json["changes"],
        json!([{
            "fixup": "vfx-filtering",
            "changed": 2, "removed": 0, "lost": 0, "unconvertible": 0,
            "changes": [
                {
                    "object": reached.to_string(),
                    "entry": format!("{:08x}", name_hash("Reached").0),
                    "reached": true,
                    "path": "keywordsExcluded",
                    "before": { "kind": "String", "text": "\"spectator\"" },
                    "after": { "as": "moved", "to": "Filtering.keywordsExcluded" },
                },
                {
                    "object": spell.to_string(),
                    "entry": format!("{:08x}", name_hash("Spell").0),
                    "reached": false,
                    "path": "keywordsExcluded",
                    "before": { "kind": "String", "text": "\"spectator\"" },
                    "after": { "as": "moved", "to": "Filtering.keywordsExcluded" },
                },
            ],
        }])
    );
    // A report with no fixup run holds no such part.
    let plain = Report::builder().module(&run.fixed).build();
    assert!(plain.changes.is_none());
}

#[test]
fn a_set_with_nothing_to_do_gives_the_mod_back_as_it_was() {
    let env = env();
    let objects = vec![emitter(
        "E",
        vec![("emitterName", text("e")), ("on", flag(true))],
    )];
    let run = FixRun::of(&env, &rules_set(), &skin_archive(objects.clone()));
    assert!(run.changes.is_empty() && run.changes.edited().is_empty());
    assert_eq!(run.object("E"), &objects[0]);
    assert!(run.log.iter().all(|d| d.stage != Stage::Fix));
    let none = FixRun::of(&env, &FixupSet::empty(), &old_mod());
    assert!(none.changes.is_empty());
}
