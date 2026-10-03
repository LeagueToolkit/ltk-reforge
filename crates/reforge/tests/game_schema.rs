//! The meta schema: a field's type at a build, through the class's bases, and at the newest
//! build the schema describes when the install is past it.

use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_meta::PropertyKind as Kind;
use reforge::diag::{Channel, Diagnostics, Input, Render, Severity, Stage, Subject};
use reforge::game::{
    Build, FieldType, Schema, SchemaAbsent, SchemaError, SchemaInstallNewer, SchemaUnreadable,
};
use reforge::model::BinHash;
use serde_json::{Value, json};

const BASE: BinHash = BinHash(0x10);
const DERIVED: BinHash = BinHash(0x20);
const TEXTURE: BinHash = BinHash(0xa1);
const RESOURCE_MAP: BinHash = BinHash(0xb2);
const CHILDREN: BinHash = BinHash(0xc3);

const DB: &str = r#"{
  "formatVersion": 1,
  "latest": 300,
  "hashSource": { "fetchedAt": "2026-09-28T09:29:09Z" },
  "versions": [{ "patch": "1.1", "build": 100 }, { "patch": "1.3", "build": 300 }],
  "externalTypeNames": {},
  "classes": {
    "0x10": { "name": "Base", "revisions": [{ "from": 100, "bases": [] }],
      "properties": {
        "0xa1": { "name": "texture", "revisions": [
          { "from": 100, "to": 199, "type": ["String", "0x0", "0x0", "0x0"] },
          { "from": 200, "type": ["File", "0x0", "0x0", "0x0"] } ] } } },
    "0x20": { "name": "Derived", "revisions": [{ "from": 100, "bases": ["0x10"] }],
      "properties": {
        "0xb2": { "name": "resourceMap", "revisions": [
          { "from": 100, "type": ["Map", "Hash", "Link", "0x0"] } ] },
        "0xc3": { "name": "children", "revisions": [
          { "from": 100, "type": ["List2", "0x3", "Embed", "0x30"] } ] } } }
  }
}"#;

fn schema() -> Schema {
    Schema::parse(DB.as_bytes()).unwrap()
}

/// A database of these classes, at builds up to 300.
fn database(classes: Value) -> Schema {
    let json = json!({ "formatVersion": 1, "latest": 300, "classes": classes });
    Schema::parse(json.to_string().as_bytes()).unwrap()
}

/// A class with these bases since build 100.
fn class(bases: &[&str], fields: Value) -> Value {
    json!({ "revisions": [{ "from": 100, "bases": bases }], "properties": fields })
}

/// A field of one scalar kind since build 100.
fn field(kind: &str) -> Value {
    json!({ "revisions": [{ "from": 100, "type": [kind, "0x0", "0x0", "0x0"] }] })
}

fn kind(schema: &Schema, class: u32, field: u32, build: u64) -> Option<Kind> {
    schema
        .field_type(BinHash(class), BinHash(field), build)
        .map(|field_type| field_type.kind)
}

#[test]
fn a_type_is_read_at_a_build() {
    let s = schema();
    assert_eq!(
        s.field_type(BASE, TEXTURE, 150),
        Some(FieldType {
            kind: Kind::String,
            key: None,
            item: None,
            class: None
        })
    );
    assert_eq!(kind(&s, 0x10, 0xa1, 199), Some(Kind::String));
    assert_eq!(kind(&s, 0x10, 0xa1, 200), Some(Kind::WadChunkLink));
    assert_eq!(kind(&s, 0x10, 0xa1, 250), Some(Kind::WadChunkLink));
    assert_eq!(kind(&s, 0x10, 0xa1, 50), None, "before the class existed");
    assert_eq!(kind(&s, 0x10, 0xb2, 250), None, "a field the class lacks");
    assert_eq!(kind(&s, 0x99, 0xa1, 250), None, "a class the schema lacks");
}

#[test]
fn a_field_is_found_through_the_bases() {
    let s = schema();
    assert_eq!(kind(&s, 0x20, 0xa1, 250), Some(Kind::WadChunkLink));
    assert_eq!(
        s.field_type(DERIVED, RESOURCE_MAP, 250),
        Some(FieldType {
            kind: Kind::Map,
            key: Some(Kind::Hash),
            item: Some(Kind::ObjectLink),
            class: None
        })
    );
    // A list's second slot is not a key kind.
    assert_eq!(
        s.field_type(DERIVED, CHILDREN, 250),
        Some(FieldType {
            kind: Kind::UnorderedContainer,
            key: None,
            item: Some(Kind::Embedded),
            class: Some(BinHash(0x30))
        })
    );
}

#[test]
fn a_newer_install_reads_the_newest_build() {
    let s = schema();
    assert_eq!(s.at(999), 300);
    assert_eq!(s.at(150), 150);
    assert_eq!(kind(&s, 0x10, 0xa1, 999), Some(Kind::WadChunkLink));
    assert_eq!(s.generation(), Some("2026-09-28T09:29:09Z"));
    assert_eq!((s.earliest(), s.latest()), (100, 300));

    let newer = Build::parse("1.4.999").unwrap();
    let older = Build::parse("1.2.150").unwrap();
    assert_eq!(s.at_install(Some(&newer)), 300);
    assert_eq!(s.at_install(Some(&older)), 150);
    assert_eq!(
        s.at_install(None),
        300,
        "an install that does not say its build"
    );
}

#[test]
fn every_type_a_field_had_between_two_builds() {
    let s = schema();
    let kinds = |lo, hi| {
        s.field_types_between(DERIVED, TEXTURE, lo, hi)
            .map(|types| types.iter().map(|t| t.kind).collect::<Vec<_>>())
    };
    assert_eq!(kinds(150, 180), Some(vec![Kind::String]));
    assert_eq!(
        kinds(150, 250),
        Some(vec![Kind::String, Kind::WadChunkLink]),
        "through the base, both revisions"
    );
    assert_eq!(kinds(250, 999), Some(vec![Kind::WadChunkLink]));
    assert_eq!(
        kinds(50, 250),
        None,
        "before the schema's first build nothing is known"
    );
    assert_eq!(
        s.field_types_between(DERIVED, BinHash(0xdd), 150, 250),
        Some(Vec::new()),
        "a field no build of the range declares"
    );
}

#[test]
fn the_nearest_class_that_declares_a_field_gives_its_type() {
    let s = database(json!({
        "0x1": class(&[], json!({ "0xf": field("String") })),
        "0x2": class(&["0x1"], json!({ "0xf": field("Hash") })),
        "0x3": class(&["0x2"], json!({})),
    }));
    assert_eq!(kind(&s, 0x1, 0xf, 200), Some(Kind::String));
    assert_eq!(kind(&s, 0x2, 0xf, 200), Some(Kind::Hash));
    assert_eq!(
        kind(&s, 0x3, 0xf, 200),
        Some(Kind::Hash),
        "its base declares the field before its base's base"
    );
}

#[test]
fn bases_are_walked_through_several_levels_several_bases_and_a_cycle() {
    // 0x4 derives from 0x2 and 0x3, both from 0x1: a class reached twice. 0x5 and 0x6 name each
    // other as base.
    let s = database(json!({
        "0x1": class(&[], json!({ "0xa": field("String") })),
        "0x2": class(&["0x1"], json!({})),
        "0x3": class(&["0x1"], json!({ "0xb": field("Hash") })),
        "0x4": class(&["0x2", "0x3"], json!({})),
        "0x5": class(&["0x6"], json!({ "0xa": field("String") })),
        "0x6": class(&["0x5"], json!({})),
    }));
    assert_eq!(kind(&s, 0x4, 0xa, 200), Some(Kind::String));
    assert_eq!(kind(&s, 0x4, 0xb, 200), Some(Kind::Hash));
    assert_eq!(kind(&s, 0x2, 0xb, 200), None, "a sibling's field");
    assert_eq!(
        s.field_types_between(BinHash(0x4), BinHash(0xa), 100, 300)
            .map(|types| types.len()),
        Some(1),
        "the base reached by two ways is read once"
    );

    assert_eq!(kind(&s, 0x6, 0xa, 200), Some(Kind::String));
    assert_eq!(kind(&s, 0x6, 0xb, 200), None, "the walk of a cycle ends");
    assert!(s.derives_from(BinHash(0x5), BinHash(0x6), 200));
    assert!(!s.derives_from(BinHash(0x5), BinHash(0x1), 200));
}

#[test]
fn a_class_derives_from_its_bases_at_the_builds_it_has_them() {
    // 0x2 derives from 0x1 up to build 199, and from 0x3 since.
    let s = database(json!({
        "0x1": class(&[], json!({ "0xa": field("String") })),
        "0x3": {
            "revisions": [{ "from": 200, "bases": [] }],
            "properties": { "0xa": field("Hash") },
        },
        "0x2": { "revisions": [
            { "from": 100, "to": 199, "bases": ["0x1"] },
            { "from": 200, "bases": ["0x3"] },
        ] },
    }));
    let (old_base, derived, new_base) = (BinHash(0x1), BinHash(0x2), BinHash(0x3));
    assert!(s.derives_from(derived, derived, 150), "a class is its own");
    assert!(s.derives_from(derived, old_base, 150));
    assert!(!s.derives_from(derived, new_base, 150));
    assert!(!s.derives_from(derived, old_base, 250));
    assert!(s.derives_from(derived, new_base, 250));
    assert!(
        s.derives_from(derived, new_base, 999),
        "past the newest build"
    );
    assert!(!s.derives_from(old_base, derived, 150), "not the other way");

    assert_eq!(kind(&s, 0x2, 0xa, 150), Some(Kind::String));
    assert_eq!(kind(&s, 0x2, 0xa, 250), Some(Kind::Hash));
    let kinds = |lo, hi| {
        s.field_types_between(derived, BinHash(0xa), lo, hi)
            .map(|types| types.iter().map(|t| t.kind).collect::<Vec<_>>())
    };
    assert_eq!(kinds(100, 199), Some(vec![Kind::String]));
    assert_eq!(kinds(200, 300), Some(vec![Kind::Hash]));
    let mut both = kinds(100, 300).unwrap();
    both.sort_by_key(|kind| *kind as u8);
    assert_eq!(both, [Kind::String, Kind::Hash], "through both its bases");
}

#[test]
fn a_class_is_known_and_exists_at_its_builds() {
    let s = database(json!({
        "0x1": { "revisions": [{ "from": 150, "to": 199, "bases": [] }] },
        "0x2": { "revisions": [{ "from": 100, "bases": [] }] },
    }));
    let (gone, kept, never) = (BinHash(0x1), BinHash(0x2), BinHash(0x9));
    assert!(s.knows_class(gone) && s.knows_class(kept) && !s.knows_class(never));
    assert!(!s.has_class_at(gone, 149));
    assert!(s.has_class_at(gone, 150));
    assert!(s.has_class_at(gone, 199));
    assert!(!s.has_class_at(gone, 200));
    assert!(s.has_class_at(kept, 999), "past the newest build");
    assert!(!s.has_class_at(never, 200));
    assert_eq!(s.earliest(), 100);
    assert_eq!(s.generation(), None);
}

#[test]
fn a_database_this_reader_does_not_read_says_why() {
    let error = |json: &str| Schema::parse(json.as_bytes()).unwrap_err();

    let other_format = error(r#"{ "formatVersion": 2, "latest": 300, "classes": {} }"#);
    assert!(
        matches!(other_format, SchemaError::Format { found: 2 }),
        "{other_format}"
    );
    assert!(matches!(error("not json"), SchemaError::Json(_)));
    assert!(matches!(
        error(r#"{ "formatVersion": 1 }"#),
        SchemaError::Json(_)
    ));

    let one_class = |key: &str, base: &str, field: &str, slots: [&str; 4]| {
        let revision = json!({ "revisions": [{ "from": 100, "type": slots }] });
        let json = json!({ "formatVersion": 1, "latest": 300, "classes": {
            key: { "revisions": [{ "from": 100, "bases": [base] }], "properties": { field: revision } },
        } });
        error(&json.to_string())
    };
    let string = ["String", "0x0", "0x0", "0x0"];
    for (found, text) in [
        (one_class("Base", "0x1", "0xa", string), "Base"),
        (one_class("0x2", "one", "0xa", string), "one"),
        (one_class("0x2", "0x1", "texture", string), "texture"),
        (
            one_class("0x2", "0x1", "0xa", ["Pointer", "0x0", "0x0", "Base"]),
            "Base",
        ),
    ] {
        assert!(
            matches!(&found, SchemaError::Hash { text: named } if named == text),
            "{found}"
        );
    }
    // A kind this reader does not know is not skipped: its field would read as undeclared.
    for (found, name) in [
        (
            one_class("0x2", "0x1", "0xa", ["Quaternion", "0x0", "0x0", "0x0"]),
            "Quaternion",
        ),
        (
            one_class("0x2", "0x1", "0xa", ["List", "0x0", "Quaternion", "0x0"]),
            "Quaternion",
        ),
        (
            one_class("0x2", "0x1", "0xa", ["Map", "0x0", "String", "0x0"]),
            "0x0",
        ),
    ] {
        assert!(
            matches!(&found, SchemaError::Kind { name: named } if named == name),
            "{found}"
        );
    }
    // A type is four slots.
    let three = error(
        r#"{ "formatVersion": 1, "latest": 300, "classes": { "0x2": { "properties": { "0xa": {
             "revisions": [{ "from": 100, "type": ["String", "0x0", "0x0"] }] } } } } }"#,
    );
    assert!(matches!(three, SchemaError::Json(_)), "{three}");
}

#[test]
fn a_database_is_opened_from_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join("meta-schema.json")).unwrap();
    fs::write(&path, DB).unwrap();
    let s = Schema::open(&path).unwrap();
    assert_eq!(kind(&s, 0x20, 0xa1, 250), Some(Kind::WadChunkLink));

    let missing = Schema::open(&path.with_file_name("none.json")).unwrap_err();
    assert!(matches!(missing, SchemaError::Io(_)), "{missing}");
}

#[test]
fn no_database_and_one_that_does_not_read_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join("meta-schema.json")).unwrap();
    let mut log = Diagnostics::new();

    assert!(Schema::open_reported(None, &mut log.at(Stage::Read)).is_none());
    fs::write(
        &path,
        r#"{ "formatVersion": 7, "latest": 1, "classes": {} }"#,
    )
    .unwrap();
    assert!(Schema::open_reported(Some(&path), &mut log.at(Stage::Read)).is_none());
    fs::write(&path, DB).unwrap();
    assert!(Schema::open_reported(Some(&path), &mut log.at(Stage::Read)).is_some());

    assert_eq!(log.len(), 2);
    assert_eq!(log.of::<SchemaAbsent>().count(), 1);
    let unreadable: Vec<&SchemaUnreadable> = log.of::<SchemaUnreadable>().collect();
    assert_eq!(unreadable.len(), 1);
    assert_eq!(
        unreadable[0].reason,
        "schema format 7, and this reads format 1"
    );

    let mut found = log.iter();
    let absent = found.next().unwrap();
    assert_eq!(absent.code.name, "schema.absent");
    assert_eq!(absent.severity(), Severity::Note);
    assert_eq!(absent.channel(), Channel::Run);
    assert_eq!(absent.subject, Subject::Input(Input::Schema));
    assert_eq!(
        absent.message(&Render::plain()),
        "no meta schema database; types come from the install's bins"
    );
    let unreadable = found.next().unwrap();
    assert_eq!(unreadable.code.name, "schema.unreadable");
    assert_eq!(unreadable.severity(), Severity::Warning);
    assert_eq!(unreadable.channel(), Channel::Run);
    assert_eq!(unreadable.subject, Subject::Input(Input::Schema));
    assert_eq!(
        unreadable.message(&Render::plain()),
        "the meta schema database does not read: schema format 7, and this reads format 1; \
         types come from the install's bins"
    );
}

#[test]
fn an_install_past_the_schema_is_reported() {
    let s = schema();
    let mut log = Diagnostics::new();
    s.report(None, &mut log.at(Stage::Read));
    s.report(Build::parse("1.2.150").as_ref(), &mut log.at(Stage::Read));
    s.report(Build::parse("1.3.300").as_ref(), &mut log.at(Stage::Read));
    assert!(log.is_empty());

    let newer = Build::parse("1.4.301").unwrap();
    s.report(Some(&newer), &mut log.at(Stage::Read));
    let found: Vec<&SchemaInstallNewer> = log.of::<SchemaInstallNewer>().collect();
    assert_eq!(found.len(), 1);
    assert_eq!((&found[0].install, found[0].latest), (&newer, 300));

    let diagnostic = log.iter().next().unwrap();
    assert_eq!(diagnostic.code.name, "schema.install-newer");
    assert_eq!(diagnostic.severity(), Severity::Warning);
    assert_eq!(diagnostic.channel(), Channel::Run);
    assert_eq!(diagnostic.subject, Subject::Input(Input::Schema));
    assert_eq!(
        diagnostic.message(&Render::plain()),
        "the install is build 301, and the schema describes up to 300; types changed since are \
         known from the install's bins only"
    );
    assert_eq!(diagnostic.data()["install"], "1.4.301");
    assert_eq!(diagnostic.data()["latest"], 300);
}

#[test]
fn a_schema_is_shared_between_threads() {
    fn shared<T: Send + Sync>() {}
    shared::<Schema>();
}
