//! The checks that are code: magics held to a value or at an offset, text read for what it says,
//! names that say what a file is, and the heuristics for formats with no magic.

use crate::Kind;
use crate::table;

fn u16_le(head: &[u8], at: usize) -> Option<u16> {
    head.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_le(head: &[u8], at: usize) -> Option<u32> {
    head.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Magics that are not a fixed prefix: at an offset, or a short prefix held to the value after
/// it, which text starting with the same letters does not have.
pub(crate) fn structured(head: &[u8]) -> Option<Kind> {
    let byte = |i: usize| head.get(i).copied();
    if head.starts_with(b"PROP") && u32_le(head, 4).is_some_and(|v| (1..=3).contains(&v)) {
        return Some(Kind::PropertyBin);
    }
    // A DOS header's `e_lfanew`, where the PE header starts.
    if head.starts_with(b"MZ") && u32_le(head, 0x3C).is_some_and(|v| (0x40..0x1000).contains(&v)) {
        return Some(Kind::Pe);
    }
    if head.starts_with(b"RST") && byte(3).is_some_and(|v| (2..=5).contains(&v)) {
        return Some(Kind::StringTable);
    }
    if head.starts_with(b"BZh") && byte(3).is_some_and(|v| v.is_ascii_digit() && v != b'0') {
        return Some(Kind::Bzip2);
    }
    if head.starts_with(b"\x1f\x8b") && byte(2) == Some(8) {
        return Some(Kind::Gzip);
    }
    if head.starts_with(b"ID3") && byte(3).is_some_and(|v| (2..=4).contains(&v)) {
        return Some(Kind::Mp3);
    }
    if head.starts_with(b"r3d2") {
        // A Wwise package is `r3d2` and version 1; any other `r3d2` is a League file this crate
        // does not know, never a looser match below.
        return Some(if u32_le(head, 4) == Some(1) {
            Kind::WwisePackage
        } else {
            Kind::R3d2
        });
    }
    if u32_le(head, 4) == Some(0x22FD_4FC3) {
        return Some(Kind::Skeleton);
    }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(Kind::Jpeg);
    }
    if head.starts_with(b"RIFF") {
        return match head.get(8..12) {
            Some(b"WAVE") => Some(Kind::Wav),
            Some(b"WEBP") => Some(Kind::WebP),
            Some(b"AVI ") => Some(Kind::Avi),
            _ => None,
        };
    }
    if head.get(4..8) == Some(b"ftyp") {
        return Some(Kind::Mp4);
    }
    if head.starts_with(b"FOR4") || head.starts_with(b"FOR8") {
        // An IFF file: its form type after the size names what it holds.
        if head.get(8..12) == Some(b"CIMG") {
            return Some(Kind::MayaImage);
        }
        if head.windows(4).any(|w| w == b"Maya") {
            return Some(Kind::MayaBinary);
        }
    }
    // A WAD: `RW`, a major version from 1 to 3, a small minor.
    if head.starts_with(b"RW") && head.len() >= 4 && (1..=3).contains(&head[2]) && head[3] <= 8 {
        return Some(Kind::Wad);
    }
    None
}

/// The text after a byte-order mark and leading whitespace, when the head reads as text.
pub(crate) fn text_of(head: &[u8]) -> Option<&str> {
    let head = head.strip_prefix(b"\xef\xbb\xbf").unwrap_or(head);
    let text = match std::str::from_utf8(head) {
        Ok(text) => text,
        // A head cut inside a character is still text.
        Err(e) if e.error_len().is_none() => std::str::from_utf8(&head[..e.valid_up_to()]).ok()?,
        Err(_) => return None,
    };
    let printable = text
        .chars()
        .all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'));
    (printable && !text.trim().is_empty()).then(|| text.trim_start())
}

/// The kind of a text file: what its opening says, else what its extension says, else plain text.
pub(crate) fn text_kind(text: &str, ext: &str) -> Kind {
    if text.starts_with("#PROP_text") {
        return Kind::RitobinText;
    }
    if let Some(rest) = text.strip_prefix('{') {
        let rest = rest.trim_start();
        let bin = rest.starts_with("\"entries\"")
            || (rest.starts_with("\"type\"") && text.contains("PROP"));
        if bin {
            return Kind::RitobinJson;
        }
        return if ext == "gltf" {
            Kind::Gltf
        } else {
            Kind::Json
        };
    }
    if let Some(rest) = text.strip_prefix('[') {
        // `[Section]` on its first line is INI; `[` then a value is a JSON array.
        let line = rest.lines().next().unwrap_or("");
        let section = line.starts_with(|c: char| c.is_ascii_alphabetic() || c == '.' || c == '_')
            && line.contains(']');
        if !section {
            return Kind::Json;
        }
        if line.starts_with(".ShellClassInfo") {
            return Kind::DesktopIni;
        }
        return if ext == "troy" { Kind::Troy } else { Kind::Ini };
    }
    if text.starts_with("<svg") || (text.starts_with("<?xml") && text.contains("<svg")) {
        return Kind::Svg;
    }
    if text.starts_with("<?xml") {
        return Kind::Xml;
    }
    // `get`, not a slice: the fifth byte of a mod's text may be inside a character.
    if text.get(..5).is_some_and(|open| {
        open.eq_ignore_ascii_case("<!doc") || open.eq_ignore_ascii_case("<html")
    }) {
        return Kind::Html;
    }
    if text.starts_with("; FBX") {
        return Kind::FbxAscii;
    }
    if text.starts_with("#!") {
        return Kind::Script;
    }
    table::by_text_extension(ext).unwrap_or(Kind::Text)
}

/// A name that says what the file is whatever it holds: the leftovers of file browsers.
pub(crate) fn by_name(name: &str) -> Option<Kind> {
    let lower = name.replace('\\', "/").to_ascii_lowercase();
    let base = lower.rsplit('/').next().unwrap_or(&lower);
    if lower.contains("__macosx/") || base.starts_with("._") {
        return Some(Kind::AppleDouble);
    }
    match base {
        ".ds_store" => Some(Kind::DsStore),
        "thumbs.db" | "ehthumbs.db" => Some(Kind::ThumbsDb),
        "desktop.ini" => Some(Kind::DesktopIni),
        _ if base.starts_with("iconcache") && base.ends_with(".db") => Some(Kind::IconCache),
        _ => None,
    }
}

/// Formats with no magic, judged by plausible header values. Last, since random bytes pass.
pub(crate) fn heuristic(head: &[u8]) -> Option<Kind> {
    if head.starts_with(b"BM") && head.len() >= 14 && u32_le(head, 6) == Some(0) {
        return Some(Kind::Bmp);
    }
    if head.len() >= 6 && head[..4] == [0, 0, 1, 0] && head[4] != 0 {
        return Some(Kind::Ico);
    }
    // TGA: colour map type 0 or 1, and a defined image type.
    if head.len() >= 3 && head[1] <= 1 && matches!(head[2], 1 | 2 | 3 | 9 | 10 | 11) {
        return Some(Kind::Tga);
    }
    // Inibin version 2, as `.troybin` files are: the version, the string table's length, then
    // flags naming the value sections that follow, 0 to 13, and the first section's count.
    let inibin = head.first() == Some(&2)
        && u16_le(head, 3).is_some_and(|flags| flags != 0 && flags & 0xC000 == 0)
        && u16_le(head, 5).is_some_and(|count| count != 0);
    if inibin {
        return Some(Kind::Inibin);
    }
    if u32_le(head, 0) == Some(3) {
        return Some(Kind::LightGrid);
    }
    None
}
