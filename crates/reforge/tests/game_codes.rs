//! The codes the game side raises are the ones `docs/design/CODES.md` lists for the install, the
//! names and the schema, each with the severity and the channel the table gives.

use std::collections::BTreeMap;

use fs_err as fs;
use reforge::diag::{Channel, Diag};
use reforge::game::{
    CODES, MimirAbsent, SchemaAbsent, SchemaInstallNewer, SchemaUnreadable, VersionUnknown,
    WadsUnreadable,
};

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
fn every_code_of_the_install_the_names_and_the_schema_is_raised_here() {
    let ours: Vec<&str> = CODES.iter().map(|code| code.name).collect();
    let documented = documented();
    let theirs: Vec<&str> = documented
        .keys()
        .map(String::as_str)
        .filter(|code| {
            ["install.", "names.", "schema."]
                .iter()
                .any(|area| code.starts_with(area))
        })
        .collect();
    let mut sorted = ours.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, theirs);
}

#[test]
fn each_finding_has_a_code_of_its_own() {
    let declared = [
        WadsUnreadable::CODE,
        VersionUnknown::CODE,
        MimirAbsent::CODE,
        SchemaAbsent::CODE,
        SchemaUnreadable::CODE,
        SchemaInstallNewer::CODE,
    ];
    assert_eq!(CODES, declared);
    let names: Vec<&str> = declared.iter().map(|code| code.name).collect();
    assert_eq!(
        names,
        [
            "install.wad-unreadable",
            "install.version-unknown",
            "names.mimir-absent",
            "schema.absent",
            "schema.unreadable",
            "schema.install-newer",
        ]
    );
}
