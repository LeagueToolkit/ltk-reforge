//! The checks: a bin written again read back against what it was built to hold, and every
//! value written typed as the target build reads it. What a check finds does not stop the
//! write, unless a caller's policy says so.

use std::collections::BTreeSet;

use ltk_meta::property::values;
use ltk_meta::{BinObject, PropertyKind, PropertyValueEnum};
use reforge::diag::{DiagnosticPolicy, Diagnostics, Record, Render, Severity, Stage};
use reforge::fixups::TargetTypes;
use reforge::game::{Build, Schema};
use reforge::model::entity::FileId;
use reforge::model::known::field;
use reforge::model::{
    BinHash, ByteView, ChunkPath, FieldPath, SkinEntry, Spelled, WadName, name_hash, path_hash,
};
use reforge::package::{
    Against, BaseSkin, BinWrite, Blocked, Check, CheckCx, CheckInfo, CheckSelection, CheckSet,
    Chunk, ChunkKey, ClassUndeclared, Coverage, Done, FieldUndeclared, HeldBy, Layer, Origin,
    Package, ProjectConfig, Refused, RoundTripFailed, RoundTripProblem, SkinValue, TypeMismatch,
    TypePlaces, Types, WrittenBin, permit,
};
use reforge_testkit::SchemaRows;
use reforge_testkit::bins::{bin, float, object, pointer, text};
use reforge_testkit::schema::{field, kind, list_of, map_of, pointer as to};
use reforge_testkit::skins::WAD;
use reforge_testkit::sounds::{clip, media_bank, package as voice_package};
use reforge_wwise::CLIENT_VERSION;
use serde_json::json;

const SKIN_BIN: &str = "data/characters/test/skins/skin0.bin";

fn config() -> ProjectConfig {
    ProjectConfig {
        name: "Synthetic".into(),
        display_name: Spelled::new("Synthetic"),
        version: "1.0.0".into(),
        description: Spelled::new("A package built by hand."),
        authors: Vec::new(),
        champions: vec!["Test".into()],
    }
}

fn key(path: &str) -> ChunkKey {
    let wad = WadName::parse(WAD).expect("a WAD name");
    ChunkKey::new(wad, ChunkPath::parse(path).expect("a path"))
}

/// A package of one chunk: the skin bin.
fn package_of(chunk: Chunk) -> Package {
    let mut package = Package::new(config());
    package.insert(Layer::Base, key(SKIN_BIN), chunk);
    package
}

fn file() -> FileId {
    FileId::new(0)
}

/// The object `A` as the mod ships it, and as a fixup left it.
fn shipped() -> BinObject {
    object(
        "A",
        "Material",
        vec![("junk", float(1.0)), ("count", float(2.0))],
    )
}

fn fixed() -> BinObject {
    object("A", "Material", vec![("count", float(2.0))])
}

fn other() -> BinObject {
    object("B", "Material", vec![("textureName", text("Diffuse"))])
}

/// A bin written again, built to hold `A` as fixed and `B`, with these bytes.
fn rewritten(bytes: Vec<u8>) -> Chunk {
    let origin = Origin::Rewritten {
        file: file(),
        written: WrittenBin {
            how: BinWrite::Delta,
            entries: BTreeSet::from([name_hash("A"), name_hash("B")]),
            edited: vec![fixed()],
        },
    };
    Chunk::new(bytes.into(), origin)
}

fn kept(objects: Vec<BinObject>) -> Chunk {
    Chunk::new(ByteView::from(bin(objects)), Origin::Kept(file()))
}

/// A target whose `Material` holds a count, a texture name, and a child, a list of children
/// and a map of children of the class `Leaf`, which `Special` derives from.
fn schema() -> Schema {
    SchemaRows::new(300)
        .class(
            "Material",
            &[
                field("count", kind("U8")),
                field("textureName", kind("String")),
                field("child", to("Leaf")),
                field("children", list_of("Leaf")),
                field("byName", map_of("Hash", "Leaf")),
            ],
        )
        .class("Leaf", &[field("kept", kind("F32"))])
        .derived("Special", &["Leaf"], &[field("more", kind("F32"))])
        .class("Stranger", &[])
        .build()
}

/// Runs a set of checks on a package against a schema, and gives what they raised.
fn checked(package: &Package, schema: Option<&Schema>, checks: &CheckSet) -> Diagnostics {
    let build = Build::parse("16.19.300");
    let types = TargetTypes::new(schema, build.as_ref());
    let mut log = Diagnostics::new();
    let cx = CheckCx::new(package, &types);
    checks
        .run(&cx, &mut log.at(Stage::Check))
        .expect("a log with no cancel runs every check");
    log
}

fn round_trip(bytes: Vec<u8>) -> Diagnostics {
    let schema = schema();
    let set = CheckSet::from(&CheckSelection::standard().without("types"));
    checked(&package_of(rewritten(bytes)), Some(&schema), &set)
}

#[test]
fn a_bin_written_again_that_reads_back_as_built_passes() {
    let log = round_trip(bin(vec![fixed(), other()]));
    assert_eq!(log.of::<RoundTripFailed>().count(), 0);
    let covered = Coverage::RoundTrip {
        bins: 1,
        objects: 1,
        banks: 0,
    };
    assert_eq!(log.of::<Done>().collect::<Vec<_>>(), [&Done(covered)]);
    assert_eq!(log.worst(), Some(Severity::Note));
}

#[test]
fn the_round_trip_finds_a_changed_object_that_reads_back_otherwise() {
    // The bytes hold `A` as the mod shipped it, not as the fixup left it.
    let log = round_trip(bin(vec![shipped(), other()]));
    let [found] = log.of::<RoundTripFailed>().collect::<Vec<_>>()[..] else {
        panic!("one bin does not read back");
    };
    assert_eq!(found.path.as_str(), SKIN_BIN);
    assert_eq!(found.wad.as_str(), WAD);
    let entry = name_hash("A");
    assert_eq!(found.problems, [RoundTripProblem::Object { entry }]);
    let diagnostic = log.iter().next().expect("the finding is raised");
    assert_eq!(diagnostic.code.name, "check.round-trip-failed");
    assert_eq!(diagnostic.severity(), Severity::Error);
    assert_eq!(diagnostic.by.as_deref(), Some("round-trip"));
    assert_eq!(diagnostic.subject.id(), SKIN_BIN);
    assert_eq!(
        diagnostic.message(&Render::plain()),
        format!(
            "the bin written again does not read back as built: the object {:08x} reads back \
             otherwise",
            entry.0
        )
    );
}

#[test]
fn the_round_trip_finds_a_bin_that_holds_other_entries() {
    let log = round_trip(bin(vec![fixed()]));
    let found = log.of::<RoundTripFailed>().next().expect("it is found");
    assert_eq!(
        found.problems,
        [RoundTripProblem::Entries { built: 2, read: 1 }]
    );
}

#[test]
fn the_round_trip_finds_a_bin_that_does_not_read() {
    let log = round_trip(b"PROP and nothing a reader takes".to_vec());
    let found = log.of::<RoundTripFailed>().next().expect("it is found");
    let [RoundTripProblem::Unreadable { reason }] = &found.problems[..] else {
        panic!("the bin does not read: {:?}", found.problems);
    };
    assert!(!reason.is_empty());
    assert_eq!(
        serde_json::to_value(found).expect("a finding is data"),
        json!({
            "wad": WAD,
            "path": SKIN_BIN,
            "problems": [{ "problem": "unreadable", "reason": reason }],
        })
    );
}

/// #GAME-BEHAVIOR-0015: a sound bank loads at the client's version only, and a package
/// holds no version.
#[test]
fn the_round_trip_finds_a_bank_made_that_the_client_does_not_load() {
    let made = |bytes: Vec<u8>| Chunk::new(bytes.into(), Origin::Bank(file()));
    let media = |version: u32| media_bank("made_sfx_audio", version, vec![clip(1, 1)]);
    let mut package = Package::new(config());
    let banks = [
        ("assets/sounds/mod/a/client.bnk", media(CLIENT_VERSION)),
        ("assets/sounds/mod/a/older.bnk", media(134)),
        ("assets/sounds/mod/a/torn.bnk", b"BKHD".to_vec()),
        (
            "assets/sounds/mod/a/voice.wpk",
            voice_package(vec![clip(3, 3)]),
        ),
    ];
    for (path, bytes) in banks {
        package.insert(Layer::Base, key(path), made(bytes));
    }
    let schema = schema();
    let set = CheckSet::from(&CheckSelection::standard().without("types"));
    let log = checked(&package, Some(&schema), &set);

    let [older, torn] = log.of::<RoundTripFailed>().collect::<Vec<_>>()[..] else {
        panic!("two banks do not load");
    };
    assert_eq!(older.path.as_str(), "assets/sounds/mod/a/older.bnk");
    let reason = "a bank of version 134".to_string();
    assert_eq!(older.problems, [RoundTripProblem::Unreadable { reason }]);
    assert_eq!(torn.path.as_str(), "assets/sounds/mod/a/torn.bnk");
    assert!(matches!(
        torn.problems[..],
        [RoundTripProblem::Unreadable { .. }]
    ));
    let covered = Coverage::RoundTrip {
        bins: 0,
        objects: 0,
        banks: 4,
    };
    assert_eq!(log.of::<Done>().collect::<Vec<_>>(), [&Done(covered)]);
}

#[test]
fn a_file_kept_as_it_is_is_not_read_back() {
    let schema = schema();
    let set = CheckSet::from(&CheckSelection::standard().without("types"));
    let package = package_of(Chunk::new(
        ByteView::from(b"no bin at all"),
        Origin::Kept(file()),
    ));
    let log = checked(&package, Some(&schema), &set);
    let covered = Coverage::RoundTrip {
        bins: 0,
        objects: 0,
        banks: 0,
    };
    assert_eq!(log.of::<Done>().collect::<Vec<_>>(), [&Done(covered)]);
    assert_eq!(log.len(), 1);
}

/// A material with a count of another kind, a field its class does not declare, a child of
/// a derived class with a field that class does not declare, and a second child of a class
/// that is no `Leaf`.
fn mistyped() -> Vec<BinObject> {
    vec![
        object(
            "A",
            "Material",
            vec![
                ("count", float(2.0)),
                ("junk", float(1.0)),
                ("textureName", text("Diffuse")),
                (
                    "child",
                    pointer("Special", vec![("more", float(1.0)), ("stray", float(1.0))]),
                ),
            ],
        ),
        object(
            "B",
            "Material",
            vec![
                ("count", float(3.0)),
                ("child", pointer("Stranger", vec![])),
            ],
        ),
    ]
}

fn types_only() -> CheckSet {
    CheckSet::from(&CheckSelection::standard().without("round-trip"))
}

#[test]
fn the_type_check_finds_a_value_of_another_type() {
    let schema = schema();
    let log = checked(&package_of(kept(mistyped())), Some(&schema), &types_only());
    let found = log.of::<TypeMismatch>().next().expect("it is found");
    assert_eq!(found.values, 3);
    let rows: Vec<_> = found
        .fields
        .iter()
        .map(|row| (row.field, row.found.kind, row.wants.kind, row.values))
        .collect();
    assert_eq!(
        rows,
        [
            (name_hash("count"), "F32", "U8", 2),
            (name_hash("child"), "Pointer", "Pointer", 1),
        ]
    );
    // The first value of each is named, and a child of a derived class reads as its base.
    assert_eq!(found.fields[0].first.entry, name_hash("A"));
    assert_eq!(found.fields[1].first.entry, name_hash("B"));
    assert_eq!(found.fields[1].found.class, Some(name_hash("Stranger")));
    assert_eq!(found.fields[1].wants.class, Some(name_hash("Leaf")));

    let diagnostic = log.iter().next().expect("the finding is raised");
    assert_eq!(diagnostic.code.name, "check.type-mismatch");
    assert_eq!(diagnostic.severity(), Severity::Error);
    assert_eq!(diagnostic.by.as_deref(), Some("types"));
    assert_eq!(diagnostic.record, Some(Record::game_behavior(2)));
    let hex = |name: &str| format!("{:08x}", name_hash(name).0);
    assert_eq!(
        diagnostic.message(&Render::plain()),
        format!(
            "3 values the client skips, of another type than the target build declares: \
             {material}.{count} holds F32, the build reads U8 (2), {material}.{child} holds \
             Pointer<{stranger}>, the build reads Pointer<{leaf}> (1)",
            material = hex("Material"),
            count = hex("count"),
            child = hex("child"),
            stranger = hex("Stranger"),
            leaf = hex("Leaf"),
        )
    );
}

#[test]
fn the_type_check_finds_a_field_its_class_does_not_declare() {
    let schema = schema();
    let log = checked(&package_of(kept(mistyped())), Some(&schema), &types_only());
    let found = log.of::<FieldUndeclared>().next().expect("it is found");
    let rows: Vec<_> = found
        .fields
        .iter()
        .map(|row| (row.class, row.field, row.values))
        .collect();
    assert_eq!(
        rows,
        [
            (name_hash("Material"), name_hash("junk"), 1),
            (name_hash("Special"), name_hash("stray"), 1),
        ]
    );
    let against = Against::Schema { build: 300 };
    let covered = Coverage::Types {
        bins: 1,
        // Four fields of `A`, two of its child, two of `B`. The child of `B` is not read.
        fields: 8,
        against,
    };
    assert_eq!(log.of::<Done>().collect::<Vec<_>>(), [&Done(covered)]);
}

/// A list of pointers to structs of these classes.
fn children(classes: &[&str]) -> PropertyValueEnum {
    let items = classes.iter().map(|class| pointer(class, vec![])).collect();
    let list = values::Container::new(PropertyKind::Struct, items)
        .unwrap_or_else(|error| panic!("a list of {classes:?} does not build: {error}"));
    PropertyValueEnum::Container(list)
}

/// A map of pointers to structs of these classes, each under the hash of its key.
fn by_name(keyed: &[(&str, &str)]) -> PropertyValueEnum {
    let entries = keyed.iter().map(|(key, class)| {
        let key = PropertyValueEnum::Hash(values::Hash::new(name_hash(key)));
        (key, pointer(class, vec![]))
    });
    let map = values::Map::new(PropertyKind::Hash, PropertyKind::Struct, entries.collect())
        .unwrap_or_else(|error| panic!("a map of {keyed:?} does not build: {error}"));
    PropertyValueEnum::Map(map)
}

/// The rows of a type check of one material holding these fields: field, the class found,
/// how many values.
fn mismatches(fields: Vec<(&str, PropertyValueEnum)>) -> Vec<(BinHash, Option<BinHash>, usize)> {
    let schema = schema();
    let objects = vec![object("A", "Material", fields)];
    let log = checked(&package_of(kept(objects)), Some(&schema), &types_only());
    let found = log.of::<TypeMismatch>().flat_map(|found| &found.fields);
    found
        .map(|row| (row.field, row.found.class, row.values))
        .collect()
}

#[test]
fn a_struct_its_field_does_not_take_second_in_a_list_is_a_value_of_another_type() {
    let list = children(&["Leaf", "Stranger", "Stranger"]);
    let rows = mismatches(vec![("children", list)]);
    let stranger = Some(name_hash("Stranger"));
    assert_eq!(rows, [(name_hash("children"), stranger, 2)]);
}

#[test]
fn a_struct_its_field_does_not_take_under_a_later_key_of_a_map_is_a_value_of_another_type() {
    let map = by_name(&[("first", "Leaf"), ("second", "Stranger")]);
    let rows = mismatches(vec![("byName", map)]);
    assert_eq!(
        rows,
        [(name_hash("byName"), Some(name_hash("Stranger")), 1)]
    );
}

#[test]
fn a_struct_of_a_derived_class_second_in_a_list_reads_as_its_base() {
    let list = children(&["Leaf", "Special"]);
    assert_eq!(mismatches(vec![("children", list)]), []);
}

/// The places the type check gives for a package, against the schema.
fn places_of(package: &Package) -> TypePlaces {
    let schema = schema();
    let build = Build::parse("16.19.300");
    let types = TargetTypes::new(Some(&schema), build.as_ref());
    Types::places(&CheckCx::new(package, &types))
}

#[test]
fn the_type_check_places_each_value_by_its_path_in_its_object() {
    // The second and third children are strangers, and the child's own field holds text.
    let child = pointer("Special", vec![("more", text("one"))]);
    let fields = vec![
        ("children", children(&["Leaf", "Stranger", "Stranger"])),
        ("child", child),
    ];
    let package = package_of(kept(vec![object("A", "Material", fields)]));
    let places = places_of(&package);
    let material = name_hash("Material");
    let root = FieldPath::root();
    let children = root.field(material, name_hash("children"));
    let more = root
        .field(material, name_hash("child"))
        .field(name_hash("Special"), name_hash("more"));

    let rows: Vec<(BinHash, usize, Vec<&FieldPath>)> = places
        .mismatched
        .iter()
        .map(|detailed| {
            let at = detailed.places.iter().map(|place| &place.at).collect();
            (detailed.row.field, detailed.row.values, at)
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                name_hash("children"),
                2,
                vec![&children.item(1), &children.item(2)]
            ),
            (name_hash("more"), 1, vec![&more]),
        ]
    );
    let place = &places.mismatched[0].places[0].place;
    assert_eq!(place.entry, name_hash("A"));
    assert_eq!(place.path.as_str(), SKIN_BIN);
    assert!(places.undeclared.is_empty());
    assert!(places.classes.is_empty());

    // The run's finding holds the same rows, with no place but the first.
    let log = checked(&package, Some(&schema()), &types_only());
    let found = log.of::<TypeMismatch>().next().expect("it is found");
    let rows: Vec<_> = places.mismatched.iter().map(|d| &d.row).collect();
    assert_eq!(found.fields.iter().collect::<Vec<_>>(), rows);
}

#[test]
fn the_type_check_places_an_undeclared_field_and_an_undeclared_class() {
    let material = name_hash("Material");
    let root = FieldPath::root();
    let child = root.field(material, name_hash("child"));

    // `junk` is on the material, `stray` on the child's struct.
    let places = places_of(&package_of(kept(mistyped())));
    let undeclared: Vec<(BinHash, Vec<&FieldPath>)> = places
        .undeclared
        .iter()
        .map(|d| (d.row.field, d.places.iter().map(|p| &p.at).collect()))
        .collect();
    assert_eq!(
        undeclared,
        [
            (
                name_hash("junk"),
                vec![&root.field(material, name_hash("junk"))]
            ),
            (
                name_hash("stray"),
                vec![&child.field(name_hash("Special"), name_hash("stray"))]
            ),
        ]
    );

    // A ghost object is at its root, a ghost child at the field that holds it.
    let places = places_of(&package_of(kept(ghosts())));
    let classes: Vec<(Option<HeldBy>, Vec<&FieldPath>)> = places
        .classes
        .iter()
        .map(|d| (d.row.held_by, d.places.iter().map(|p| &p.at).collect()))
        .collect();
    let by = HeldBy {
        class: material,
        field: name_hash("child"),
    };
    assert_eq!(classes, [(None, vec![&root]), (Some(by), vec![&child])]);
}

/// Runs the type check on a package against these types, and gives what it raised.
fn typed(package: &Package, types: &TargetTypes<'_>) -> Diagnostics {
    let mut log = Diagnostics::new();
    let cx = CheckCx::new(package, types);
    types_only()
        .run(&cx, &mut log.at(Stage::Check))
        .expect("a log with no cancel runs every check");
    log
}

/// The rows of the classes the target build does not declare: the field that holds them, the
/// class, how many.
fn undeclared_classes(log: &Diagnostics) -> Vec<(Option<HeldBy>, BinHash, usize)> {
    let found = log.of::<ClassUndeclared>().flat_map(|found| &found.classes);
    found
        .map(|row| (row.held_by, row.held, row.structs))
        .collect()
}

/// Objects of the class `Ghost`, which the schema lacks: one alone, and one as a material's
/// child.
fn ghosts() -> Vec<BinObject> {
    vec![
        object("G", "Ghost", vec![]),
        object("A", "Material", vec![("child", pointer("Ghost", vec![]))]),
    ]
}

#[test]
fn a_struct_of_a_class_the_schema_lacks_in_a_field_taking_a_declared_class_is_undeclared() {
    let schema = schema();
    let fields = vec![
        ("child", pointer("Ghost", vec![("kept", float(1.0))])),
        ("children", children(&["Leaf", "Ghost"])),
    ];
    let objects = vec![object("A", "Material", fields)];
    let log = checked(&package_of(kept(objects)), Some(&schema), &types_only());
    let held_by = |field: &str| HeldBy {
        class: name_hash("Material"),
        field: name_hash(field),
    };
    let ghost = name_hash("Ghost");
    assert_eq!(
        undeclared_classes(&log),
        [
            (Some(held_by("child")), ghost, 1),
            (Some(held_by("children")), ghost, 1),
        ]
    );
    assert_eq!(
        log.of::<TypeMismatch>().count(),
        0,
        "no value of another type"
    );

    let diagnostic = log.iter().next().expect("the finding is raised");
    assert_eq!(diagnostic.code.name, "check.class-undeclared");
    assert_eq!(diagnostic.severity(), Severity::Error);
    assert_eq!(diagnostic.by.as_deref(), Some("types"));
    assert_eq!(diagnostic.record, Some(Record::mod_artifact(39)));
    let hex = |name: &str| format!("{:08x}", name_hash(name).0);
    assert_eq!(
        diagnostic.message(&Render::plain()),
        format!(
            "2 structs of a class the target build does not declare: {material}.{child} holds \
             {ghost} (1), {material}.{children} holds {ghost} (1)",
            material = hex("Material"),
            child = hex("child"),
            children = hex("children"),
            ghost = hex("Ghost"),
        )
    );
}

#[test]
fn a_root_object_of_a_class_the_schema_lacks_is_undeclared() {
    let schema = schema();
    let objects = vec![object("G", "Ghost", vec![("kept", float(1.0))])];
    let log = checked(&package_of(kept(objects)), Some(&schema), &types_only());
    assert_eq!(undeclared_classes(&log), [(None, name_hash("Ghost"), 1)]);
    let diagnostic = log.iter().next().expect("the finding is raised");
    assert_eq!(
        diagnostic.message(&Render::plain()),
        format!(
            "1 struct of a class the target build does not declare: {:08x} as an object (1)",
            name_hash("Ghost").0
        )
    );
}

#[test]
fn a_struct_of_a_class_the_schema_lacks_typed_against_the_installs_bins_raises_nothing() {
    let schema = schema();
    let build = Build::parse("16.19.300");
    let mut types = TargetTypes::new(Some(&schema), build.as_ref());
    // The install holds a material with a field the schema does not describe.
    let leaf = pointer("Leaf", vec![]);
    types.witness(&object("W", "Material", vec![("extra", leaf)]));
    let ghost = pointer("Ghost", vec![]);
    let objects = vec![object("A", "Material", vec![("extra", ghost)])];
    let log = typed(&package_of(kept(objects)), &types);
    assert_eq!(log.len(), 1, "what the check covered and nothing else");
}

/// The schema lacks some classes the game's data holds.
#[test]
fn a_class_the_schema_lacks_that_a_bin_of_the_install_holds_is_declared() {
    let schema = schema();
    let build = Build::parse("16.19.300");
    let mut types = TargetTypes::new(Some(&schema), build.as_ref());
    types.witness(&object("W", "Ghost", vec![]));
    let log = typed(&package_of(kept(ghosts())), &types);
    // Nor is the child a value of another type: the bases of the class are not known.
    assert_eq!(log.len(), 1, "what the check covered and nothing else");
}

#[test]
fn without_a_schema_no_class_is_undeclared() {
    let build = Build::parse("16.19.300");
    let types = TargetTypes::new(None, build.as_ref());
    let log = typed(&package_of(kept(ghosts())), &types);
    assert_eq!(log.of::<ClassUndeclared>().count(), 0);
    assert_eq!(log.len(), 1, "what the check covered and nothing else");
}

#[test]
fn values_of_the_types_the_build_reads_raise_nothing() {
    let schema = schema();
    let objects = vec![object(
        "A",
        "Material",
        vec![
            ("count", values::U8::new(2).into()),
            ("child", pointer("Leaf", vec![("kept", float(1.0))])),
        ],
    )];
    let log = checked(&package_of(kept(objects)), Some(&schema), &types_only());
    assert_eq!(log.len(), 1, "what the check covered and nothing else");
    assert_eq!(log.worst(), Some(Severity::Note));
}

#[test]
fn without_a_schema_the_types_are_the_ones_the_installs_bins_hold() {
    let build = Build::parse("16.19.300");
    let mut types = TargetTypes::new(None, build.as_ref());
    // The install holds a material with a count of one byte.
    types.witness(&object(
        "G",
        "Material",
        vec![("count", values::U8::new(1).into())],
    ));
    let package = package_of(kept(mistyped()));
    let mut log = Diagnostics::new();
    let cx = CheckCx::new(&package, &types);
    types_only()
        .run(&cx, &mut log.at(Stage::Check))
        .expect("a log with no cancel runs every check");
    let found = log.of::<TypeMismatch>().next().expect("it is found");
    assert_eq!((found.values, found.fields.len()), (2, 1));
    // Nothing is said of a field no bin of the install holds.
    assert_eq!(log.of::<FieldUndeclared>().count(), 0);
    let done = log
        .of::<Done>()
        .next()
        .expect("the check says what it covered");
    assert!(matches!(
        done.0,
        Coverage::Types {
            against: Against::Install,
            ..
        }
    ));
}

#[test]
fn what_a_check_finds_does_not_stop_the_write_unless_the_policy_says_so() {
    let mut log = round_trip(bin(vec![shipped(), other()]));
    assert_eq!(log.worst(), Some(Severity::Error));
    assert_eq!(permit(&DiagnosticPolicy::default(), &mut log), Ok(()));
    assert_eq!(log.of::<Blocked>().count(), 0);

    let refused = permit(&DiagnosticPolicy::strict(), &mut log);
    let failed = log.iter().next().expect("the check's finding").id;
    let blocked = log.iter().last().expect("the refusal");
    assert_eq!(refused, Err(Refused(blocked.id)));
    assert_eq!(blocked.code.name, "write.blocked");
    assert_eq!(blocked.stage, Stage::Write);
    assert_eq!(
        log.of::<Blocked>().next(),
        Some(&Blocked { by: vec![failed] })
    );
}

#[test]
fn the_checks_are_listed_and_chosen_as_data() {
    let catalog = CheckSet::catalog();
    let ids: Vec<&str> = catalog.iter().map(|info| info.id.as_str()).collect();
    assert_eq!(ids, ["types", "round-trip", "apply", "base-skin"]);
    assert!(catalog.iter().all(|info| info.default));
    let json = serde_json::to_value(&catalog[0]).expect("a description is data");
    assert_eq!(
        json,
        json!({
            "id": "types",
            "title": "Every value written is typed as the target build reads it",
            "record": { "series": "game-behavior", "number": 2 },
            "default": true,
        })
    );
    let back: CheckInfo = serde_json::from_value(json).expect("it reads back");
    assert_eq!(back, catalog[0]);

    let selection = CheckSelection::standard().without("types").with("nothing");
    let text = serde_json::to_string(&selection).expect("a selection is data");
    assert_eq!(text, r#"{"off":["types"],"on":["nothing"]}"#);
    let read: CheckSelection = serde_json::from_str(&text).expect("it reads back");
    assert_eq!(read, selection);
    let unknown: Vec<&str> = read.unknown().iter().map(|id| id.as_str()).collect();
    assert_eq!(unknown, ["nothing"]);
    let set = CheckSet::from(&read);
    let ids: Vec<&str> = set.infos().iter().map(|info| info.id.as_str()).collect();
    assert_eq!(ids, ["round-trip", "apply", "base-skin"]);
}

/// A check a caller adds: it raises what it covered and nothing else.
struct Counted(CheckInfo);

impl Check for Counted {
    fn info(&self) -> &CheckInfo {
        &self.0
    }

    fn run(&self, cx: &CheckCx<'_>, sink: &mut reforge::diag::Sink<'_>) {
        sink.raise(Done(Coverage::RoundTrip {
            bins: cx.package.len(),
            objects: 0,
            banks: 0,
        }));
    }
}

#[test]
fn a_check_a_caller_adds_runs_after_the_built_in_ones_under_its_own_id() {
    let schema = schema();
    let info = CheckInfo::built_in("counted", "The chunks, counted", None);
    let set = CheckSet::empty().with(Counted(info));
    let log = checked(&package_of(kept(vec![fixed()])), Some(&schema), &set);
    let [diagnostic] = log.iter().collect::<Vec<_>>()[..] else {
        panic!("one diagnostic");
    };
    assert_eq!(diagnostic.by.as_deref(), Some("counted"));
    assert_eq!(diagnostic.stage, Stage::Check);
}

#[test]
fn a_file_the_base_skin_holds_by_its_hash_is_not_read_for_another_skins_name() {
    let entry = name_hash("Characters/Aatrox/Skins/Skin0");
    let mesh = BinObject::builder(name_hash("mesh"), name_hash("SkinMeshDataProperties"))
        .property(
            name_hash(field::SKELETON),
            values::String::from("ASSETS/Characters/Aatrox/Skins/Skin07/Aatrox_Skin07.skl"),
        )
        .property(
            name_hash(field::TEXTURE),
            values::WadChunkLink::new(path_hash(
                "ASSETS/Characters/Aatrox/Skins/Skin07/Aatrox_Skin07_TX_CM.tex",
            )),
        )
        .build();
    let mesh = values::Embedded(values::Struct {
        class_hash: mesh.class_hash,
        properties: mesh.properties,
    });
    let object = BinObject::builder(entry, SkinEntry::class())
        .property(name_hash(field::SKIN_MESH_PROPERTIES), mesh)
        .property(
            name_hash(field::CHAMPION_SKIN_NAME),
            values::String::from("Aatrox"),
        )
        .build();
    let found = BaseSkin::other_skin(SkinEntry::read(&object), "aatrox");
    let found: Vec<SkinValue> = found.iter().map(|name| name.value).collect();
    assert_eq!(found, [SkinValue::Skeleton]);
}
