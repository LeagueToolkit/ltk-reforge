//! The records bundled at build time, against the folders they come from.

use std::path::Path;

use fs_err as fs;
use reforge_app::records::{self, Series};

const FOLDERS: [&str; 4] = ["adr", "game-change", "game-behavior", "mod-artifact"];

#[test]
fn the_bundle_holds_every_file_of_the_four_folders() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let files: usize = FOLDERS
        .iter()
        .map(|folder| fs::read_dir(docs.join(folder)).unwrap().count())
        .sum();
    assert_eq!(records::list().len(), files);
}

#[test]
fn every_head_has_a_title_and_its_series_in_its_code() {
    for head in records::list() {
        assert!(!head.title.is_empty(), "{} has no title", head.code);
        assert_eq!(
            head.status.is_some(),
            head.series == Series::Adr,
            "{} has a status of {:?}",
            head.code,
            head.status
        );
        let number = format!("-{:04}", head.number);
        assert!(
            head.code.ends_with(&number),
            "{} is not {number}",
            head.code
        );
    }
}

#[test]
fn an_adr_cites_what_it_names_and_lists_what_names_it() {
    let view = records::record("#ADR-0091").expect("the bundle holds #ADR-0091");
    assert_eq!(view.head.code, "#ADR-0091");
    assert!(view.head.status.as_deref().unwrap().starts_with("Accepted"));
    assert!(
        view.cites.iter().any(|code| code == "#ADR-0046"),
        "{:?}",
        view.cites
    );
    assert!(
        !view.cites.iter().any(|code| code == "#ADR-0091"),
        "{:?}",
        view.cites
    );
    assert!(!view.cited_by.is_empty());
    let sorted = {
        let mut cites = view.cites.clone();
        cites.sort();
        cites
    };
    assert_eq!(view.cites, sorted);
}

#[test]
fn a_code_opens_with_or_without_its_hash() {
    assert_eq!(records::record("ADR-0091"), records::record("#ADR-0091"));
}

#[test]
fn an_unknown_code_answers_none() {
    assert_eq!(records::record("#ADR-9999"), None);
    assert_eq!(records::record("not a code"), None);
}
