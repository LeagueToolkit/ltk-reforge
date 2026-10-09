//! Bins read from bytes: the objects in file order and the two digests of each.

use ltk_meta::property::values;
use ltk_meta::{Bin, BinObject};
use reforge::model::bin::{self, BinError};
use reforge::model::{ContentId, name_hash};

fn object(entry: &str, skin_name: &str) -> BinObject {
    BinObject::builder(name_hash(entry), name_hash("SkinCharacterDataProperties"))
        .property(
            name_hash("championSkinName"),
            values::String::from(skin_name),
        )
        .build()
}

fn bytes(objects: Vec<BinObject>, dependencies: &[&str]) -> Vec<u8> {
    let mut built = Bin::builder();
    for dependency in dependencies {
        built = built.dependency(*dependency);
    }
    for object in objects {
        built = built.object(object);
    }
    bin::write(&built.build())
}

#[test]
fn a_bin_reads_back_with_its_objects_and_links() {
    let bytes = bytes(
        vec![
            object("Characters/Lux/Skins/Skin0", "Lux"),
            object("Characters/Lux/Skins/Skin7", "LuxSkin07"),
        ],
        &["DATA/Characters/Lux/Lux.bin"],
    );
    let parsed = bin::parse(&bytes).unwrap();
    assert_eq!(parsed.dependencies, ["DATA/Characters/Lux/Lux.bin"]);
    assert_eq!(parsed.objects.len(), 2);
    assert_eq!(
        parsed.objects[0].value.path_hash,
        name_hash("Characters/Lux/Skins/Skin0")
    );
    assert!(parsed.duplicates.is_empty());
}

#[test]
fn the_same_object_under_another_name_has_the_same_body_and_another_raw_digest() {
    let a = bin::parse(&bytes(
        vec![object("Characters/Lux/Skins/Skin0", "Lux")],
        &[],
    ))
    .unwrap();
    let b = bin::parse(&bytes(
        vec![object("Characters/Lux/Skins/Skin7", "Lux")],
        &[],
    ))
    .unwrap();
    let c = bin::parse(&bytes(
        vec![object("Characters/Lux/Skins/Skin0", "Other")],
        &[],
    ))
    .unwrap();
    assert_eq!(a.objects[0].body, b.objects[0].body);
    assert_ne!(a.objects[0].raw, b.objects[0].raw);
    assert_ne!(a.objects[0].body, c.objects[0].body);
}

#[test]
fn the_raw_digest_is_of_the_bytes_from_the_entry_hash_to_the_objects_end() {
    let bytes = bytes(vec![object("Characters/Lux/Skins/Skin0", "Lux")], &[]);
    let parsed = bin::parse(&bytes).unwrap();
    let entry = name_hash("Characters/Lux/Skins/Skin0").0.to_le_bytes();
    let at = bytes
        .windows(4)
        .rposition(|w| w == entry)
        .expect("the entry hash is in the bytes");
    assert_eq!(parsed.objects[0].raw, ContentId::of(&bytes[at..]));
    assert_eq!(parsed.objects[0].body, ContentId::of(&bytes[at + 4..]));
}

#[test]
fn a_hash_listed_twice_keeps_the_first_object_listed() {
    // Two objects of one size, the second renamed to the first's entry hash in the bytes.
    let (first, second) = ("Characters/Lux/Skins/Skin0", "Characters/Lux/Skins/Skin7");
    let alone = bin::parse(&bytes(vec![object(first, "Lux")], &[])).unwrap();
    let mut twice = bytes(vec![object(first, "Lux"), object(second, "Pix")], &[]);
    let at = twice
        .windows(4)
        .position(|w| w == name_hash(second).0.to_le_bytes())
        .expect("the second entry hash is in the bytes");
    twice[at..at + 4].copy_from_slice(&name_hash(first).0.to_le_bytes());

    let parsed = bin::parse(&twice).unwrap();
    // #GAME-BEHAVIOR-0020: the client keeps the first.
    assert_eq!(parsed.duplicates, [name_hash(first)]);
    let [kept] = &parsed.objects[..] else {
        panic!("one object: {:?}", parsed.objects);
    };
    assert_eq!(kept.value, alone.objects[0].value);
    assert_eq!(kept.raw, alone.objects[0].raw);
}

#[test]
fn bytes_that_are_no_bin_are_refused_with_the_reason() {
    assert_eq!(bin::parse(b"PTCH\x01\0\0\0").unwrap_err(), BinError::Patch);
    assert_eq!(bin::parse(b"DDS ").unwrap_err(), BinError::NotABin);
    assert_eq!(bin::parse(b"").unwrap_err(), BinError::NotABin);
}

#[test]
fn every_object_the_table_lists_is_read_or_the_bin_is_refused() {
    // The reader sweeps the table whole, so a bin that reads has an object per row. A count
    // that differs is a refusal with both counts, not a bin missing its last objects.
    let objects: Vec<BinObject> = (0..5)
        .map(|i| object(&format!("Characters/Lux/Skins/Skin{i}"), "Lux"))
        .collect();
    let parsed = bin::parse(&bytes(objects, &[])).unwrap();
    assert_eq!(parsed.objects.len() + parsed.duplicates.len(), 5);
    let refused = BinError::ObjectCount { listed: 5, read: 4 };
    assert_eq!(
        refused.to_string(),
        "the table lists 5 objects and 4 were read"
    );
}

#[test]
fn every_prefix_of_a_bin_reads_or_is_refused_and_never_panics() {
    let bytes = bytes(
        vec![
            object("Characters/Lux/Skins/Skin0", "Lux"),
            object("Characters/Lux/Skins/Skin7", "LuxSkin07"),
        ],
        &["DATA/Characters/Lux/Lux.bin"],
    );
    for end in 0..bytes.len() {
        assert!(
            bin::parse(&bytes[..end]).is_err(),
            "a bin cut at {end} of {} read",
            bytes.len()
        );
    }
    assert!(bin::parse(&bytes).is_ok());
}
