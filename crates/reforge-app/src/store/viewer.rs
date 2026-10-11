//! One file of a kept run or of an explored mod as the Runs page and the Mod page show it, by
//! the kind its first bytes give: a bin as the rows of a tree the window reads apart
//! (#ADR-0104), a texture as a picture the app decoded, text, else the first bytes as hex. A
//! mod's bytes reach the window only as text through `clean` or as a PNG the app encoded
//! (#ADR-0093).

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use ltk_texture::Texture;
use reforge::model::{ContentId, clean};
use reforge_magic::{Categories, Kind};
use serde::{Deserialize, Serialize};

use super::AppStore;
use super::files::Tiers;
use crate::error::AppError;

/// A file past this size is shown as its first bytes whatever its kind: generous, a power of
/// two, and well past the largest texture or bin a skin ships.
pub const MAX_DECODED: u64 = 64 << 20;

/// The most text of a text file shown.
pub const MAX_TEXT: usize = 1 << 20;

/// How many first bytes a file shown as hex shows.
pub const HEX_BYTES: usize = 256;

/// How many bytes a row of hex holds.
const HEX_ROW: usize = 16;

/// The widest and the tallest texture decoded, as for a package's thumbnail: a larger one is
/// shown as hex.
const MAX_SIDE: u32 = 8192;

/// The longest side of the picture a texture is shown as.
const SHOWN_SIDE: u32 = 1024;

/// One file's bytes as the window shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "as", rename_all = "kebab-case")]
pub enum FileView {
    /// A bin, a `PROP` or a `PTCH`, whose rows the window reads through `bin_open` (#ADR-0104).
    Bin,
    /// A texture's largest mip, decoded by the app.
    Image {
        /// A PNG as a `data:` URL, its longest side at most 1024 pixels.
        png: String,
        /// The texture's width.
        width: u32,
        /// The texture's height.
        height: u32,
        /// Its pixel format, as its container names it.
        format: String,
    },
    /// Text, up to [`MAX_TEXT`].
    Text {
        /// The text.
        text: String,
        /// Whether the file holds more than is shown.
        cut: bool,
    },
    /// The first bytes, as hex.
    Hex {
        /// The first [`HEX_BYTES`] bytes, as pairs of hex digits, sixteen to a line.
        head: String,
        /// The file's size.
        size: u64,
    },
}

impl AppStore {
    /// The bytes under an id, from the run tier over the ingest tier, shown by their kind.
    /// Refused when no tier holds the bytes.
    pub fn file_view(&self, content: ContentId) -> Result<FileView, AppError> {
        file_view(&Tiers::of(self), content)
    }
}

/// The bytes under an id, from the first of `tiers` that holds them, shown by their kind.
/// Refused when no tier holds the bytes.
pub fn file_view(tiers: &Tiers<'_>, content: ContentId) -> Result<FileView, AppError> {
    let missing = || AppError::BlobMissing { id: content.hex() };
    let (size, head) = tiers.blob_facts(content).ok_or_else(missing)?;
    let kind = head.identify(None).kind;
    if matches!(kind, Kind::PropertyBin | Kind::PropertyBinOverride) {
        return Ok(FileView::Bin);
    }
    if size > MAX_DECODED {
        let head = tiers.get_head(content, HEX_BYTES).ok_or_else(missing)?;
        return Ok(hex(&head.bytes, size));
    }
    let bytes = tiers.get(content).ok_or_else(missing)?;
    let shown = match kind {
        Kind::Texture | Kind::TextureDds => picture(&bytes),
        kind if kind.info().categories.contains(Categories::TEXT) => Some(text(&bytes)),
        _ => None,
    };
    Ok(shown.unwrap_or_else(|| hex(&bytes, size)))
}

/// A texture's largest mip as a PNG. None for one that does not decode, or is larger than
/// [`MAX_SIDE`]. The bytes may be a mod's, so the decoders run where their panic is caught.
fn picture(bytes: &[u8]) -> Option<FileView> {
    catch_unwind(AssertUnwindSafe(|| decode(bytes)))
        .ok()
        .flatten()
}

fn decode(bytes: &[u8]) -> Option<FileView> {
    let texture = Texture::from_reader(&mut Cursor::new(bytes)).ok()?;
    let (width, height) = (texture.width(), texture.height());
    if width == 0 || height == 0 || width.max(height) > MAX_SIDE {
        return None;
    }
    let format = match &texture {
        Texture::Tex(tex) => format!("{:?}", tex.format),
        Texture::Dds(_) => dds_format(bytes),
    };
    let image = texture.decode_mipmap(0).ok()?.into_rgba_image().ok()?;
    let mut image = DynamicImage::ImageRgba8(image);
    if width.max(height) > SHOWN_SIDE {
        image = image.resize(SHOWN_SIDE, SHOWN_SIDE, FilterType::Triangle);
    }
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    Some(FileView::Image {
        png: format!("data:image/png;base64,{}", BASE64.encode(&png)),
        width,
        height,
        format,
    })
}

/// A DDS's pixel format, as its header names it.
fn dds_format(bytes: &[u8]) -> String {
    let Ok(dds) = ddsfile::Dds::read(bytes) else {
        return "DDS".to_string();
    };
    match (&dds.header10, dds.get_d3d_format()) {
        (Some(header), _) => format!("{:?}", header.dxgi_format),
        (None, Some(format)) => format!("{format:?}"),
        (None, None) => "DDS".to_string(),
    }
}

/// Text, cleaned, up to [`MAX_TEXT`] bytes cut at a character.
fn text(bytes: &[u8]) -> FileView {
    let cut = bytes.len() > MAX_TEXT;
    let read = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_TEXT)]);
    let mut read = read.into_owned();
    // A cut inside a character reads as a replacement character at the end.
    if cut && read.ends_with('\u{fffd}') {
        read.pop();
    }
    FileView::Text {
        text: clean(&read).into_owned(),
        cut,
    }
}

/// The first bytes as hex, sixteen to a line.
fn hex(bytes: &[u8], size: u64) -> FileView {
    let head = &bytes[..bytes.len().min(HEX_BYTES)];
    let mut text = String::new();
    for (at, row) in head.chunks(HEX_ROW).enumerate() {
        if at > 0 {
            text.push('\n');
        }
        let pairs: Vec<String> = row.iter().map(|byte| format!("{byte:02x}")).collect();
        text.push_str(&pairs.join(" "));
    }
    FileView::Hex { head: text, size }
}
