//! A mod read alone when it is picked: its metadata and characters through `clean`, its
//! thumbnail as the package's WebP (#ADR-0051), and a mod that does not read.

use std::io::Cursor;

use camino::Utf8PathBuf;
use fs_err as fs;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use reforge::model::hash::skin_paths;
use reforge::model::{Slot, path_hash};
use reforge_app::error::AppError;
use reforge_app::run::{CharacterPeek, peek};
use reforge_testkit::archive::zip;
use reforge_testkit::bins::blob;
use tempfile::TempDir;
use zip::CompressionMethod;

/// A picture of one colour, 2 by 2, as a PNG.
fn picture() -> Vec<u8> {
    let image = RgbImage::from_fn(2, 2, |_, _| Rgb([255, 64, 128]));
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("the picture encodes");
    bytes
}

const INFO: &[u8] = br#"{"Name":"Lux\u001b[31m","Author":"a"}"#;

/// A champion's WAD with a spelled skin bin and one named by its hash, an info.json whose
/// name holds a control character, and an image when given one.
fn lux_mod(image: Option<Vec<u8>>) -> Vec<u8> {
    let hashed = format!(
        "WAD/Lux.wad.client/{:016x}.bin",
        path_hash(&skin_paths::skin_bin("lux", Slot(7))).0
    );
    let spelled = format!(
        "WAD/Lux.wad.client/{}",
        skin_paths::skin_bin("lux", Slot(5))
    );
    let (a, b) = (blob("a"), blob("b"));
    let mut entries: Vec<(&str, &[u8], CompressionMethod)> = vec![
        ("META/info.json", INFO, CompressionMethod::Stored),
        (&spelled, &a, CompressionMethod::Stored),
        (&hashed, &b, CompressionMethod::Stored),
    ];
    if let Some(image) = &image {
        entries.push(("META/image.png", image, CompressionMethod::Stored));
    }
    zip(&entries)
}

fn written(dir: &TempDir, name: &str, bytes: &[u8]) -> Utf8PathBuf {
    let path = Utf8PathBuf::from_path_buf(dir.path().join(name)).unwrap();
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn a_peek_names_the_mod_through_clean_with_its_characters_and_thumbnail() {
    let dir = tempfile::tempdir().unwrap();
    let path = written(&dir, "lux.fantome", &lux_mod(Some(picture())));
    let peek = peek(&path).unwrap();
    assert_eq!(peek.meta.name.as_deref(), Some("Lux\\u{1b}[31m"));
    assert_eq!(peek.meta.author.as_deref(), Some("a"));
    assert_eq!(peek.meta.file_name, "lux.fantome");
    assert_eq!(
        peek.characters,
        vec![CharacterPeek {
            name: "lux".to_string(),
            slots: vec![5, 7],
        }]
    );
    let thumbnail = peek.thumbnail.expect("the image is carried");
    assert!(thumbnail.starts_with("data:image/webp;base64,"));
}

#[test]
fn a_mod_with_no_image_has_no_thumbnail() {
    let dir = tempfile::tempdir().unwrap();
    let path = written(&dir, "lux.fantome", &lux_mod(None));
    assert!(peek(&path).unwrap().thumbnail.is_none());
}

#[test]
fn a_file_that_is_no_archive_answers_mod_unreadable_with_the_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    let path = written(&dir, "not.fantome", b"not a zip");
    match peek(&path).unwrap_err() {
        AppError::ModUnreadable { path: at, detail } => {
            assert_eq!(at, path.as_str());
            assert!(!detail.is_empty());
        }
        other => panic!("answered {other:?}"),
    }
}
