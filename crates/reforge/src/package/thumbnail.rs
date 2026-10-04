//! The package's thumbnail: the mod's image decoded under limits and encoded again as WebP,
//! which is what a mod project and a `.modpkg` hold (#ADR-0051). A package holds the bytes
//! this module encoded and never the ones the archive shipped.

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use image::{DynamicImage, ImageFormat, ImageReader, Limits};
use serde::Serialize;

use super::Package;
use super::findings::ThumbnailDropped;
use crate::diag::Sink;
use crate::model::entity::{MAX_IMAGE_BYTES, ModImage};

/// The thumbnail's file name in a mod project.
pub const THUMBNAIL_FILE: &str = "thumbnail.webp";

/// The widest and the tallest image decoded.
const MAX_SIDE: u32 = 8192;

/// The most memory a decoder may ask for. An allocation that fails ends the process.
const MAX_DECODE_BYTES: u64 = 512 << 20;

/// What `META/image.png` is read as, in this order: the name is the format's, and authors put
/// a JPEG or a WebP under it.
const READ_AS: [ImageFormat; 3] = [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP];

/// Why a mod's image is not the package's thumbnail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "why", rename_all = "kebab-case")]
pub enum ThumbnailRefusal {
    /// The entry is larger than a run holds.
    EntryTooLarge {
        /// Its inflated size.
        bytes: u64,
    },
    /// The entry does not read from the archive.
    EntryUnreadable,
    /// The bytes decode as no PNG, JPEG or WebP within the limits.
    NotAnImage,
    /// The image does not encode as WebP.
    NotEncoded,
    /// The WebP is larger than a packer takes.
    TooLarge {
        /// Its size.
        bytes: u64,
    },
}

/// A package's thumbnail: a WebP this module encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    webp: Arc<[u8]>,
}

impl Thumbnail {
    /// The thumbnail of a mod's image.
    pub fn of(image: &ModImage) -> Result<Self, ThumbnailRefusal> {
        let bytes = match image {
            ModImage::Held(bytes) => bytes,
            ModImage::TooLarge(bytes) => {
                return Err(ThumbnailRefusal::EntryTooLarge { bytes: *bytes });
            }
            ModImage::Unreadable => return Err(ThumbnailRefusal::EntryUnreadable),
        };
        // The bytes are a mod's: a decoder's panic is caught.
        let decoded = catch_unwind(AssertUnwindSafe(|| decode(bytes)));
        let decoded = decoded.ok().flatten();
        let decoded = decoded.ok_or(ThumbnailRefusal::NotAnImage)?;
        let webp = encode(decoded).ok_or(ThumbnailRefusal::NotEncoded)?;
        if webp.len() as u64 > MAX_IMAGE_BYTES {
            return Err(ThumbnailRefusal::TooLarge {
                bytes: webp.len() as u64,
            });
        }
        Ok(Self {
            webp: Arc::from(webp),
        })
    }

    /// The WebP's bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.webp
    }
}

fn decode(bytes: &[u8]) -> Option<DynamicImage> {
    READ_AS.into_iter().find_map(|format| {
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        limits.max_alloc = Some(MAX_DECODE_BYTES);
        let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
        reader.limits(limits);
        reader.decode().ok()
    })
}

fn encode(image: DynamicImage) -> Option<Vec<u8>> {
    // The WebP encoder takes 8-bit RGB and RGBA only.
    let image = match image {
        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => image,
        other => DynamicImage::ImageRgba8(other.to_rgba8()),
    };
    let mut webp = Vec::new();
    let written = image.write_to(&mut Cursor::new(&mut webp), ImageFormat::WebP);
    written.ok().map(|()| webp)
}

impl Package {
    /// Takes a mod's image as the thumbnail. An image that is not taken raises why, and the
    /// package has no thumbnail.
    pub fn carry_image(&mut self, image: &ModImage, sink: &mut Sink<'_>) {
        match Thumbnail::of(image) {
            Ok(thumbnail) => self.thumbnail = Some(thumbnail),
            Err(why) => {
                sink.raise(ThumbnailDropped { why });
            }
        }
    }
}
