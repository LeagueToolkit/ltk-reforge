//! An archive read alone (#ADR-0091): its metadata, the characters its paths name with their
//! slots, and the package's thumbnail (#ADR-0051), with no game and no history.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, ImageReader, Rgb, RgbImage};
use reforge::archive::{Caps, NotZip};
use reforge::diag::Render;
use reforge::model::hash::skin_paths;
use reforge::model::{Slot, path_hash};
use reforge::package::ThumbnailRefusal;
use reforge::peek::{PeekCharacter, peek};
use reforge_testkit::ModArchive;
use reforge_testkit::bins::blob;
use reforge_testkit::mods::FILE_NAME;

const IMAGE: &str = "META/image.png";

/// A picture of two colours, 3 by 2, as a PNG.
fn picture() -> Vec<u8> {
    let image = RgbImage::from_fn(3, 2, |x, _| Rgb([if x == 0 { 255 } else { 0 }, 64, 128]));
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("the picture encodes");
    bytes
}

/// A champion's WAD with a spelled skin bin, one named by its hash, and a sub-character's.
fn lux_mod() -> ModArchive {
    let hashed = path_hash(&skin_paths::skin_bin("lux", Slot(7)));
    ModArchive::new()
        .file(
            "Lux.wad.client",
            &skin_paths::skin_bin("lux", Slot(5)),
            blob("a"),
        )
        .file(
            "Lux.wad.client",
            &format!("{:016x}.bin", hashed.0),
            blob("b"),
        )
        .file(
            "Lux.wad.client",
            &skin_paths::skin_bin("luxair", Slot(5)),
            blob("c"),
        )
        .file("Lux.wad.client", "assets/lux/x.tex", blob("x"))
}

fn slots(numbers: &[u16]) -> Vec<Slot> {
    numbers.iter().copied().map(Slot).collect()
}

#[test]
fn a_peek_names_the_mod_and_each_character_with_the_slots_its_paths_and_hashes_ship() {
    let bytes = lux_mod().build();
    let peek = peek(&bytes, FILE_NAME, &Caps::default()).unwrap();
    assert_eq!(peek.meta.name.unwrap().raw(), "Synthetic");
    assert_eq!(peek.meta.author.unwrap().raw(), "reforge tests");
    assert_eq!(peek.meta.file_name.raw(), FILE_NAME);
    assert_eq!(peek.meta.mod_id.len(), 12);
    assert_eq!(
        peek.characters,
        vec![
            PeekCharacter {
                name: "lux".to_string(),
                slots: slots(&[5, 7]),
            },
            PeekCharacter {
                name: "luxair".to_string(),
                slots: slots(&[5]),
            },
        ]
    );
    assert!(peek.thumbnail.is_none());
    assert!(peek.image_refused.is_none());
}

#[test]
fn a_wad_with_no_skin_bin_names_no_character() {
    let bytes = ModArchive::new()
        .file("Other.wad.client", "assets/other/x.tex", blob("x"))
        .build();
    let peek = peek(&bytes, FILE_NAME, &Caps::default()).unwrap();
    assert!(peek.characters.is_empty());
}

#[test]
fn the_thumbnail_is_the_image_encoded_as_webp() {
    let bytes = lux_mod().extra(IMAGE, picture()).build();
    let peek = peek(&bytes, FILE_NAME, &Caps::default()).unwrap();
    let thumbnail = peek.thumbnail.expect("the image is carried");
    assert_ne!(thumbnail.bytes(), picture().as_slice());
    let reader = ImageReader::with_format(Cursor::new(thumbnail.bytes()), ImageFormat::WebP);
    let decoded = reader.decode().expect("a WebP");
    assert_eq!((decoded.width(), decoded.height()), (3, 2));
}

#[test]
fn an_image_that_is_no_image_is_refused_and_the_rest_is_read() {
    let bytes = lux_mod().extra(IMAGE, b"not a picture".to_vec()).build();
    let peek = peek(&bytes, FILE_NAME, &Caps::default()).unwrap();
    assert!(peek.thumbnail.is_none());
    assert_eq!(peek.image_refused, Some(ThumbnailRefusal::NotAnImage));
    assert_eq!(peek.characters.len(), 2);
}

#[test]
fn bytes_that_are_no_zip_give_the_halting_diagnostic_and_its_log() {
    let not_read = peek(b"not a zip", FILE_NAME, &Caps::default()).unwrap_err();
    let halting = not_read
        .log
        .get(not_read.halted.0)
        .expect("the log holds it");
    assert!(halting.downcast_ref::<NotZip>().is_some());
    assert!(!halting.message(&Render::plain()).is_empty());
}
