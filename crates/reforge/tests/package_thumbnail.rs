//! The thumbnail (#ADR-0051): a mod's `META/image.png` encoded again as WebP in the package
//! a run builds, in the project and in the `.modpkg`, and the note when it is not carried.

use std::io::Cursor;

use camino::Utf8PathBuf;
use fs_err as fs;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use ltk_modpkg::Modpkg as Packed;
use reforge::diag::DiagnosticPolicy;
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::ContentId;
use reforge::model::entity::{MAX_IMAGE_BYTES, ModImage};
use reforge::package::{
    Modpkg, PackageWriter, ProjectDir, THUMBNAIL_FILE, Thumbnail, ThumbnailDropped,
    ThumbnailRefusal,
};
use reforge::{Porter, Repairer, Run};
use reforge_testkit::bins::{bin, entry};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::mods::FILE_NAME;
use reforge_testkit::skins::{self, skin_bin};
use serde_json::Value;

const IMAGE: &str = "META/image.png";
const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

/// A picture of two colours, 3 by 2, in a format.
fn picture(format: ImageFormat) -> Vec<u8> {
    let image = RgbImage::from_fn(3, 2, |x, _| Rgb([if x == 0 { 255 } else { 0 }, 64, 128]));
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut bytes), format)
        .expect("the picture encodes");
    bytes
}

/// A mod of its own rig at skin0, with an image when given one.
fn mod_with(image: Option<Vec<u8>>) -> Vec<u8> {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
    ];
    let mut files = own_files("a");
    files.push((skin_bin("test", 0), bin(objects)));
    let archive = archive(&files);
    match image {
        Some(image) => archive.extra(IMAGE, image).build(),
        None => archive.build(),
    }
}

fn game_env() -> Env {
    env(skins::game(), NoHistory)
}

fn scratch() -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("a UTF-8 path");
    (dir, path)
}

/// The size of a WebP's picture.
fn webp_size(bytes: &[u8]) -> (u32, u32) {
    let image = image::load_from_memory_with_format(bytes, ImageFormat::WebP);
    let image = image.expect("the thumbnail is a WebP");
    (image.width(), image.height())
}

/// The one refusal a run raised.
fn refusal(run: &Run<'_>) -> ThumbnailRefusal {
    let raised: Vec<&ThumbnailDropped> = run.log().of::<ThumbnailDropped>().collect();
    let [dropped] = raised[..] else {
        panic!("{} refusals", raised.len());
    };
    dropped.why
}

#[test]
fn a_mods_image_is_the_thumbnail_of_the_project_and_of_the_modpkg() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    let png = picture(ImageFormat::Png);
    let mut run = repairer
        .repair(&mod_with(Some(png.clone())), FILE_NAME)
        .expect("no fault");
    assert_eq!(run.log().of::<ThumbnailDropped>().count(), 0);

    let package = run.package().expect("a package");
    let webp = package.thumbnail.as_ref().expect("a thumbnail").bytes();
    let webp = webp.to_vec();
    assert_ne!(webp, png, "encoded again, not the archive's bytes");
    assert_eq!(webp_size(&webp), (3, 2));
    let row = run.report().package.expect("the package's row");
    assert_eq!(row.thumbnail, Some(ContentId::of(&webp)));

    let (_dir, out) = scratch();
    let (project, packed) = (ProjectDir::new(&out), Modpkg::new(&out));
    let writers: [&dyn PackageWriter; 2] = [&project, &packed];
    run.write(&writers, &DiagnosticPolicy::default())
        .expect("the package writes");
    let package = run.package().expect("a package");

    let root = project.path_of(package);
    assert_eq!(fs::read(root.join(THUMBNAIL_FILE)).expect("the file"), webp);
    let config = fs::read_to_string(root.join("mod.config.json")).expect("the config");
    let config: Value = serde_json::from_str(&config).expect("the config is JSON");
    assert_eq!(config["thumbnail"], THUMBNAIL_FILE);

    let bytes = fs::read(packed.path_of(package)).expect("the package file");
    let mut packed = Packed::mount_from_reader(Cursor::new(bytes)).expect("the package mounts");
    let held = packed
        .load_chunk_decompressed_by_path("_meta_/thumbnail.webp", None)
        .expect("the package holds a thumbnail");
    assert_eq!(held.to_vec(), webp);
}

#[test]
fn a_port_carries_the_image_as_a_repair_does() {
    let env = game_env();
    let porter = Porter::new(&env);
    let image = Some(picture(ImageFormat::Png));
    let run = porter.port(&mod_with(image), FILE_NAME).expect("no fault");
    let package = run.package().expect("a package");
    assert!(package.thumbnail.is_some());
}

#[test]
fn a_jpeg_or_a_webp_under_the_png_name_is_read_as_what_it_is() {
    for format in [ImageFormat::Jpeg, ImageFormat::WebP] {
        let image = ModImage::Held(picture(format).into());
        let thumbnail = Thumbnail::of(&image).expect("a thumbnail");
        assert_eq!(webp_size(thumbnail.bytes()), (3, 2), "{format:?}");
    }
}

#[test]
fn the_same_image_gives_the_same_thumbnail() {
    let image = ModImage::Held(picture(ImageFormat::Png).into());
    let first = Thumbnail::of(&image).expect("a thumbnail");
    assert_eq!(Thumbnail::of(&image), Ok(first));
}

#[test]
fn a_mod_with_no_image_has_no_thumbnail_and_raises_nothing() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    let mut run = repairer
        .repair(&mod_with(None), FILE_NAME)
        .expect("no fault");
    assert!(run.package().expect("a package").thumbnail.is_none());
    assert_eq!(run.log().of::<ThumbnailDropped>().count(), 0);
    assert_eq!(run.report().package.expect("the row").thumbnail, None);

    let (_dir, out) = scratch();
    let project = ProjectDir::new(&out);
    let writers: [&dyn PackageWriter; 1] = [&project];
    run.write(&writers, &DiagnosticPolicy::default())
        .expect("the package writes");
    let root = project.path_of(run.package().expect("a package"));
    assert!(!root.join(THUMBNAIL_FILE).exists());
    let config = fs::read_to_string(root.join("mod.config.json")).expect("the config");
    assert!(!config.contains("thumbnail"), "{config}");
}

#[test]
fn an_image_that_does_not_decode_is_dropped_with_a_note_and_the_package_writes() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    // A PNG's signature and nothing a decoder reads after it.
    let broken = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    let mut run = repairer
        .repair(&mod_with(Some(broken)), FILE_NAME)
        .expect("no fault");
    assert_eq!(run.halted(), None);
    assert_eq!(refusal(&run), ThumbnailRefusal::NotAnImage);
    assert!(run.package().expect("a package").thumbnail.is_none());

    let (_dir, out) = scratch();
    let packed = Modpkg::new(&out);
    let writers: [&dyn PackageWriter; 1] = [&packed];
    run.write(&writers, &DiagnosticPolicy::default())
        .expect("the package writes");
    let bytes = fs::read(packed.path_of(run.package().expect("a package"))).expect("the file");
    let mut packed = Packed::mount_from_reader(Cursor::new(bytes)).expect("the package mounts");
    assert!(
        packed
            .load_chunk_decompressed_by_path("_meta_/thumbnail.webp", None)
            .is_err()
    );
}

#[test]
fn an_image_the_run_does_not_hold_is_dropped_with_why() {
    let env = game_env();
    let repairer = Repairer::new(&env);
    let large = vec![0u8; MAX_IMAGE_BYTES as usize + 1];
    let run = repairer
        .repair(&mod_with(Some(large)), FILE_NAME)
        .expect("no fault");
    let bytes = MAX_IMAGE_BYTES + 1;
    assert_eq!(refusal(&run), ThumbnailRefusal::EntryTooLarge { bytes });

    assert_eq!(
        Thumbnail::of(&ModImage::Unreadable),
        Err(ThumbnailRefusal::EntryUnreadable)
    );
}

#[test]
fn an_image_wider_than_the_limit_is_not_decoded() {
    // A PNG header that declares 100,000 by 1: the decoder refuses it by its size.
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = b"IHDR".to_vec();
    header.extend(100_000u32.to_be_bytes());
    header.extend(1u32.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    png.extend(13u32.to_be_bytes());
    png.extend(&header);
    png.extend(crc32(&header).to_be_bytes());
    assert_eq!(
        Thumbnail::of(&ModImage::Held(png.into())),
        Err(ThumbnailRefusal::NotAnImage)
    );
}

/// The CRC a PNG chunk ends with.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}
