//! The run's report as a block of a package's README (#ADR-0102): the block's body, written and
//! read.
//!
//! The body is the report's JSON as the writer writes it beside the package, compressed as one
//! zstd frame with its checksum and encoded as standard base64 in lines. A reader ignores
//! whitespace, takes any level and any line length, and stops decoding at a cap of its own.

use std::io::{self, Read as _};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

use super::evidence::MAX_README_BYTES;

/// The info string of the report's block.
pub const REPORT_FORMAT: &str = "reforge.report+zstd/1";

/// The zstd level the writer compresses at.
pub const REPORT_ZSTD_LEVEL: i32 = 9;

/// The characters of base64 on each line of the body but the last.
pub const REPORT_LINE: usize = 96;

/// The largest body the writer embeds, its line ends counted: past it the block is left out.
/// With the evidence's block, a README stays under [`MAX_README_BYTES`]. The largest report
/// measured, 11.5 MB of JSON, encodes to a body of 544 KiB at level 9 in 28 ms (2026-10-11).
pub const MAX_REPORT_BLOCK_BYTES: usize = 768 << 10;

/// The most bytes of JSON a reader decodes from a body: above the largest report measured,
/// 11.5 MB.
pub const MAX_REPORT_JSON_BYTES: usize = 64 << 20;

/// The largest zstd window a reader allows: a frame never needs one past the bytes it may
/// decode to, and a frame that declares one is refused before its window is allocated.
const MAX_WINDOW_LOG: u32 = MAX_REPORT_JSON_BYTES.ilog2();

const _: () = assert!(MAX_REPORT_BLOCK_BYTES < MAX_README_BYTES);

/// Why the body of a report's block gives no report.
#[derive(Debug, thiserror::Error)]
pub enum ReportBlockError {
    /// The body is not base64 of the standard alphabet with its padding.
    #[error("the report is not base64: {0}")]
    Base64(#[from] base64::DecodeError),
    /// The bytes are not a zstd frame, or its checksum does not hold.
    #[error("the report is not a zstd frame: {0}")]
    Zstd(#[source] io::Error),
    /// The frame decodes past [`MAX_REPORT_JSON_BYTES`].
    #[error("the report decodes past {max} bytes", max = MAX_REPORT_JSON_BYTES)]
    TooLarge,
    /// The decoded bytes are not UTF-8.
    #[error("the report is not UTF-8")]
    Utf8(#[from] std::string::FromUtf8Error),
}

/// The body of a report's block: `json` compressed and encoded, in lines of [`REPORT_LINE`].
/// None when the body would pass [`MAX_REPORT_BLOCK_BYTES`].
///
/// # Panics
///
/// When zstd fails to compress into memory, which no input makes it do.
pub fn encode_report(json: &str) -> Option<String> {
    let frame = compress(json.as_bytes()).unwrap_or_else(|error| {
        panic!(
            "a report of {} bytes does not compress at level {REPORT_ZSTD_LEVEL}: {error}",
            json.len()
        )
    });
    let encoded = BASE64.encode(frame);
    let lines = encoded.len().div_ceil(REPORT_LINE);
    let size = encoded.len() + lines.saturating_sub(1);
    if size > MAX_REPORT_BLOCK_BYTES {
        return None;
    }
    let mut body = String::with_capacity(size);
    // Base64 is ASCII: every index is a character's boundary.
    for start in (0..encoded.len()).step_by(REPORT_LINE) {
        if start > 0 {
            body.push('\n');
        }
        body.push_str(&encoded[start..(start + REPORT_LINE).min(encoded.len())]);
    }
    Some(body)
}

/// One frame at the writer's level, with its checksum, so a body changed in one character is
/// refused rather than read as another report.
fn compress(bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut compressor = zstd::bulk::Compressor::new(REPORT_ZSTD_LEVEL)?;
    compressor.include_checksum(true)?;
    compressor.compress(bytes)
}

/// The report's JSON a block's body holds. The body is untrusted: it is decoded up to
/// [`MAX_REPORT_JSON_BYTES`].
pub fn decode_report(body: &str) -> Result<String, ReportBlockError> {
    let encoded: Vec<u8> = body.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let frame = BASE64.decode(encoded)?;
    // A frame that declares its size past the cap is refused unread; one that declares none
    // is cut at the cap.
    let declared = zstd::zstd_safe::get_frame_content_size(&frame);
    if declared.is_ok_and(|size| size.is_some_and(|size| size > MAX_REPORT_JSON_BYTES as u64)) {
        return Err(ReportBlockError::TooLarge);
    }
    let mut decoder = zstd::stream::read::Decoder::with_buffer(frame.as_slice())
        .map_err(ReportBlockError::Zstd)?;
    decoder
        .window_log_max(MAX_WINDOW_LOG)
        .map_err(ReportBlockError::Zstd)?;
    let mut json = Vec::new();
    // One byte past the cap tells a report at the cap from one over it.
    let cap = MAX_REPORT_JSON_BYTES as u64 + 1;
    decoder
        .take(cap)
        .read_to_end(&mut json)
        .map_err(ReportBlockError::Zstd)?;
    if json.len() > MAX_REPORT_JSON_BYTES {
        return Err(ReportBlockError::TooLarge);
    }
    Ok(String::from_utf8(json)?)
}
