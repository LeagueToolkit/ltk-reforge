//! One file of a kept run or of an explored mod as the Runs page and the Mod page show it, by
//! the kind its first bytes give: a bin as
//! text with its names spelled and an object asked for first, a texture as a picture the app
//! decoded, text, else the first bytes as hex. A mod's bytes reach the window only as text through `clean` or as a PNG the
//! app encoded (#ADR-0093).

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use ltk_texture::Texture;
use reforge::game::{NameBook, Names, Speller};
use reforge::model::dump::dump_object;
use reforge::model::{BinHash, ContentId, bin, clean};
use reforge_magic::{Categories, Kind};
use serde::{Deserialize, Serialize};

use super::AppStore;
use super::files::Tiers;
use crate::error::AppError;

/// A file past this size is shown as its first bytes whatever its kind: generous, a power of
/// two, and well past the largest texture or bin a skin ships.
pub const MAX_DECODED: u64 = 64 << 20;

/// The most text of a bin's objects shown: whole objects up to it, and one object of the history
/// cut at a line end.
pub const MAX_BIN_TEXT: usize = 2 << 20;

/// The most text of a text file shown.
pub const MAX_TEXT: usize = 1 << 20;

/// The text of the lines that fence the object asked for in a bin's text: the record
/// separator, a control a dump escapes as it escapes every one but newline and tab, so no
/// object's text holds it.
pub const ENTRY_FENCE: &str = "\u{1e}";

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
    /// A bin's objects as text, names spelled where the run or the tables spell them.
    Bin {
        /// The objects, whole, up to [`MAX_BIN_TEXT`]. The object of the entry asked for comes
        /// first, between two lines that hold [`ENTRY_FENCE`] alone, and the others follow in
        /// the bin's order.
        text: String,
        /// How many objects follow the ones shown.
        more: u64,
        /// Whether the bin holds the entry asked for. None when none was asked.
        entry_found: Option<bool>,
    },
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

/// A file asked for by the window: the id of its bytes, and for a bin, the entry of the object
/// to show first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileAsk {
    /// The id of the bytes.
    pub content: ContentId,
    /// The object's entry hash.
    pub entry: Option<BinHash>,
}

impl From<ContentId> for FileAsk {
    fn from(content: ContentId) -> Self {
        Self {
            content,
            entry: None,
        }
    }
}

impl AppStore {
    /// The bytes under an id, from the run tier over the ingest tier, shown by their kind.
    /// A bin's names are spelled through the run's load snapshot and `names`, the tables of
    /// the inputs where they are open. Refused when no tier holds the bytes.
    pub fn file_view(
        &self,
        id: u64,
        ask: impl Into<FileAsk>,
        names: Option<&Names>,
    ) -> Result<FileView, AppError> {
        let book = || {
            let load = self.kept_snapshots(id)?.and_then(|kept| kept.load);
            Ok(load.map(|load| load.names().clone()).unwrap_or_default())
        };
        file_view(&Tiers::of(self), ask, book, names)
    }
}

/// The bytes under an id, from the first of `tiers` that holds them, shown by their kind. A
/// bin's names are spelled through the run's names, which `book` reads when a bin needs them,
/// and `names`, the tables of the inputs where they are open. Refused when no tier holds the
/// bytes.
pub fn file_view(
    tiers: &Tiers<'_>,
    ask: impl Into<FileAsk>,
    book: impl FnOnce() -> Result<NameBook, AppError>,
    names: Option<&Names>,
) -> Result<FileView, AppError> {
    let FileAsk { content, entry } = ask.into();
    let missing = || AppError::BlobMissing { id: content.hex() };
    let (size, head) = tiers.blob_facts(content).ok_or_else(missing)?;
    if size > MAX_DECODED {
        let head = tiers.get_head(content, HEX_BYTES).ok_or_else(missing)?;
        return Ok(hex(&head.bytes, size));
    }
    let bytes = tiers.get(content).ok_or_else(missing)?;
    let kind = head.identify(None).kind;
    let shown = match kind {
        Kind::PropertyBin => bin_view(&bytes, &book()?, names, entry),
        Kind::Texture | Kind::TextureDds => picture(&bytes),
        kind if kind.info().categories.contains(Categories::TEXT) => Some(text(&bytes)),
        _ => None,
    };
    Ok(shown.unwrap_or_else(|| hex(&bytes, size)))
}

/// A bin's objects as text, the object of `entry` first between two fences. None for bytes
/// that do not read as a bin.
fn bin_view(
    bytes: &[u8],
    book: &NameBook,
    names: Option<&Names>,
    entry: Option<BinHash>,
) -> Option<FileView> {
    let parsed = bin::parse(bytes).ok()?;
    let none = Names::default();
    let speller = Speller::new(book, names.unwrap_or(&none));
    let objects = parsed.objects.iter().map(|object| &object.value);
    let asked = entry.and_then(|entry| objects.clone().position(|o| o.path_hash == entry));
    let mut text = String::new();
    if let Some(at) = asked {
        let fence = format!("{ENTRY_FENCE}\n");
        text.push_str(&fence);
        text.push_str(&dump_object(&parsed.objects[at].value, &speller));
        text.push_str(&fence);
    }
    let mut shown = usize::from(asked.is_some());
    let rest = objects.enumerate().filter(|&(at, _)| Some(at) != asked);
    for (_, object) in rest {
        let dumped = dump_object(object, &speller);
        if shown > 0 && text.len() + dumped.len() > MAX_BIN_TEXT {
            break;
        }
        text.push_str(&dumped);
        shown += 1;
    }
    Some(FileView::Bin {
        text,
        more: (parsed.objects.len() - shown) as u64,
        entry_found: entry.map(|_| asked.is_some()),
    })
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
