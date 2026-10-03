//! The catalogue of diagnostic codes: each code is declared once, and `docs/design/CODES.md`
//! lists it with the severity it is declared with.

use std::collections::{BTreeMap, BTreeSet};

use fs_err as fs;

/// The code and severity of every row of the tables in `CODES.md`.
fn documented() -> BTreeMap<String, String> {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/design/CODES.md");
    let text = fs::read_to_string(file).expect("CODES.md reads");
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
fn a_code_is_declared_once() {
    let mut seen = BTreeSet::new();
    for code in reforge::codes() {
        assert!(seen.insert(code.name), "{code} is declared twice");
    }
    assert!(!seen.is_empty(), "the catalogue holds the modules' codes");
}

#[test]
fn every_declared_code_is_documented_with_its_severity() {
    let documented = documented();
    for code in reforge::codes() {
        assert_eq!(
            documented.get(code.name).map(String::as_str),
            Some(code.severity.as_str()),
            "{code}"
        );
    }
}

#[test]
fn a_code_is_its_area_and_its_finding() {
    for code in reforge::codes() {
        let (area, finding) = code
            .name
            .split_once('.')
            .unwrap_or_else(|| panic!("{code} has no area"));
        let plain = |part: &str| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        };
        assert!(plain(area) && plain(finding), "{code}");
        assert_eq!(code.area(), area);
    }
}
