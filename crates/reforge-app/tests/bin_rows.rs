//! A bin as the viewer's rows (#ADR-0104): the rows at depth zero, the children of each kind of
//! node by hash path, the pages of a long list, a map that repeats a key, a `PTCH`'s targets
//! and records, and the cache of parsed bins.

use std::sync::Arc;

use ltk_meta::path::{MapKey, PropertyPath, ValuePath, ValueSegment};
use ltk_meta::property::{Kind, values};
use ltk_meta::{BinOverride, PropertyValueEnum};
use reforge::game::{NameBook, Names, Speller};
use reforge::model::{BinHash, ByteView, name_hash};
use reforge::store::{BlobStore, MemoryStore};
use reforge_app::bin_rows::{
    BinDocument, BinFileKind, BinReadError, BinRow, BinRows, BinValue, BinViews, HELD_BINS,
    PropertyKind, READ_PAGE, RowNode, TARGET_PATH,
};
use reforge_app::store::Tiers;
use reforge_testkit::bins::{bin, embedded, float, object, pointer, text};

/// A field no name of the book spells.
const UNNAMED: &str = "secret";

/// How many items the long list holds: past one page.
const LONG: usize = READ_PAGE + 100;

fn i32_value(value: i32) -> PropertyValueEnum {
    PropertyValueEnum::I32(values::I32::new(value))
}

fn string_key(key: &str) -> PropertyValueEnum {
    text(key)
}

/// A bin of one object holding a field of every kind of node the viewer opens.
fn holder_bin() -> Vec<u8> {
    let long = values::Container::new(Kind::I32, (0..LONG as i32).map(i32_value).collect());
    let map = values::Map::new(
        Kind::String,
        Kind::Embedded,
        vec![
            (string_key("a"), embedded("Inner", vec![("x", float(1.0))])),
            (string_key("a"), embedded("Inner", vec![("x", float(2.0))])),
            (
                string_key("x.y}z"),
                embedded("Inner", vec![("x", float(3.0))]),
            ),
        ],
    );
    let leaf_option = values::Optional::new(Kind::F32, Some(float(4.0)));
    let struct_option = values::Optional::new(
        Kind::Embedded,
        Some(embedded("Inner", vec![("x", float(5.0))])),
    );
    let null = values::Struct {
        class_hash: BinHash(0),
        properties: Default::default(),
    };
    bin(vec![object(
        "Root",
        "Holder",
        vec![
            ("name", text("root")),
            ("inner", embedded("Inner", vec![("x", float(6.0))])),
            ("ptr", pointer("Inner", vec![("x", float(7.0))])),
            ("nothing", PropertyValueEnum::Struct(null)),
            ("long", long.unwrap().into()),
            ("map", map.unwrap().into()),
            ("leafOption", leaf_option.unwrap().into()),
            ("structOption", struct_option.unwrap().into()),
            (UNNAMED, float(8.0)),
        ],
    )])
}

/// Every name of [`holder_bin`] but [`UNNAMED`].
fn book() -> NameBook {
    let mut book = NameBook::new();
    book.learn_entry("Root");
    book.learn_entry("Target");
    for class in ["Holder", "Inner"] {
        book.learn_class(class);
    }
    let fields = [
        "name",
        "inner",
        "ptr",
        "nothing",
        "long",
        "map",
        "leafOption",
        "structOption",
        "x",
    ];
    for field in fields {
        book.learn_field(field);
    }
    book
}

/// A bin read with [`book`]'s names: what `read` answers of it.
fn with_names<T>(bytes: &[u8], read: impl FnOnce(&BinDocument, &Speller<'_>) -> T) -> T {
    let (book, names) = (book(), Names::default());
    let document = BinDocument::parse(bytes).unwrap();
    read(&document, &Speller::new(&book, &names))
}

/// The entry of an object, as a row writes it.
fn entry(name: &str) -> BinHash {
    name_hash(name)
}

/// The hash path segment of a field.
fn seg(field: &str) -> String {
    format!("{:08x}", name_hash(field).0)
}

fn children(bytes: &[u8], path: &str) -> BinRows {
    with_names(bytes, |document, speller| {
        document
            .children(entry("Root"), path, 0, READ_PAGE, speller)
            .unwrap()
    })
}

fn row<'a>(rows: &'a BinRows, name: &str) -> &'a BinRow {
    rows.rows
        .iter()
        .find(|row| row.name == name)
        .unwrap_or_else(|| panic!("no row {name} among {:?}", rows.rows))
}

#[test]
fn a_bin_opens_to_one_row_per_object_with_its_class() {
    let handle = with_names(&holder_bin(), |document, speller| document.open(speller));
    assert_eq!(handle.header.kind, BinFileKind::Prop);
    assert_eq!(handle.header.objects, 1);
    let [root] = handle.rows.as_slice() else {
        panic!("one object, not {:?}", handle.rows);
    };
    assert_eq!(root.entry, format!("0x{:08x}", entry("Root").0));
    assert_eq!(
        (root.name.as_str(), root.node, root.path.as_str()),
        ("Root", RowNode::Object, "")
    );
    let BinValue::Struct { class, len, .. } = &root.value else {
        panic!("an object row holds its class, not {:?}", root.value);
    };
    assert_eq!((class.as_deref(), *len), (Some("Holder"), 9));
}

#[test]
fn an_objects_properties_are_rows_by_hash_path_named_by_the_book() {
    let rows = children(&holder_bin(), "");
    assert_eq!(rows.total, 9);
    let name = row(&rows, "name");
    assert_eq!(name.path, seg("name"));
    assert_eq!(name.label, "name");
    assert_eq!(name.kind, Some(PropertyKind::String));
    assert_eq!(
        name.value,
        BinValue::String {
            value: "root".into()
        }
    );
}

#[test]
fn a_field_no_name_spells_is_its_hex_in_the_name_and_the_path() {
    let rows = children(&holder_bin(), "");
    let hex = format!("0x{:08x}", name_hash(UNNAMED).0);
    let unnamed = row(&rows, &hex);
    assert!(unnamed.unnamed);
    assert_eq!(unnamed.path, seg(UNNAMED));
}

#[test]
fn an_embed_and_a_pointer_open_to_their_fields_with_the_readable_path() {
    let bytes = holder_bin();
    for (field, value) in [("inner", 6.0), ("ptr", 7.0)] {
        let rows = children(&bytes, &seg(field));
        let x = row(&rows, "x");
        assert_eq!(x.path, format!("{}.{}", seg(field), seg("x")));
        assert_eq!(x.label, format!("{field}.x"));
        assert_eq!(x.value, BinValue::Float { value });
    }
}

#[test]
fn a_null_pointer_draws_null_and_holds_no_rows() {
    let bytes = holder_bin();
    assert_eq!(row(&children(&bytes, ""), "nothing").value, BinValue::Null);
    assert_eq!(children(&bytes, &seg("nothing")).total, 0);
}

#[test]
fn a_list_past_a_page_answers_a_page_at_a_time() {
    let bytes = holder_bin();
    let long = seg("long");
    let page = |offset| {
        with_names(&bytes, |document, speller| {
            document
                .children(entry("Root"), &long, offset, READ_PAGE, speller)
                .unwrap()
        })
    };
    let first = page(0);
    assert_eq!((first.rows.len(), first.total), (READ_PAGE, LONG));
    let second = page(READ_PAGE);
    assert_eq!(second.rows.len(), LONG - READ_PAGE);
    let last = second.rows.last().unwrap();
    assert_eq!(last.path, format!("{long}[{}]", LONG - 1));
    assert_eq!(
        (last.name.as_str(), last.node),
        (&*format!("[{}]", LONG - 1), RowNode::Element)
    );
}

#[test]
fn a_repeated_map_key_takes_its_count_and_reaches_its_own_entry() {
    let bytes = holder_bin();
    let map = seg("map");
    let rows = children(&bytes, &map);
    let paths: Vec<&str> = rows.rows.iter().map(|row| row.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            format!(r#"{map}{{"a"}}"#),
            format!(r#"{map}{{"a"}}#1"#),
            format!(r#"{map}{{"x.y}}z"}}"#),
        ]
    );
    let second = children(&bytes, &format!(r#"{map}{{"a"}}#1"#));
    assert_eq!(row(&second, "x").value, BinValue::Float { value: 2.0 });
    assert_eq!(row(&second, "x").label, r#"map{"a"}.x"#);
}

#[test]
fn a_place_written_as_the_type_check_writes_it_is_a_rows_path() {
    let place: ValuePath = [
        ValueSegment::Field(name_hash("map")),
        ValueSegment::Key(MapKey::String("x.y}z".into())),
        ValueSegment::Field(name_hash("x")),
    ]
    .into_iter()
    .collect();
    let holder = format!(r#"{}{{"x.y}}z"}}"#, seg("map"));
    let rows = children(&holder_bin(), &holder);
    assert_eq!(row(&rows, "x").path, place.to_string());
}

#[test]
fn a_string_key_holding_a_dot_and_a_brace_is_one_step() {
    let bytes = holder_bin();
    let rows = children(&bytes, &format!(r#"{}{{"x.y}}z"}}"#, seg("map")));
    assert_eq!(row(&rows, "x").value, BinValue::Float { value: 3.0 });
}

#[test]
fn an_option_draws_the_leaf_it_holds_and_opens_to_the_struct_it_holds() {
    let bytes = holder_bin();
    let rows = children(&bytes, "");
    assert_eq!(
        row(&rows, "leafOption").value,
        BinValue::Float { value: 4.0 }
    );
    assert_eq!(children(&bytes, &seg("leafOption")).total, 0);
    let held = children(&bytes, &seg("structOption"));
    assert_eq!(held.rows[0].path, format!("{}[0]", seg("structOption")));
}

#[test]
fn a_path_that_reaches_nothing_is_refused_alone_and_empty_in_a_read() {
    let bytes = holder_bin();
    let nowhere = format!("{}.{}", seg("inner"), seg("nowhere"));
    with_names(&bytes, |document, speller| {
        let alone = document.children(entry("Root"), &nowhere, 0, READ_PAGE, speller);
        assert!(matches!(alone, Err(BinReadError::NodeNotFound(_))));
        let read = document
            .children_each(entry("Root"), &[nowhere.clone(), seg("inner")], speller)
            .unwrap();
        assert_eq!((read[0].total, read[1].total), (0, 1));
    });
}

#[test]
fn a_read_past_four_pages_is_refused() {
    let bytes = holder_bin();
    let paths = vec![seg("long"); 5];
    with_names(&bytes, |document, speller| {
        let read = document.children_each(entry("Root"), &paths, speller);
        assert!(matches!(read, Err(BinReadError::ReadTooWide { .. })));
    });
}

/// A `PTCH` adding one object and writing two records to another.
fn patch_bin() -> Vec<u8> {
    let patch = BinOverride::builder()
        .object(object("Root", "Holder", vec![("name", text("added"))]))
        .set(
            entry("Target"),
            PropertyPath::new("inner").unwrap(),
            embedded("Inner", vec![("x", float(9.0))]),
        )
        .set(
            entry("Target"),
            PropertyPath::new("name").unwrap(),
            text("patched"),
        )
        .build();
    let mut out = std::io::Cursor::new(Vec::new());
    patch.to_writer(&mut out).unwrap();
    out.into_inner()
}

#[test]
fn a_patch_opens_to_its_objects_then_a_target_per_patched_object() {
    let handle = with_names(&patch_bin(), |document, speller| document.open(speller));
    assert_eq!(handle.header.kind, BinFileKind::Patch);
    assert_eq!(handle.header.patches, 2);
    let nodes: Vec<(&str, RowNode)> = handle
        .rows
        .iter()
        .map(|row| (row.name.as_str(), row.node))
        .collect();
    assert_eq!(
        nodes,
        [("Root", RowNode::Object), ("Target", RowNode::Target)]
    );
    assert_eq!(handle.rows[1].path, TARGET_PATH);
    assert_eq!(handle.rows[1].value, BinValue::Records { len: 2 });
}

#[test]
fn a_target_opens_to_its_records_and_a_record_to_what_its_value_holds() {
    with_names(&patch_bin(), |document, speller| {
        let records = document
            .children(entry("Target"), TARGET_PATH, 0, READ_PAGE, speller)
            .unwrap();
        let names: Vec<(&str, &str)> = records
            .rows
            .iter()
            .map(|row| (row.name.as_str(), row.path.as_str()))
            .collect();
        assert_eq!(names, [("inner", "#0"), ("name", "#1")]);
        assert_eq!(records.rows[0].node, RowNode::Record);

        let under = document
            .children(entry("Target"), "#0", 0, READ_PAGE, speller)
            .unwrap();
        let x = &under.rows[0];
        assert_eq!(
            (x.path.as_str(), x.label.as_str()),
            (&*format!("#0.{}", seg("x")), "inner.x")
        );
        assert_eq!(x.value, BinValue::Float { value: 9.0 });
    });
}

#[test]
fn a_bin_viewed_again_is_the_one_held_until_others_push_it_out() {
    let layer = MemoryStore::new();
    let first = layer.put(ByteView::from(holder_bin()));
    let tiers = Tiers::layered(&layer, None);
    let views = BinViews::default();
    let held = views.document(first, &tiers).unwrap();
    assert!(Arc::ptr_eq(&held, &views.document(first, &tiers).unwrap()));
    for at in 0..HELD_BINS {
        let other = object(&format!("Other{at}"), "Holder", vec![]);
        let content = layer.put(ByteView::from(bin(vec![other])));
        views.document(content, &tiers).unwrap();
    }
    assert!(!Arc::ptr_eq(&held, &views.document(first, &tiers).unwrap()));
}
