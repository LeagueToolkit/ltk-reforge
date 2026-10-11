//! The run's report a package's README embeds (#ADR-0102): the block's body written and read,
//! its bounds, and a `.modpkg` that carries it.

use std::io::{Cursor, Write as _};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_modpkg::Modpkg as Packed;
use reforge::model::entity::FileId;
use reforge::model::{ByteView, ChunkPath, Spelled, WadName};
use reforge::package::{
    ArchiveRef, Chunk, ChunkKey, EVIDENCE_FORMAT, EvidenceDating, EvidenceDoc, EvidenceInputs,
    Layer, MAX_REPORT_BLOCK_BYTES, MAX_REPORT_JSON_BYTES, Made, Modpkg, Origin, Package,
    PackageWriter, Program, ProjectConfig, REPORT_FORMAT, REPORT_LINE, Readme, ReportBlockError,
    ReportNote, decode_report, encode_report,
};
use reforge_testkit::bins::{bin, object, text};
use reforge_testkit::skins::WAD;
use serde_json::json;

/// A report of a few hundred rows, pretty as `Report::to_json` writes one.
fn report() -> String {
    let files: Vec<_> = (0..400)
        .map(|n| json!({ "path": format!("assets/mod/a/file{n}.tex"), "size": n * 37 }))
        .collect();
    let report = json!({ "schema": "reforge.report/1", "files": files });
    serde_json::to_string_pretty(&report).expect("JSON") + "\n"
}

/// A report whose bytes zstd does not shrink much: a string of characters from a generator,
/// six bits of each random.
fn noise(chars: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let noise: String = (0..chars)
        .map(|_| {
            // xorshift64: deterministic, and no pattern a compressor finds.
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            char::from(ALPHABET[(state >> 58) as usize])
        })
        .collect();
    format!("{{ \"noise\": \"{noise}\" }}\n")
}

/// The base64 of a zstd frame of a long run of one byte, past the reader's cap: small encoded,
/// large decoded. With `declared`, the frame states its size in its header.
fn bomb(declared: bool) -> String {
    let bytes = MAX_REPORT_JSON_BYTES + 1;
    let frame = if declared {
        zstd::bulk::compress(&vec![b' '; bytes], 1).expect("it compresses")
    } else {
        let mut encoder = zstd::stream::Encoder::new(Vec::new(), 1).expect("an encoder");
        let block = vec![b' '; 1 << 20];
        let mut left = bytes;
        while left > 0 {
            let take = left.min(block.len());
            encoder.write_all(&block[..take]).expect("it compresses");
            left -= take;
        }
        encoder.finish().expect("it compresses")
    };
    BASE64.encode(frame)
}

#[test]
fn a_report_reads_back_from_the_body_it_is_encoded_in() {
    let report = report();
    let body = encode_report(&report).expect("the report fits");
    assert_eq!(decode_report(&body).expect("it decodes"), report);
    assert!(body.len() < report.len() / 4, "{} bytes", body.len());
    // A report of no bytes is a frame too.
    let empty = encode_report("").expect("it fits");
    assert_eq!(decode_report(&empty).expect("it decodes"), "");
}

#[test]
fn the_body_is_lines_of_96_characters_but_the_last() {
    let body = encode_report(&report()).expect("the report fits");
    let lines: Vec<&str> = body.split('\n').collect();
    assert!(lines.len() > 2, "{} lines", lines.len());
    let (last, full) = lines.split_last().expect("lines");
    assert!(full.iter().all(|line| line.len() == REPORT_LINE));
    assert!(!last.is_empty() && last.len() <= REPORT_LINE);
    assert!(!body.ends_with('\n'));
    let alphabet = |b: u8| b.is_ascii_alphanumeric() || b"+/=\n".contains(&b);
    assert!(body.bytes().all(alphabet));
}

#[test]
fn a_reader_takes_any_line_length_any_line_end_and_any_level() {
    let report = report();
    let body = encode_report(&report).expect("the report fits");
    let one_line: String = body.split('\n').collect();
    let crlf = body.replace('\n', "\r\n");
    let short: Vec<String> = one_line
        .as_bytes()
        .chunks(64)
        .map(|line| format!("  {}\t", String::from_utf8_lossy(line)))
        .collect();
    for body in [one_line.clone(), crlf, short.join("\n")] {
        assert_eq!(decode_report(&body).expect("it decodes"), report);
    }
    let fast = zstd::bulk::compress(report.as_bytes(), 1).expect("it compresses");
    let fast = BASE64.encode(fast);
    assert_ne!(fast, one_line);
    assert_eq!(decode_report(&fast).expect("it decodes"), report);
}

#[test]
fn a_body_with_a_character_changed_is_refused() {
    let body = encode_report(&report()).expect("the report fits");
    let body: String = body.split('\n').collect();
    let middle = body.len() / 2;
    for at in [
        middle - REPORT_LINE,
        middle,
        middle + 3,
        body.len() - REPORT_LINE,
    ] {
        let mut changed = body.clone().into_bytes();
        // Another character of the alphabet: the body is still base64.
        changed[at] = if changed[at] == b'Q' { b'R' } else { b'Q' };
        let changed = String::from_utf8(changed).expect("ASCII");
        let refused = decode_report(&changed);
        assert!(
            matches!(refused, Err(ReportBlockError::Zstd(_))),
            "at {at}: {refused:?}"
        );
    }
    // A character outside the alphabet, and a body cut short.
    let foreign = body.replacen(|c: char| c.is_ascii_alphanumeric(), "*", 1);
    assert!(matches!(
        decode_report(&foreign),
        Err(ReportBlockError::Base64(_))
    ));
    let cut = &body[..body.len() / 2];
    assert!(decode_report(cut).is_err());
}

#[test]
fn a_body_that_decodes_past_the_cap_is_refused() {
    for declared in [true, false] {
        let body = bomb(declared);
        assert!(body.len() < MAX_REPORT_BLOCK_BYTES, "{} bytes", body.len());
        let refused = decode_report(&body);
        assert!(
            matches!(refused, Err(ReportBlockError::TooLarge)),
            "declared {declared}: {refused:?}"
        );
    }
}

#[test]
fn bytes_that_are_no_text_are_refused() {
    let frame = zstd::bulk::compress(&[0xff, 0xfe, 0x00], 3).expect("it compresses");
    let refused = decode_report(&BASE64.encode(frame));
    assert!(
        matches!(refused, Err(ReportBlockError::Utf8(_))),
        "{refused:?}"
    );
}

#[test]
fn a_report_over_the_bound_is_left_out_and_the_note_says_so() {
    let large = noise(1 << 20);
    assert_eq!(encode_report(&large), None);

    let doc = evidence();
    let readme = doc.readme(Some(&large));
    assert!(!readme.contains(REPORT_FORMAT), "{} bytes", readme.len());
    let read = Readme::parse(&readme).expect("the README reads");
    let note = ReportNote {
        embedded: false,
        json_bytes: large.len() as u64,
    };
    assert_eq!(read.evidence.map(|doc| doc.report), Some(note));
    assert_eq!(read.report, None);

    // One that fits is embedded, its note with its bytes.
    let small = report();
    let read = Readme::parse(&doc.readme(Some(&small))).expect("the README reads");
    let note = ReportNote {
        embedded: true,
        json_bytes: small.len() as u64,
    };
    assert_eq!(read.evidence.map(|doc| doc.report), Some(note));
    assert_eq!(read.report, Some(small));
}

/// The evidence of a run that dated nothing.
fn evidence() -> EvidenceDoc {
    EvidenceDoc {
        format: EVIDENCE_FORMAT.into(),
        program: Program {
            name: "reforge".into(),
            version: "0.1.0".into(),
        },
        run: Made::Repaired,
        archive: ArchiveRef {
            sha256: "cd".repeat(32),
        },
        inputs: EvidenceInputs {
            build: Some("16.19.1".into()),
            history: None,
        },
        dating: EvidenceDating {
            not_before: None,
            evidence: None,
            dated: 0,
            at_bound: 0,
        },
        restored: Vec::new(),
        fixups: vec!["retype".into()],
        port: None,
        report: ReportNote::default(),
    }
}

/// A package of one bin, with [`evidence`].
fn package() -> Package {
    let config = ProjectConfig {
        name: "Embedded".into(),
        display_name: Spelled::new("Embedded"),
        version: "1.0.0".into(),
        description: Spelled::new("A package built by hand."),
        authors: vec![Spelled::new("reforge tests")],
        champions: vec!["Test".into()],
    };
    let mut package = Package::new(config);
    let wad = WadName::parse(WAD).expect("a WAD name");
    let path = ChunkPath::parse("data/characters/test/skins/skin0.bin").expect("a path");
    let bytes = bin(vec![object("A", "Material", vec![("name", text("a"))])]);
    let chunk = Chunk::new(ByteView::from(bytes), Origin::Kept(FileId::new(0)));
    package.insert(Layer::Base, ChunkKey::new(wad, path), chunk);
    package.evidence = Some(evidence());
    package
}

/// The readme a `.modpkg` the writer wrote holds, and the report it wrote beside it.
fn written(report: &str) -> (String, String) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let out = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("a UTF-8 path");
    let package = package();
    let writer = Modpkg::new(&out);
    writer.write(&package, report).expect("the package writes");
    let bytes = fs::read(writer.path_of(&package)).expect("the package reads");
    let mut packed = Packed::mount_from_reader(Cursor::new(bytes)).expect("the package mounts");
    let readme = packed.load_readme().expect("the package has a readme");
    let readme = String::from_utf8(readme).expect("the readme is text");
    let beside = fs::read_to_string(writer.report_path_of(&package)).expect("the report");
    (readme, beside)
}

#[test]
fn a_modpkg_carries_the_report_written_beside_it() {
    let report = report();
    let (readme, beside) = written(&report);
    assert_eq!(beside, report);
    assert!(
        readme.contains(&format!("```{REPORT_FORMAT}\n")),
        "{readme}"
    );

    let read = Readme::parse(&readme).expect("the README reads");
    assert_eq!(read.report.as_deref(), Some(beside.as_str()));
    let doc = read.evidence.expect("the evidence");
    assert!(doc.report.embedded);
    assert_eq!(doc.report.json_bytes, report.len() as u64);

    // The same package and report give the same readme.
    let (again, _) = written(&report);
    assert_eq!(again, readme);
}
