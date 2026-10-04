//! A DDS at a `.tex` path, put in TEX's container with its blocks as they are
//! (#GAME-CHANGE-0007, #ADR-0022). The client reads either container by its magic
//! (#GAME-BEHAVIOR-0003), so a texture TEX does not hold as it is stays the DDS it is, and
//! so does every one when the run keeps DDS (#ADR-0055).
//!
//! TEX holds, as they are: BC1 (DXT1), BC3 (DXT5), BC7 (sRGB), BC5 (signed), and 32-bit pixels
//! with alpha as BGRA8, any other channel order swapped into it byte for byte. One 2D surface,
//! in whole blocks, with one mip or the whole chain. `ltk_texture` builds a TEX only by
//! encoding pixels, so the 12-byte header is written here and the result is read back
//! through it.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ltk_texture::tex::{Format, ResourceType, TextureFlags};
use serde::Serialize;

use super::findings::TextureContained;
use super::{Origin, Package};
use crate::diag::Sink;
use crate::model::ChunkPath;

/// Why a DDS does not go into TEX's container as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TexRefusal {
    /// The bytes do not read as a DDS.
    Unreadable,
    /// A pixel format TEX does not hold as it is: DXT3, a 24-bit format.
    Format,
    /// A cube map, a volume or an array.
    NotOneSurface,
    /// A side longer than TEX's 16 bits count.
    TooLarge,
    /// A size that is not in whole blocks.
    PartialBlocks,
    /// More than one mip and fewer than the whole chain.
    PartialMips,
    /// Fewer bytes than the mips need.
    Truncated,
    /// The TEX made does not read back with every mip.
    NotReadBack,
    /// The run keeps each DDS as it is (#ADR-0055).
    Asked,
}

impl TexRefusal {
    /// The reason in a message.
    pub fn as_str(self) -> &'static str {
        match self {
            TexRefusal::Unreadable => "not a readable DDS",
            TexRefusal::Format => "a format TEX does not hold as it is",
            TexRefusal::NotOneSurface => "not one 2D surface",
            TexRefusal::TooLarge => "larger than TEX holds",
            TexRefusal::PartialBlocks => "not in whole blocks",
            TexRefusal::PartialMips => "a partial mip chain",
            TexRefusal::Truncated => "fewer bytes than its mips need",
            TexRefusal::NotReadBack => "TEX does not read back with every mip",
            TexRefusal::Asked => "under the option to keep DDS",
        }
    }
}

/// TEX's format for a DDS's pixels, and for 32-bit pixels the byte each of B, G, R, A comes
/// from when that is not their order.
fn tex_format(dds: &ddsfile::Dds) -> Result<(Format, Option<[usize; 4]>), TexRefusal> {
    use ddsfile::{D3DFormat as D, DxgiFormat as X};
    if let Some(header) = &dds.header10 {
        return match header.dxgi_format {
            X::BC1_UNorm => Ok((Format::Bc1, None)),
            X::BC3_UNorm => Ok((Format::Bc3, None)),
            X::BC7_UNorm_sRGB => Ok((Format::Bc7, None)),
            X::BC5_SNorm => Ok((Format::Bc5Snorm, None)),
            X::B8G8R8A8_UNorm => Ok((Format::Bgra8, None)),
            X::R8G8B8A8_UNorm => Ok((Format::Bgra8, Some([2, 1, 0, 3]))),
            _ => Err(TexRefusal::Format),
        };
    }
    match dds.get_d3d_format() {
        Some(D::DXT1) => return Ok((Format::Bc1, None)),
        Some(D::DXT5) => return Ok((Format::Bc3, None)),
        _ => {}
    }
    // 32-bit pixels with alpha, in any byte order.
    let pixels = &dds.header.spf;
    let masks = [
        pixels.b_bit_mask,
        pixels.g_bit_mask,
        pixels.r_bit_mask,
        pixels.a_bit_mask,
    ];
    if pixels.fourcc.is_some() || pixels.rgb_bit_count != Some(32) {
        return Err(TexRefusal::Format);
    }
    let byte_of = |mask: Option<u32>| {
        let bytes = [0xff, 0xff00, 0xff_0000, 0xff00_0000];
        bytes.iter().position(|&byte| Some(byte) == mask)
    };
    let order = match masks.map(byte_of) {
        [Some(b), Some(g), Some(r), Some(a)] => [b, g, r, a],
        _ => return Err(TexRefusal::Format),
    };
    let mut seen = order;
    seen.sort_unstable();
    if seen != [0, 1, 2, 3] {
        return Err(TexRefusal::Format);
    }
    Ok((Format::Bgra8, (order != [0, 1, 2, 3]).then_some(order)))
}

fn contain(bytes: &[u8]) -> Result<Vec<u8>, TexRefusal> {
    let dds = ddsfile::Dds::read(bytes).map_err(|_| TexRefusal::Unreadable)?;
    let (format, swizzle) = tex_format(&dds)?;
    let (width, height) = (dds.get_width(), dds.get_height());
    let cube = dds.header.caps2.contains(ddsfile::Caps2::CUBEMAP);
    if dds.get_depth() > 1 || dds.get_num_array_layers() != 1 || cube {
        return Err(TexRefusal::NotOneSurface);
    }
    let (Ok(width16), Ok(height16)) = (u16::try_from(width), u16::try_from(height)) else {
        return Err(TexRefusal::TooLarge);
    };
    let (block_w, block_h) = format.block_size();
    let whole = |side: u32, block: usize| side != 0 && (side as usize).is_multiple_of(block);
    if !whole(width, block_w) || !whole(height, block_h) {
        return Err(TexRefusal::PartialBlocks);
    }
    let mips = dds.get_num_mipmap_levels().max(1);
    if mips != 1 && mips != width.max(height).ilog2() + 1 {
        return Err(TexRefusal::PartialMips);
    }
    let level_bytes = |level: u32| {
        let blocks = |side: u32, block: usize| ((side >> level).max(1) as usize).div_ceil(block);
        blocks(width, block_w) * blocks(height, block_h) * format.bytes_per_block()
    };
    let mut starts = Vec::with_capacity(mips as usize);
    let mut end = 0usize;
    for level in 0..mips {
        starts.push(end);
        end += level_bytes(level);
    }
    if end > dds.data.len() {
        return Err(TexRefusal::Truncated);
    }
    let flags = if mips > 1 {
        TextureFlags::HasMipMaps
    } else {
        TextureFlags::empty()
    };
    let mut out = Vec::with_capacity(12 + end);
    out.extend_from_slice(b"TEX\0");
    out.extend_from_slice(&width16.to_le_bytes());
    out.extend_from_slice(&height16.to_le_bytes());
    // The rest of the header: a depth of one slice, the format, the resource type and the
    // flags.
    out.extend_from_slice(&[
        1,
        format.to_u8(),
        ResourceType::Texture.to_u8(),
        flags.bits(),
    ]);
    // DDS stores the largest mip first, TEX the smallest.
    for level in (0..mips).rev() {
        let start = starts[level as usize];
        let pixels = &dds.data[start..start + level_bytes(level)];
        match swizzle {
            None => out.extend_from_slice(pixels),
            Some(o) => {
                let (pixels, _) = pixels.as_chunks::<4>();
                out.extend(
                    pixels
                        .iter()
                        .flat_map(|p| [p[o[0]], p[o[1]], p[o[2]], p[o[3]]]),
                );
            }
        }
    }
    let back = ltk_texture::Tex::from_reader(&mut out.as_slice());
    let back = back.map_err(|_| TexRefusal::NotReadBack)?;
    if back.mip_count != mips || back.available_mip_count() != mips {
        return Err(TexRefusal::NotReadBack);
    }
    Ok(out)
}

/// A DDS's bytes in TEX's container, or why they do not go in as they are. The bytes may be
/// a mod's, so the readers run where their panic is caught.
pub fn dds_to_tex(bytes: &[u8]) -> Result<Vec<u8>, TexRefusal> {
    catch_unwind(AssertUnwindSafe(|| contain(bytes))).unwrap_or(Err(TexRefusal::Unreadable))
}

/// The step that puts each DDS at a `.tex` path in TEX's container, and counts what went in
/// and what stays DDS.
#[derive(Debug, Clone, Default)]
pub struct TexContainer {
    keep_dds: bool,
    contained: usize,
    kept: BTreeMap<TexRefusal, usize>,
}

impl TexContainer {
    /// A step that has seen no texture.
    pub fn new() -> Self {
        Self::default()
    }

    /// Keeps each DDS as it is, and counts it (#ADR-0055).
    #[must_use]
    pub fn keep_dds(mut self, keep: bool) -> Self {
        self.keep_dds = keep;
        self
    }

    /// The bytes of a DDS at a `.tex` path in TEX's container. None for any other chunk, and
    /// for a DDS TEX's container does not hold as it is, which is counted by the reason.
    pub fn contain(&mut self, path: &ChunkPath, bytes: &[u8]) -> Option<Vec<u8>> {
        if path.extension() != Some("tex") || !bytes.starts_with(b"DDS ") {
            return None;
        }
        let contained = if self.keep_dds {
            Err(TexRefusal::Asked)
        } else {
            dds_to_tex(bytes)
        };
        match contained {
            Ok(tex) => Some(tex),
            Err(why) => {
                *self.kept.entry(why).or_default() += 1;
                None
            }
        }
    }

    /// Counts a texture that ships in TEX's container.
    pub fn shipped(&mut self) {
        self.contained += 1;
    }

    /// Replaces the bytes of each DDS at a `.tex` path of a package that TEX's container
    /// holds as it is.
    pub fn apply(&mut self, package: &mut Package) {
        for (_, key, chunk) in package.chunks_mut() {
            let Some(file) = chunk.origin.file() else {
                continue;
            };
            if let Some(tex) = self.contain(&key.path, &chunk.bytes) {
                chunk.bytes = tex.into();
                chunk.origin = Origin::Contained(file);
                self.shipped();
            }
        }
    }

    /// Raises how many textures went in and how many stay DDS, when there was any.
    pub fn report(self, sink: &mut Sink<'_>) {
        if self.contained > 0 || !self.kept.is_empty() {
            sink.raise(TextureContained {
                contained: self.contained,
                kept: self.kept,
            });
        }
    }
}
