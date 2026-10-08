//! The bytes of every bin entry's object at every build (#ADR-0078), held in the index so that a
//! run reads an object with no clone of the census history.
//!
//! Each distinct object is one row of `object_bytes`, a zstd frame of its bytes from the entry
//! hash on. An entry's versions are its distinct objects in the order they appeared. Every
//! `keyframe_every`th version is a keyframe, compressed with the dictionary trained on its
//! class's keyframes; every other version is compressed with its block's keyframe as raw content.
//! A read decodes two frames at most. `docs/design/HISTORY.md`, "Object bytes".

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zstd::zstd_safe::{self, DCtx, DDict};

/// The zstd level of every frame of the object bytes.
pub const LEVEL: i32 = 19;

/// How many versions of an entry share a keyframe, the keyframe among them.
pub const KEYFRAME_EVERY: u32 = 8;

/// The largest dictionary trained for one class's keyframes.
pub const DICT_CAP: u32 = 1 << 20;

/// The most bytes one object decodes to. The largest object of the census history is 545,257
/// bytes (2026-10-08).
pub const MAX_OBJECT: usize = 16 << 20;

/// How the object bytes of an index were compressed: what the stamp records so that the frames
/// can be made again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectCodec {
    /// The zstd level of every frame.
    pub level: i32,
    /// How many versions of an entry share a keyframe.
    pub keyframe_every: u32,
    /// The largest dictionary trained for one class's keyframes, in bytes.
    pub dict_cap: u32,
    /// The version of the zstd library that wrote the frames. Another version may write other
    /// frames of the same bytes.
    pub zstd: String,
}

impl ObjectCodec {
    /// The codec this crate writes with.
    pub fn current() -> Self {
        Self {
            level: LEVEL,
            keyframe_every: KEYFRAME_EVERY,
            dict_cap: DICT_CAP,
            zstd: zstd_safe::version_string().to_string(),
        }
    }
}

/// Why a frame of the object bytes does not decode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    /// The frame states no size, or one above `MAX_OBJECT`.
    #[error("the frame states {0:?} bytes")]
    Size(Option<u64>),
    /// zstd refused the frame.
    #[error("zstd: {0}")]
    Zstd(&'static str),
}

fn zstd_error(code: usize) -> FrameError {
    FrameError::Zstd(zstd_safe::get_error_name(code))
}

/// Decodes the frames of the object bytes, holding each dictionary once it is used.
pub struct Decoder {
    dctx: DCtx<'static>,
    dicts: HashMap<i64, DDict<'static>>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self {
            dctx: DCtx::create(),
            dicts: HashMap::new(),
        }
    }
}

impl Decoder {
    /// Whether the dictionary of this id is held.
    pub fn holds(&self, dict: i64) -> bool {
        self.dicts.contains_key(&dict)
    }

    /// Holds the dictionary of this id.
    pub fn hold(&mut self, dict: i64, bytes: &[u8]) {
        self.dicts.insert(dict, DDict::create(bytes));
    }

    /// The bytes of a keyframe: its frame, with the dictionary `dict`, which is held, or none.
    pub fn keyframe(&mut self, dict: Option<i64>, frame: &[u8]) -> Result<Vec<u8>, FrameError> {
        let mut out = Vec::with_capacity(stated_size(frame)?);
        let done = match dict.and_then(|id| self.dicts.get(&id)) {
            Some(ddict) => self.dctx.decompress_using_ddict(&mut out, frame, ddict),
            None => self.dctx.decompress_using_dict(&mut out, frame, &[]),
        };
        done.map_err(zstd_error)?;
        Ok(out)
    }

    /// The bytes of a version compressed against its keyframe's bytes.
    pub fn delta(&mut self, keyframe: &[u8], frame: &[u8]) -> Result<Vec<u8>, FrameError> {
        let mut out = Vec::with_capacity(stated_size(frame)?);
        self.dctx
            .decompress_using_dict(&mut out, frame, keyframe)
            .map_err(zstd_error)?;
        Ok(out)
    }
}

/// The size a frame states, within `MAX_OBJECT`.
pub fn stated_size(frame: &[u8]) -> Result<usize, FrameError> {
    let stated = zstd_safe::get_frame_content_size(frame).ok().flatten();
    stated
        .and_then(|size| usize::try_from(size).ok())
        .filter(|size| *size <= MAX_OBJECT)
        .ok_or(FrameError::Size(stated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delta_decodes_against_its_keyframes_bytes() {
        let keyframe: Vec<u8> = (0..4096u32).flat_map(|n| (n % 251).to_le_bytes()).collect();
        let mut version = keyframe.clone();
        version[100] ^= 1;
        let frame = zstd::bulk::compress(&keyframe, LEVEL).unwrap();
        let mut c = zstd::bulk::Compressor::with_dictionary(LEVEL, &keyframe).unwrap();
        let delta = c.compress(&version).unwrap();
        assert!(
            delta.len() < 64,
            "a one-byte change takes {} bytes",
            delta.len()
        );
        let mut decoder = Decoder::default();
        let base = decoder.keyframe(None, &frame).unwrap();
        assert_eq!(base, keyframe);
        assert_eq!(decoder.delta(&base, &delta).unwrap(), version);
    }

    #[test]
    fn a_frame_that_states_no_size_is_refused() {
        let mut streamed = Vec::new();
        let mut encoder = zstd::stream::Encoder::new(&mut streamed, 3).unwrap();
        std::io::Write::write_all(&mut encoder, b"no size").unwrap();
        encoder.finish().unwrap();
        let mut decoder = Decoder::default();
        assert_eq!(
            decoder.keyframe(None, &streamed),
            Err(FrameError::Size(None))
        );
    }
}
