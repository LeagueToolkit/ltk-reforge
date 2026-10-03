//! The codes the history raises are the ones `docs/design/CODES.md` lists for it, each with the
//! severity and the channel the table gives.

use std::collections::BTreeMap;

use fs_err as fs;
use reforge::diag::{Channel, Diag, Diagnostics, Input, Render, Stage, Subject};
use reforge::game::Build;
use reforge::history::{CODES, HistoryAbsent, IndexMissing, IndexStale, InstallNewer};

/// The code and severity of every row of the tables in `CODES.md`.
fn documented() -> BTreeMap<String, String> {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design/CODES.md");
    let text = fs::read_to_string(file).unwrap();
    text.lines()
        .filter_map(|line| {
            let mut cells = line.strip_prefix("| `")?.split('|');
            let code = cells.next()?.trim().trim_end_matches('`');
            let severity = cells.next()?.trim();
            Some((code.to_string(), severity.to_string()))
        })
        .collect()
}

#[test]
fn every_code_is_in_the_codes_table_with_its_severity() {
    let documented = documented();
    for code in CODES {
        assert_eq!(
            documented.get(code.name).map(String::as_str),
            Some(code.severity.as_str()),
            "{code}"
        );
        assert_eq!(
            code.channel,
            Channel::Run,
            "{code}: the run's other inputs speak on `run`"
        );
    }
}

#[test]
fn every_code_of_the_history_is_raised_here() {
    let declared = [
        HistoryAbsent::CODE,
        IndexMissing::CODE,
        IndexStale::CODE,
        InstallNewer::CODE,
    ];
    assert_eq!(CODES, declared);
    let mut ours: Vec<&str> = CODES.iter().map(|code| code.name).collect();
    ours.sort_unstable();
    let documented = documented();
    let theirs: Vec<&str> = documented
        .keys()
        .map(String::as_str)
        .filter(|code| code.starts_with("history."))
        .collect();
    assert_eq!(ours, theirs);
}

#[test]
fn each_finding_is_about_the_history_and_says_what_it_found() {
    let mut log = Diagnostics::new();
    let mut sink = log.at(Stage::Read);
    sink.raise(HistoryAbsent);
    sink.raise(IndexMissing {
        index: "data/index".into(),
        reason: "no stamp".into(),
    });
    sink.raise(IndexStale {
        index_head: "0123456789abcdef0123".into(),
        repo_head: "fedcba9876543210fedc".into(),
    });
    sink.raise(InstallNewer {
        install: Build::parse("16.20.1").unwrap(),
        newest: Build::parse("16.19.8207193").unwrap(),
    });
    let cx = Render::plain();
    let messages: Vec<String> = log.iter().map(|d| d.message(&cx)).collect();
    assert_eq!(
        messages,
        [
            "no history: files, objects and clips are judged against the install only",
            "the index at data/index does not open: no stamp; judged against the install only",
            "the index is of 0123456789ab, the history is at fedcba987654; append to the index \
             or rebuild it",
            "the install is 16.20.1, the history ends at 16.19.8207193: bytes shipped since \
             pass as the mod's own",
        ]
    );
    assert!(
        log.iter()
            .all(|d| d.subject == Subject::Input(Input::History))
    );
}
