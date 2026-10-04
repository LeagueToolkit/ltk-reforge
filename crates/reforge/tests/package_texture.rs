//! A DDS put in TEX's container: the blocks as they are, the mips in TEX's order, and
//! everything TEX does not hold as it is refused (#ADR-0022). A run that keeps DDS puts none
//! in (#ADR-0055).

use reforge::history::NoHistory;
use reforge::package::{TexRefusal, TextureContained, dds_to_tex};
use reforge::{Porter, Repairer, Run};
use reforge_testkit::bins::{bin, entry, particle, resolver_with};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::mods::FILE_NAME;
use reforge_testkit::packs::{DdsPixels, dds};
use reforge_testkit::skins::{self, skin_bin};

/// The bytes of a DDS after its header.
fn data(dds: &[u8]) -> &[u8] {
    &dds[128..]
}

#[test]
fn one_mip_of_whole_blocks_goes_in_as_it_is() {
    let source = dds(DdsPixels::Dxt1, (8, 4), 1);
    let tex = dds_to_tex(&source).expect("TEX holds BC1");
    // The magic, the width and the height, then a depth of one slice, the format, the
    // resource type and the flags: BC1 is format 10, and a texture with one mip has no flag.
    assert_eq!(&tex[..12], b"TEX\0\x08\0\x04\0\x01\x0a\0\0");
    assert_eq!(&tex[12..], data(&source));
    let back = ltk_texture::Tex::from_reader(&mut tex.as_slice()).expect("the TEX reads");
    assert_eq!((back.width, back.height, back.mip_count), (8, 4, 1));
}

#[test]
fn a_whole_mip_chain_is_stored_smallest_first() {
    let source = dds(DdsPixels::Dxt5, (8, 8), 4);
    let tex = dds_to_tex(&source).expect("TEX holds BC3");
    // BC3 is format 12, and the flag says the texture has its mips.
    assert_eq!(&tex[8..12], b"\x01\x0c\0\x01");
    // 8, 4, 2 and 1 pixels a side are 4, 1, 1 and 1 blocks of 16 bytes. The test's DDS fills
    // mip `n` with the byte `n + 1`.
    let mut expected = Vec::new();
    for (level, blocks) in [(4u8, 1), (3, 1), (2, 1), (1, 4)] {
        expected.extend(std::iter::repeat_n(level, blocks * 16));
    }
    assert_eq!(&tex[12..], expected.as_slice());
    assert_eq!(tex.len() - 12, data(&source).len());
}

#[test]
fn pixels_of_another_byte_order_are_swapped_into_bgra() {
    let as_bgra = dds(DdsPixels::Bgra8, (4, 4), 1);
    let tex = dds_to_tex(&as_bgra).expect("TEX holds BGRA8");
    assert_eq!(&tex[12..], data(&as_bgra));

    // The test's DDS holds the bytes 0, 1, 2, 3 in each pixel: R, G, B, A here.
    let as_rgba = dds(DdsPixels::Rgba8, (4, 4), 1);
    let tex = dds_to_tex(&as_rgba).expect("TEX holds RGBA8 as BGRA8");
    assert_eq!(tex.len(), 12 + 64);
    let (pixels, rest) = tex[12..].as_chunks::<4>();
    assert!(rest.is_empty() && pixels.iter().all(|pixel| *pixel == [2, 1, 0, 3]));
}

#[test]
fn what_tex_does_not_hold_as_it_is_is_refused() {
    let refused = |bytes: &[u8]| dds_to_tex(bytes).expect_err("TEX does not hold it");
    assert_eq!(
        refused(&dds(DdsPixels::Dxt3, (8, 8), 1)),
        TexRefusal::Format
    );
    assert_eq!(
        refused(&dds(DdsPixels::Dxt1, (6, 8), 1)),
        TexRefusal::PartialBlocks
    );
    assert_eq!(
        refused(&dds(DdsPixels::Dxt1, (8, 8), 2)),
        TexRefusal::PartialMips
    );
    let whole = dds(DdsPixels::Dxt1, (8, 8), 4);
    assert_eq!(refused(&whole[..whole.len() - 1]), TexRefusal::Truncated);
    assert_eq!(refused(b"DDS and no header"), TexRefusal::Unreadable);
    assert_eq!(refused(b""), TexRefusal::Unreadable);
}

/// Sets one 32-bit word of a DDS header, counted from the header's size.
fn set_word(dds: &mut [u8], word: usize, value: u32) {
    let at = 4 + word * 4;
    dds[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// The header words of a DDS: its flags, its depth and its second set of capabilities.
const FLAGS: usize = 1;
const DEPTH: usize = 5;
const CAPS2: usize = 27;

#[test]
fn a_cube_map_and_a_volume_are_not_one_surface() {
    let flat = dds(DdsPixels::Dxt1, (8, 8), 1);
    dds_to_tex(&flat).expect("TEX holds one surface");

    // A cube map: the flag and its six faces.
    let mut cube = flat.clone();
    set_word(&mut cube, CAPS2, 0x200 | 0xfc00);
    assert_eq!(dds_to_tex(&cube), Err(TexRefusal::NotOneSurface));

    // A volume two slices deep: the header says it has a depth, and holds it.
    let mut volume = flat.clone();
    let flags = u32::from_le_bytes([volume[8], volume[9], volume[10], volume[11]]);
    set_word(&mut volume, FLAGS, flags | 0x80_0000);
    set_word(&mut volume, DEPTH, 2);
    set_word(&mut volume, CAPS2, 0x20_0000);
    assert_eq!(dds_to_tex(&volume), Err(TexRefusal::NotOneSurface));
}

#[test]
fn a_side_longer_than_sixteen_bits_count_is_too_large() {
    // TEX's header holds each side in two bytes.
    let longest = dds(DdsPixels::Dxt1, (65_532, 4), 1);
    let tex = dds_to_tex(&longest).expect("TEX holds a side of 65,532 pixels");
    assert_eq!(&tex[4..8], b"\xfc\xff\x04\0");
    let wide = dds(DdsPixels::Dxt1, (65_536, 4), 1);
    assert_eq!(dds_to_tex(&wide), Err(TexRefusal::TooLarge));
    let tall = dds(DdsPixels::Dxt1, (4, 65_536), 1);
    assert_eq!(dds_to_tex(&tall), Err(TexRefusal::TooLarge));
}

/// A mod of its own rig at skin0 whose particle names a texture at a `.tex` path, held as
/// these bytes.
fn mod_with_texture(texture: Vec<u8>) -> Vec<u8> {
    let skin = "Characters/Test/Skins/Skin0";
    let graph = "Characters/Test/Animations/Skin0";
    let resources = "Characters/Test/Skins/Skin0/Resources";
    let fx = "Characters/Test/Skins/Skin0/Particles/Test_Own";
    let path = "assets/mod/a/own.tex";
    let objects = vec![
        entry(skin, &own_skin("a", graph, resources)),
        own_graph("a", graph),
        resolver_with(resources, &[("Test_Q_Cast", fx)]),
        particle(fx, path),
    ];
    let mut files = own_files("a");
    files.push((path.into(), texture));
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files).build()
}

/// The chunks of a run's package that start with these bytes.
fn chunks_starting(run: &Run<'_>, magic: &[u8]) -> usize {
    let package = run.package().expect("a package");
    let chunks = package
        .chunks()
        .filter(|(_, _, chunk)| chunk.bytes.starts_with(magic));
    chunks.count()
}

#[test]
fn a_run_that_keeps_dds_ships_a_dds_at_a_tex_path_as_it_is() {
    // #ADR-0055
    let env = env(skins::game(), NoHistory);
    let texture = dds(DdsPixels::Dxt1, (8, 8), 4);
    let bytes = mod_with_texture(texture.clone());
    let porter = Porter::new(&env);
    let repairer = Repairer::new(&env);
    let runs = [
        porter.port(&bytes, FILE_NAME).expect("no fault"),
        repairer.repair(&bytes, FILE_NAME).expect("no fault"),
    ];
    for run in &runs {
        let shipped = (
            chunks_starting(run, b"TEX\0"),
            chunks_starting(run, b"DDS "),
        );
        assert_eq!(shipped, (1, 0));
    }

    let porter = Porter::new(&env).keep_dds(true);
    let repairer = Repairer::new(&env).keep_dds(true);
    let runs = [
        porter.port(&bytes, FILE_NAME).expect("no fault"),
        repairer.repair(&bytes, FILE_NAME).expect("no fault"),
    ];
    for run in &runs {
        let package = run.package().expect("a package");
        let shipped = package
            .chunks()
            .filter(|(_, _, chunk)| *chunk.bytes == *texture);
        assert_eq!(shipped.count(), 1);
        assert_eq!(chunks_starting(run, b"TEX\0"), 0);
        let found: Vec<_> = run.log().of::<TextureContained>().collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].contained, 0);
        let kept: Vec<_> = found[0].kept.iter().collect();
        assert_eq!(kept, [(&TexRefusal::Asked, &1)]);
    }
}
