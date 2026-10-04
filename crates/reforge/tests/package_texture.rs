//! A DDS put in TEX's container: the blocks as they are, the mips in TEX's order, and
//! everything TEX does not hold as it is refused (#ADR-0022).

use reforge::package::{TexRefusal, dds_to_tex};
use reforge_testkit::packs::{DdsPixels, dds};

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
