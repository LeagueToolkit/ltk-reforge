//! The evidence a package embeds (#ADR-0041): the document a run's results give, the README
//! that carries it, and the reader of that README with its refusals. The game is
//! `reforge_testkit::skins`.

use reforge::env::Env;
use reforge::fixups::{FixupSet, ObjectSet};
use reforge::history::History;
use reforge::load::LoadOptions;
use reforge::load::dating::Piece;
use reforge::model::text::is_unsafe;
use reforge::model::{ContentId, NormKey, ObjectKey, name_hash, path_hash};
use reforge::package::{
    ArchiveRef, EVIDENCE_FORMAT, EvidenceBound, EvidenceDating, EvidenceDoc, EvidenceError,
    EvidenceHistory, EvidenceInputs, EvidencePiece, EvidencePolicy, EvidencePort, EvidenceUnit,
    MAX_README_BYTES, Made, PieceKind, Program, REPORT_FORMAT, Readme, ReportBlockError,
    ReportNote, encode_report,
};
use reforge_testkit::bins::{bin, entry};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::plans::{NEW_TEX, OLD_TEX, reworked};
use reforge_testkit::skins::{self, BASE_MESH, BASE_TEX, WAD, base_skin, skin_bin};
use reforge_testkit::{FixRun, LoadRun, MemoryHistory, ModArchive, PortRun};

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";
const SHA256: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
const KEY: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

/// The document of a repair, written by hand: a mod dated by a file at the first of two
/// builds, with one restoring WAD.
fn repair_doc() -> EvidenceDoc {
    EvidenceDoc {
        format: EVIDENCE_FORMAT.into(),
        program: Program {
            name: "reforge".into(),
            version: "0.1.0".into(),
        },
        run: Made::Repaired,
        archive: ArchiveRef {
            sha256: SHA256.into(),
        },
        inputs: EvidenceInputs {
            build: Some("16.19.1".into()),
            history: Some(EvidenceHistory {
                head: "c1".into(),
                newest_build: "16.19.1".into(),
                builds: 2,
            }),
        },
        dating: EvidenceDating {
            not_before: Some(EvidenceBound {
                at: 0,
                build: "16.18.1".into(),
            }),
            evidence: Some(EvidencePiece {
                piece: PieceKind::File,
                hash: "0123456789abcdef".into(),
                key: KEY.into(),
            }),
            dated: 3,
            at_bound: 1,
        },
        restored: vec!["test.wad.client".into()],
        fixups: vec!["legacy-formats".into(), "retype".into()],
        port: None,
        report: ReportNote::default(),
    }
}

/// The document of a port, written by hand: a mod dated by an object, with one unit that
/// ships.
fn port_doc() -> EvidenceDoc {
    let mut doc = repair_doc();
    doc.run = Made::Ported;
    doc.restored.clear();
    doc.dating.evidence = Some(EvidencePiece {
        piece: PieceKind::Object,
        hash: "89abcdef".into(),
        key: KEY.into(),
    });
    doc.port = Some(EvidencePort {
        policy: EvidencePolicy {
            target: "auto".into(),
            anim_reuse: "own-rig".into(),
        },
        units: vec![EvidenceUnit {
            character: "test".into(),
            source: 0,
            target: 3,
            reason: "fallback".into(),
        }],
    });
    doc
}

/// A README whose one block of evidence holds this body.
fn readme_of(body: &str) -> String {
    format!("# A package\n\n```{EVIDENCE_FORMAT}\n{body}\n```\n")
}

/// The README of a document with one line of its JSON written another way.
fn with_line(doc: &EvidenceDoc, from: &str, to: &str) -> String {
    let readme = doc.readme(None);
    assert!(readme.contains(from), "the README holds {from}:\n{readme}");
    readme.replace(from, to)
}

fn field_refused(readme: &str) -> &'static str {
    match EvidenceDoc::parse(readme) {
        Err(EvidenceError::Field(field)) => field,
        other => panic!("a field is refused, and the reader gave {other:?}"),
    }
}

#[test]
fn a_document_reads_back_from_the_readme_it_is_written_in() {
    for doc in [repair_doc(), port_doc()] {
        let read = EvidenceDoc::parse(&doc.readme(None)).expect("the README reads");
        assert_eq!(read, Some(doc));
    }
    // A README saved with the line ends of another system reads the same.
    let crlf = repair_doc().readme(None).replace('\n', "\r\n");
    assert_eq!(
        EvidenceDoc::parse(&crlf).expect("the README reads"),
        Some(repair_doc())
    );
}

#[test]
fn the_readme_says_what_made_the_package_and_holds_the_document_in_one_block() {
    let readme = repair_doc().readme(None);
    let expected = format!(
        "# Package made by reforge\n\
         \n\
         reforge 0.1.0 made this package by a repair of a mod archive, for the game build 16.19.1.\n\
         The SHA-256 of the archive is `{SHA256}`.\n\
         The block below holds what the run established of the mod, in the format\n\
         `reforge.evidence/1`. A program reads it, so it stays as written.\n\
         \n\
         ```reforge.evidence/1\n\
         {}\n\
         ```\n",
        repair_doc().to_json()
    );
    assert_eq!(readme, expected);

    let port = port_doc().readme(None);
    assert!(port.contains("made this package by a port of a mod archive"));
    let mut unstated = repair_doc();
    unstated.inputs.build = None;
    let unstated = unstated.readme(None);
    assert!(unstated.contains("for a game that does not state its build."));
}

#[test]
fn the_documents_json_holds_its_fields_in_one_order_and_no_port_for_a_repair() {
    let keys = |doc: &EvidenceDoc| -> Vec<String> {
        let json = doc.to_json();
        let top = json.lines().filter(|line| line.starts_with("  \""));
        top.map(|line| {
            line.trim()
                .split('"')
                .nth(1)
                .unwrap_or_default()
                .to_string()
        })
        .collect()
    };
    let common = [
        "format", "program", "run", "archive", "inputs", "dating", "restored", "fixups",
    ];
    assert_eq!(keys(&repair_doc()), common);
    let mut ported: Vec<&str> = common.to_vec();
    ported.push("port");
    assert_eq!(keys(&port_doc()), ported);

    let json: serde_json::Value = serde_json::from_str(&port_doc().to_json()).expect("JSON");
    assert_eq!(json["format"], "reforge.evidence/1");
    assert_eq!(json["run"], "port");
    let piece = serde_json::json!({ "piece": "object", "hash": "89abcdef", "key": KEY });
    assert_eq!(json["dating"]["evidence"], piece);
    let unit = serde_json::json!({
        "character": "test", "source": 0, "target": 3, "reason": "fallback",
    });
    assert_eq!(json["port"]["units"][0], unit);
    let repair: serde_json::Value = serde_json::from_str(&repair_doc().to_json()).expect("JSON");
    assert_eq!(repair["run"], "repair");
    assert_eq!(repair["dating"]["evidence"]["piece"], "file");
}

#[test]
fn text_of_the_mod_is_escaped_in_the_readme_and_reads_back_as_written() {
    // A WAD's name and a character's are the mod's: neither closes the block nor reaches a
    // terminal as written.
    let mut doc = port_doc();
    doc.restored = vec!["a\n```\n\u{1b}[31m\u{202e}.wad.client".into()];
    let port = doc.port.as_mut().expect("a port");
    port.units[0].character = "te\"st\u{7f}".into();
    let readme = doc.readme(None);
    assert!(!readme.chars().any(is_unsafe), "{readme:?}");
    assert_eq!(
        readme
            .lines()
            .filter(|line| line.trim_end() == "```")
            .count(),
        1,
        "one line closes the block"
    );
    assert_eq!(EvidenceDoc::parse(&readme).expect("it reads"), Some(doc));
}

#[test]
fn a_readme_with_no_block_of_evidence_gives_none() {
    let none = |readme: &str| {
        let read = EvidenceDoc::parse(readme).expect("no block is no error");
        assert_eq!(read, None, "{readme}");
    };
    none("");
    none("# A mod\n\nInstall it with a mod manager.\n");
    // A block of another language, and the format named inside it, are no evidence.
    none("```json\n{ \"format\": \"reforge.evidence/1\" }\n```\n");
    none("```text\n```reforge.evidence/1\n{}\n```\n");
    none("```\nnever closed\n");
}

#[test]
fn a_block_of_another_format_is_refused() {
    let later = "# A package\n\n```reforge.evidence/2\n{}\n```\n";
    let refused = EvidenceDoc::parse(later);
    let Err(EvidenceError::Format { found }) = refused else {
        panic!("another format is refused: {refused:?}");
    };
    assert_eq!(found.raw(), "reforge.evidence/2");

    // The block states this format and the document in it another.
    let inside = with_line(
        &repair_doc(),
        "\"format\": \"reforge.evidence/1\"",
        "\"format\": \"reforge.evidence/9\"",
    );
    let refused = EvidenceDoc::parse(&inside);
    let Err(EvidenceError::Format { found }) = refused else {
        panic!("another format is refused: {refused:?}");
    };
    assert_eq!(found.raw(), "reforge.evidence/9");

    // What an error keeps of a format's name is bounded, and prints cleaned.
    let long = format!("```reforge.evidence/\u{1b}{}\n{{}}\n```\n", "9".repeat(500));
    let Err(error @ EvidenceError::Format { .. }) = EvidenceDoc::parse(&long) else {
        panic!("another format is refused");
    };
    let message = error.to_string();
    assert!(message.len() < 200, "{message}");
    assert!(!message.chars().any(is_unsafe), "{message:?}");
}

#[test]
fn a_readme_over_the_cap_is_refused_before_it_is_read() {
    let mut readme = repair_doc().readme(None);
    readme.push_str(&" ".repeat(MAX_README_BYTES));
    let refused = EvidenceDoc::parse(&readme);
    let Err(EvidenceError::TooLarge { bytes }) = refused else {
        panic!("a README over the cap is refused: {refused:?}");
    };
    assert_eq!(bytes, readme.len());

    let mut at_the_cap = repair_doc().readme(None);
    at_the_cap.push_str(&" ".repeat(MAX_README_BYTES - at_the_cap.len()));
    assert_eq!(at_the_cap.len(), MAX_README_BYTES);
    assert!(matches!(EvidenceDoc::parse(&at_the_cap), Ok(Some(_))));
}

#[test]
fn a_block_that_is_no_document_is_an_error_and_no_panic() {
    let malformed = [
        "",
        "{",
        "not JSON",
        "[]",
        "{}",
        "{ \"format\": \"reforge.evidence/1\" }",
        "{ \"format\": 1 }",
        "\u{0}\u{1b}",
    ];
    for body in malformed {
        let refused = EvidenceDoc::parse(&readme_of(body));
        assert!(
            matches!(refused, Err(EvidenceError::Json(_))),
            "{body:?}: {refused:?}"
        );
    }
    // A value of another type, and a kind of run or of piece no document has.
    let wrong = [
        ("\"dated\": 3", "\"dated\": \"three\""),
        ("\"dated\": 3", "\"dated\": -3"),
        ("\"at\": 0", "\"at\": 65536"),
        ("\"run\": \"repair\"", "\"run\": \"rebuild\""),
        ("\"piece\": \"file\"", "\"piece\": \"folder\""),
    ];
    for (from, to) in wrong {
        let refused = EvidenceDoc::parse(&with_line(&repair_doc(), from, to));
        assert!(
            matches!(refused, Err(EvidenceError::Json(_))),
            "{to}: {refused:?}"
        );
    }
}

#[test]
fn a_field_no_reader_knows_is_ignored() {
    let added = with_line(
        &repair_doc(),
        "  \"run\": \"repair\",",
        "  \"run\": \"repair\",\n  \"added_later\": { \"any\": [1, 2] },",
    );
    assert_eq!(
        EvidenceDoc::parse(&added).expect("it reads"),
        Some(repair_doc())
    );
}

#[test]
fn a_block_that_is_not_closed_is_refused() {
    let readme = repair_doc().readme(None);
    let open = readme.trim_end().trim_end_matches("```");
    let refused = EvidenceDoc::parse(open);
    assert!(
        matches!(refused, Err(EvidenceError::Unclosed)),
        "{refused:?}"
    );
}

#[test]
fn a_second_block_of_evidence_is_refused() {
    let twice = format!("{}\n{}", repair_doc().readme(None), port_doc().readme(None));
    let refused = EvidenceDoc::parse(&twice);
    assert!(
        matches!(refused, Err(EvidenceError::Repeated)),
        "{refused:?}"
    );
}

#[test]
fn a_field_that_is_not_what_it_holds_is_refused_by_its_name() {
    let repair = repair_doc();
    let of_repair = |from: &str, to: &str| field_refused(&with_line(&repair, from, to));
    assert_eq!(of_repair(SHA256, "not a digest"), "archive.sha256");
    assert_eq!(of_repair(SHA256, &SHA256.to_uppercase()), "archive.sha256");
    assert_eq!(
        of_repair("\"build\": \"16.19.1\"", "\"build\": \"soon\""),
        "inputs.build"
    );
    assert_eq!(
        of_repair("\"newest_build\": \"16.19.1\"", "\"newest_build\": \"\""),
        "inputs.history.newest_build"
    );
    assert_eq!(
        of_repair("\"build\": \"16.18.1\"", "\"build\": \"16.x\""),
        "dating.not_before.build"
    );
    // A bound is a build of the history the document names.
    assert_eq!(of_repair("\"at\": 0", "\"at\": 2"), "dating.not_before.at");
    let mut no_history = repair_doc();
    no_history.inputs.history = None;
    assert_eq!(
        field_refused(&no_history.readme(None)),
        "dating.not_before.at"
    );
    // A file's hash is sixteen digits and an object's eight.
    assert_eq!(
        of_repair("0123456789abcdef", "89abcdef"),
        "dating.evidence.hash"
    );
    assert_eq!(
        field_refused(&with_line(&port_doc(), "89abcdef", "0123456789abcdef")),
        "dating.evidence.hash"
    );
    assert_eq!(of_repair(KEY, "00"), "dating.evidence.key");
    assert_eq!(
        of_repair("\"at_bound\": 1", "\"at_bound\": 4"),
        "dating.at_bound"
    );
    let of_port = |from: &str, to: &str| field_refused(&with_line(&port_doc(), from, to));
    assert_eq!(of_port("\"auto\"", "\"premium\""), "port.policy.target");
    assert_eq!(of_port("\"own-rig\"", "\"some\""), "port.policy.anim_reuse");
    // A slot the policy names is a target.
    let fixed = with_line(&port_doc(), "\"auto\"", "\"7\"");
    assert!(matches!(EvidenceDoc::parse(&fixed), Ok(Some(_))));
}

/// The report of a run, as `Report::to_json` writes one.
const REPORT: &str = "{\n  \"schema\": \"reforge.report/1\"\n}\n";

/// A document as a README with [`REPORT`] gives it back: its note says the report is
/// embedded (#ADR-0102).
fn noted(doc: EvidenceDoc) -> EvidenceDoc {
    EvidenceDoc {
        report: ReportNote {
            embedded: true,
            json_bytes: REPORT.len() as u64,
        },
        ..doc
    }
}

#[test]
fn a_readme_with_the_report_reads_back_to_the_evidence_and_the_report() {
    for doc in [repair_doc(), port_doc()] {
        let readme = doc.readme(Some(REPORT));
        let read = Readme::parse(&readme).expect("the README reads");
        let both = Readme {
            evidence: Some(noted(doc.clone())),
            report: Some(REPORT.into()),
        };
        assert_eq!(read, both);
        // The evidence alone reads as it is written.
        assert_eq!(
            EvidenceDoc::parse(&readme).expect("the evidence reads"),
            Some(noted(doc))
        );
        let crlf = readme.replace('\n', "\r\n");
        assert_eq!(Readme::parse(&crlf).expect("the README reads"), both);
    }
}

#[test]
fn the_report_follows_the_evidence_after_a_line_for_a_person() {
    let readme = repair_doc().readme(Some(REPORT));
    let body = encode_report(REPORT).expect("the report fits");
    let block = format!(
        "```\n\
         \n\
         The block below holds the run's report, compressed and encoded; a program reads it.\n\
         \n\
         ```{REPORT_FORMAT}\n\
         {body}\n\
         ```\n"
    );
    assert!(readme.ends_with(&block), "{readme}");
    let note = format!(
        "  \"report\": {{\n    \"embedded\": true,\n    \"json_bytes\": {}\n  }}\n}}\n```",
        REPORT.len()
    );
    assert!(readme.contains(&note), "{readme}");
    // A README with no report states no note: the document reads in an earlier program.
    assert!(!repair_doc().readme(None).contains("\"report\""));
    assert!(!noted(repair_doc()).readme(None).contains("\"report\""));
}

#[test]
fn a_report_with_no_evidence_beside_it_is_no_report() {
    let body = encode_report(REPORT).expect("the report fits");
    let alone = format!("# A package\n\n```{REPORT_FORMAT}\n{body}\n```\n");
    assert_eq!(Readme::parse(&alone).expect("it reads"), Readme::default());
    // A report of another format is a block of another language.
    let later = repair_doc()
        .readme(Some(REPORT))
        .replace(REPORT_FORMAT, "reforge.report+zstd/2");
    let read = Readme::parse(&later).expect("the README reads");
    assert_eq!(read.report, None);
    assert_eq!(read.evidence, Some(noted(repair_doc())));
}

#[test]
fn a_second_block_of_the_report_and_one_not_closed_are_refused() {
    let readme = repair_doc().readme(Some(REPORT));
    let at = readme
        .find(&format!("```{REPORT_FORMAT}"))
        .expect("the block");
    let twice = format!("{readme}\n{}", &readme[at..]);
    for refused in [
        Readme::parse(&twice).map(|_| ()),
        EvidenceDoc::parse(&twice).map(|_| ()),
    ] {
        assert!(
            matches!(refused, Err(EvidenceError::ReportRepeated)),
            "{refused:?}"
        );
    }
    let open = readme.trim_end().trim_end_matches("```");
    let refused = Readme::parse(open);
    assert!(
        matches!(refused, Err(EvidenceError::ReportUnclosed)),
        "{refused:?}"
    );
}

#[test]
fn a_report_that_does_not_decode_or_that_the_note_does_not_state_is_refused() {
    let readme = repair_doc().readme(Some(REPORT));
    let body = encode_report(REPORT).expect("the report fits");
    let broken = readme.replace(&body, "not base64 at all");
    let refused = Readme::parse(&broken);
    assert!(
        matches!(
            refused,
            Err(EvidenceError::Report(ReportBlockError::Base64(_)))
        ),
        "{refused:?}"
    );
    // The evidence alone does not decode the report.
    assert!(matches!(EvidenceDoc::parse(&broken), Ok(Some(_))));

    let other = encode_report("{}\n").expect("it fits");
    let swapped = readme.replace(&body, &other);
    let refused = Readme::parse(&swapped);
    assert!(
        matches!(refused, Err(EvidenceError::Field("report.json_bytes"))),
        "{refused:?}"
    );
    let unstated = readme.replace("\"embedded\": true", "\"embedded\": false");
    let refused = Readme::parse(&unstated);
    assert!(
        matches!(refused, Err(EvidenceError::Field("report.embedded"))),
        "{refused:?}"
    );
}

/// Bytes the game shipped at the base texture's path at the first build, large enough to
/// date a mod.
fn dated_texture() -> Vec<u8> {
    vec![b't'; 5000]
}

/// The history of [`reworked`], in which the game also held [`dated_texture`] at the base
/// texture's path at the first build.
fn history() -> MemoryHistory {
    let (history, _) = reworked();
    history
        .with_content(BASE_TEX, &dated_texture(), 0..=0)
        .with_path(BASE_TEX, 0..=1)
}

fn env_with(history: impl History + 'static) -> Env {
    env(skins::game(), history)
}

/// A mod with no bin that restores the model from before the rework: its own mesh, the old
/// texture at the base texture's path, a file at a path the rework dropped and one at a path
/// only the rework has.
fn restoring_mod() -> ModArchive {
    ModArchive::new()
        .file(WAD, BASE_MESH, b"the mod's own mesh".to_vec())
        .file(WAD, BASE_TEX, dated_texture())
        .file(WAD, OLD_TEX, b"the old model's texture".to_vec())
        .file(
            WAD,
            NEW_TEX,
            b"the mod's file at the reworked model's path".to_vec(),
        )
}

#[test]
fn the_evidence_of_a_repair_names_the_archive_the_inputs_the_dating_and_the_restoring_wad() {
    let env = env_with(history());
    let (_, reworks) = reworked();
    let set = FixupSet::standard();
    let options = LoadOptions::new().reworks(reworks);
    let archive = restoring_mod();
    let fix = FixRun::under(&env, &set, &archive, (options, &|_| ObjectSet::every()));
    let m = &fix.fixed;
    assert_eq!(m.restorations.len(), 1, "the WAD restores the old model");
    let Some(Piece::File(dated)) = fix.dating.evidence else {
        panic!("a file dates the mod: {:?}", fix.dating);
    };

    let doc = EvidenceDoc::of(Made::Repaired, m, &fix.dating, &env).fixups(set.ids());
    assert_eq!(doc.format, EVIDENCE_FORMAT);
    assert_eq!(doc.program.name, "reforge");
    assert_eq!(doc.program.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(doc.run, Made::Repaired);
    assert_eq!(doc.archive.sha256, ContentId::of(&archive.build()).hex());
    let inputs = EvidenceInputs {
        build: Some("16.19.1".into()),
        history: Some(EvidenceHistory {
            head: "c1".into(),
            newest_build: "16.19.1".into(),
            builds: 2,
        }),
    };
    assert_eq!(doc.inputs, inputs);
    let dating = EvidenceDating {
        not_before: Some(EvidenceBound {
            at: 0,
            build: "16.18.1".into(),
        }),
        evidence: Some(EvidencePiece {
            piece: PieceKind::File,
            hash: format!("{:016x}", path_hash(BASE_TEX).0),
            key: ContentId::of(&dated_texture()).hex(),
        }),
        dated: 1,
        at_bound: 1,
    };
    assert_eq!(m.files[dated].hash, path_hash(BASE_TEX));
    assert_eq!(doc.dating, dating);
    assert_eq!(doc.restored, ["test.wad.client"]);
    assert_eq!(doc.fixups, set.ids());
    assert!(!doc.fixups.is_empty());
    assert_eq!(doc.port, None);
    assert_eq!(
        EvidenceDoc::parse(&doc.readme(None)).expect("it reads"),
        Some(doc)
    );
}

#[test]
fn an_object_that_dates_the_mod_is_named_by_its_entry_hash_and_the_digest_of_its_bytes() {
    let archive = ModArchive::new().file(
        WAD,
        &skin_bin("test", 0),
        bin(vec![entry(SKIN0, &base_skin())]),
    );
    // The digest of the entry's bytes as the archive ships them, which the history is asked by.
    let raw = {
        let env = env_with(MemoryHistory::new(&["16.18.1", "16.19.1"]));
        let run = LoadRun::of(&env, LoadOptions::new(), &archive);
        let m = run.module();
        let mut objects = m.objects.values();
        let object = objects.find(|object| object.entry == name_hash(SKIN0));
        object.expect("the skin entry").raw
    };
    let history = MemoryHistory::new(&["16.18.1", "16.19.1"]).with_object(
        ObjectKey::from_sha256(&raw.0),
        NormKey(1),
        (SKIN0, "SkinCharacterDataProperties", &skin_bin("test", 0)),
        1..=1,
    );
    let env = env_with(history);
    let fix = FixRun::of(&env, &FixupSet::standard(), &archive);
    assert!(matches!(fix.dating.evidence, Some(Piece::Object(_))));

    let doc = EvidenceDoc::of(Made::Repaired, &fix.fixed, &fix.dating, &env);
    let piece = EvidencePiece {
        piece: PieceKind::Object,
        hash: format!("{:08x}", name_hash(SKIN0).0),
        key: raw.hex(),
    };
    assert_eq!(doc.dating.evidence, Some(piece));
    let bound = EvidenceBound {
        at: 1,
        build: "16.19.1".into(),
    };
    assert_eq!(doc.dating.not_before, Some(bound));
    assert_eq!(doc.restored, Vec::<String>::new());
    assert_eq!(
        EvidenceDoc::parse(&doc.readme(None)).expect("it reads"),
        Some(doc)
    );
}

#[test]
fn a_mod_no_history_dates_has_a_dating_with_no_bound_and_no_piece() {
    let env = env_with(reforge::history::NoHistory);
    let archive = restoring_mod();
    let fix = FixRun::of(&env, &FixupSet::standard(), &archive);
    let doc = EvidenceDoc::of(Made::Repaired, &fix.fixed, &fix.dating, &env);
    assert_eq!(doc.inputs.history, None);
    let dating = EvidenceDating {
        not_before: None,
        evidence: None,
        dated: 0,
        at_bound: 0,
    };
    assert_eq!(doc.dating, dating);
    assert_eq!(doc.restored, Vec::<String>::new());
    assert_eq!(
        EvidenceDoc::parse(&doc.readme(None)).expect("it reads"),
        Some(doc)
    );
}

#[test]
fn the_evidence_of_a_port_holds_the_policy_and_each_unit_that_ships_with_its_target() {
    let env = env_with(reforge::history::NoHistory);
    let mut files = own_files("a");
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
    ];
    files.push((skin_bin("test", 0), bin(objects)));
    let run = PortRun::of(&env, &archive(&files));
    let m = &run.facts.fix.fixed;
    assert_eq!(run.port.units.len(), 1, "{}", run.dump());
    let unit = &run.port.units[0];
    let decision = &run.targets.decisions[&unit.unit];

    let doc = EvidenceDoc::of(Made::Ported, m, &run.facts.fix.dating, &env).ported(
        &run.targets,
        &run.port,
        m,
    );
    assert_eq!(doc.run, Made::Ported);
    let port = EvidencePort {
        policy: EvidencePolicy {
            target: "auto".into(),
            anim_reuse: "own-rig".into(),
        },
        units: vec![EvidenceUnit {
            character: "test".into(),
            source: 0,
            target: unit.target.0,
            reason: decision.reason.as_str().into(),
        }],
    };
    assert_eq!(doc.port, Some(port));
    assert_eq!(
        EvidenceDoc::parse(&doc.readme(None)).expect("it reads"),
        Some(doc)
    );
}
