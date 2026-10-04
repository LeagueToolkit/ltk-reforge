//! Every edge case is named in a test (#ADR-0045). The build does not enforce it, so this
//! reads the sources and the test files of the workspace.
//!
//! An edge case is a diagnostic code of `reforge::codes()`, a variant of a closed set listed
//! in [`SETS`], a built-in fixup of `FixupSet::catalog()`, or a record the sources cite. It
//! is named when a file under a crate's `tests/` directory, this one left out, holds:
//!
//! - **a diagnostic:** its type as a type argument, `<Type>` or `<&Type>`, or `Type::CODE`,
//!   or its code in quotes;
//! - **a variant:** `Enum::Variant`, in a test file of the stage that decides it (the file
//!   names the set lists);
//! - **a fixup:** its id in quotes, in a `fixups_` or `load_` file;
//! - **a record:** its code, `#GAME-CHANGE-0009`, or `Record::game_change(9)`.
//!
//! A comment line names a record and nothing else. The rule finds a name, not an assertion:
//! that the test asserts the case is the reviewer's to hold.
//!
//! [`GAPS`] lists the cases no test names. The list only shrinks: the test fails for a case
//! that is neither named nor listed, and for a listed case that a test names.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use reforge::fixups::FixupSet;

/// A closed set: an enum whose variants each change an outcome.
struct Set {
    /// The set as [`GAPS`] spells it, before `::Variant`.
    label: &'static str,
    /// The source that declares the enum, under `crates/reforge/src/`.
    source: &'static str,
    /// What the name of a test file that may name its variants starts with. Two enums of one
    /// name are told apart by it.
    tests: &'static [&'static str],
}

const fn set(label: &'static str, source: &'static str, tests: &'static [&'static str]) -> Set {
    Set {
        label,
        source,
        tests,
    }
}

/// The closed sets, in the order of the stages.
const SETS: &[Set] = &[
    set("archive::PackedError", "archive/packed.rs", &["archive"]),
    set("model::FileSource", "model/entity.rs", &["load_"]),
    set("model::FileUse", "model/entity.rs", &["load_"]),
    set("model::MadeBy", "model/entity.rs", &["load_"]),
    set("model::WadRole", "model/entity.rs", &["load_"]),
    set("model::WadLayout", "model/entity.rs", &["archive", "load_"]),
    set("model::CharKind", "model/entity.rs", &["load_"]),
    set("model::LinkTo", "model/entity.rs", &["load_"]),
    set("load::LoadError", "load/mod.rs", &["load_"]),
    set("load::Fault", "load/mod.rs", &["load_"]),
    set("load::legacy::Conversion", "load/legacy.rs", &["load_"]),
    set("load::legacy::ConvertError", "load/legacy.rs", &["load_"]),
    set("load::moved::Step", "load/moved.rs", &["load_"]),
    set("load::inventory::Finding", "load/inventory.rs", &["load_"]),
    set("load::provenance::AtPath", "load/provenance.rs", &["load_"]),
    set(
        "load::provenance::BytesOrigin",
        "load/provenance.rs",
        &["load_"],
    ),
    set("fixups::After", "fixups/changes.rs", &["fixups_"]),
    set("fixups::Outcome", "fixups/changes.rs", &["fixups_"]),
    set(
        "fixups::conform::Conversion",
        "fixups/conform/conversion.rs",
        &["fixups_"],
    ),
    set("fixups::FixError", "fixups/set.rs", &["fixups_"]),
    set("fixups::SelectionError", "fixups/set.rs", &["fixups_"]),
    set("facts::FactsError", "facts/mod.rs", &["facts_"]),
    set("game::SkinClass", "game/catalog.rs", &["facts_"]),
    set("facts::CopyKind", "facts/copies.rs", &["facts_"]),
    set("facts::UnitRole", "facts/copies.rs", &["facts_"]),
    set("facts::ObjectOrigin", "facts/objects.rs", &["facts_"]),
    set("facts::Via", "facts/objects.rs", &["facts_"]),
    set("facts::origin::Verdict", "facts/origin.rs", &["facts_"]),
    set(
        "facts::origin::Weight",
        "facts/origin.rs",
        &["facts_origin"],
    ),
    set(
        "facts::origin::Tallied",
        "facts/origin.rs",
        &["facts_origin"],
    ),
    set(
        "facts::origin::Aspect",
        "facts/origin.rs",
        &["facts_origin"],
    ),
    set("facts::origin::Basis", "facts/origin.rs", &["facts_origin"]),
    set("facts::rig::Source", "facts/rig.rs", &["facts_"]),
    set("facts::rig::Visibility", "facts/rig.rs", &["facts_"]),
    set("facts::rig::GraphFrom", "facts/rig.rs", &["facts_"]),
    set(
        "facts::judge::Aspect",
        "facts/judge.rs",
        &["facts_rig", "facts_judge", "port_decide"],
    ),
    set(
        "facts::judge::Outcome",
        "facts/judge.rs",
        &["facts_", "port_decide"],
    ),
    set(
        "facts::judge::Why",
        "facts/judge.rs",
        &["facts_", "port_decide"],
    ),
    set(
        "facts::judge::ClipBlock",
        "facts/judge.rs",
        &["port_decide"],
    ),
    set(
        "facts::rework::Made",
        "facts/rework.rs",
        &["facts_", "port_decide"],
    ),
    set(
        "facts::ClipOrigin",
        "facts/sounds.rs",
        &["facts_", "port_audio"],
    ),
    set(
        "model::BankRole",
        "model/bank.rs",
        &["facts_", "port_audio"],
    ),
    set("port::TargetPolicy", "port/mod.rs", &["port_"]),
    set("port::AnimReuse", "port/mod.rs", &["port_"]),
    set("port::Reason", "port/decide.rs", &["port_"]),
    set("port::Considered", "port/decide.rs", &["port_"]),
    set("port::PlanError", "port/plan.rs", &["port_"]),
    set("port::Left", "port/shipping.rs", &["port_"]),
    set("port::Fate", "port/reach.rs", &["port_"]),
    set("port::FileVerdict", "port/refs.rs", &["port_"]),
    set("port::Carry", "port/findings.rs", &["port_"]),
    set(
        "port::NotCarried",
        "port/findings.rs",
        &["port_", "package_port"],
    ),
    set("model::Layer", "model/layer.rs", &["port_", "package_"]),
    set("package::Origin", "package/mod.rs", &["package_"]),
    set("package::BinWrite", "package/mod.rs", &["package_"]),
    set("package::LeftOut", "package/wired.rs", &["package_"]),
    set("package::BuildError", "package/inplace.rs", &["package_"]),
    set("package::TexRefusal", "package/texture.rs", &["package_"]),
    set(
        "package::overlay::Outcome",
        "package/overlay.rs",
        &["package_"],
    ),
    set(
        "package::RoundTripProblem",
        "package/findings.rs",
        &["package_"],
    ),
    set("package::Coverage", "package/findings.rs", &["package_"]),
    set("package::PieceKind", "package/evidence.rs", &["package_"]),
    set(
        "package::EvidenceError",
        "package/evidence.rs",
        &["package_"],
    ),
    set("package::WriteError", "package/write.rs", &["package_"]),
    set("porter::RunError", "porter.rs", &["porter"]),
];

/// The cases no test names, sorted, each with why.
const GAPS: &[(&str, &str)] = &[
    (
        "load::legacy::ConvertError::Write",
        "the SCB is written into memory, where the writer does not fail",
    ),
    (
        "package::TexRefusal::NotReadBack",
        "a TEX that passes the refusals before it reads back with every mip",
    ),
    (
        "package::WriteError::Config",
        "the config a writer stages serialises, and reads back as staged",
    ),
    (
        "porter::RunError::Again",
        "a package's chunks, one per WAD and path, pack as a zip in memory",
    ),
];

/// The `.rs` files under a directory, sorted.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs_err::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.map(|e| e.expect("an entry reads").path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn crates_dir() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates = manifest.parent().expect("the crate is in crates/");
    crates.to_path_buf()
}

/// The files of one kind of directory (`src` or `tests`) of every crate, each with its text.
/// The testkit has no edge case, and this file names every gap.
fn texts(kind: &str) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    let mut crates: Vec<PathBuf> = fs_err::read_dir(crates_dir())
        .expect("crates/ reads")
        .map(|e| e.expect("an entry reads").path())
        .collect();
    crates.sort();
    for dir in crates
        .iter()
        .filter(|dir| !dir.ends_with("reforge-testkit"))
    {
        sources(&dir.join(kind), &mut files);
    }
    files.retain(|file| !(kind == "tests" && file.ends_with("cases.rs")));
    let read = |file: PathBuf| {
        let text = fs_err::read_to_string(&file).expect("a source reads");
        (file, text)
    };
    files.into_iter().map(read).collect()
}

/// A text without its comment lines.
fn code_of(text: &str) -> String {
    let code = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"));
    code.collect::<Vec<_>>().join("\n")
}

fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Whether a text holds a path as a whole: `Left::Copy`, and not `Left::CopyOf` or
/// `NotLeft::Copy`.
fn holds(text: &str, path: &str) -> bool {
    text.match_indices(path).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + path.len()..].chars().next();
        !before.is_some_and(is_name) && !after.is_some_and(is_name)
    })
}

/// The variants of `pub enum <name>` in a source: the name that starts each item of the
/// enum's body.
fn variants(text: &str, name: &str) -> Vec<String> {
    let head = format!("pub enum {name} {{");
    let Some(at) = text.find(&head) else {
        return Vec::new();
    };
    let mut depth = 0usize;
    let mut out = Vec::new();
    for line in text[at + head.len()..].lines() {
        let line = line.trim();
        // An attribute's text may hold a brace of its own.
        let attribute = line.starts_with("#[") && line.ends_with(']');
        if line.starts_with("//") || (depth == 0 && attribute) {
            continue;
        }
        if depth == 0 {
            if line.starts_with('}') {
                break;
            }
            let name: String = line.chars().take_while(|c| is_name(*c)).collect();
            if !name.is_empty() {
                out.push(name);
            }
        }
        for c in line.chars() {
            match c {
                '{' | '(' => depth += 1,
                '}' | ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    out
}

/// The type of each diagnostic code a source declares: `impl Diag for <Type>` with the code
/// its `CODE` is given, and a row of a macro that declares findings, `<Type>, "<code>",`.
fn diagnostics(text: &str, out: &mut BTreeMap<String, String>) {
    for (at, head) in text.match_indices("impl Diag for ") {
        let rest = &text[at + head.len()..];
        let name: String = rest.chars().take_while(|c| is_name(*c)).collect();
        let Some(code) = rest.find("const CODE").map(|at| &rest[at..]) else {
            continue;
        };
        let end = code.find(';').unwrap_or(code.len());
        let mut quoted = code[..end].split('"');
        if let (false, Some(code)) = (name.is_empty(), quoted.nth(1)) {
            out.insert(code.to_string(), name);
        }
    }
    for line in text.lines() {
        let mut cells = line.trim().split(", ");
        let (Some(name), Some(code)) = (cells.next(), cells.next()) else {
            continue;
        };
        let typed = name.starts_with(|c: char| c.is_ascii_uppercase()) && name.chars().all(is_name);
        let code = code.strip_prefix('"').and_then(|c| c.strip_suffix('"'));
        if let (true, Some(code)) = (typed, code)
            && code.contains('.')
        {
            out.insert(code.to_string(), name.to_string());
        }
    }
}

const SERIES: &[(&str, &str)] = &[
    ("GAME-CHANGE", "game_change"),
    ("GAME-BEHAVIOR", "game_behavior"),
    ("MOD-ARTIFACT", "mod_artifact"),
];

/// The records a text cites, as `#GAME-CHANGE-0009`: by the code, or by `Record::game_change(9)`.
fn records(text: &str, out: &mut BTreeSet<String>) {
    for (series, constructor) in SERIES {
        let code = format!("#{series}-");
        for (at, _) in text.match_indices(&code) {
            let digits: String = text[at + code.len()..].chars().take(4).collect();
            if digits.len() == 4 && digits.chars().all(|c| c.is_ascii_digit()) {
                out.insert(format!("{code}{digits}"));
            }
        }
        let call = format!("Record::{constructor}(");
        for (at, _) in text.match_indices(&call) {
            let rest = &text[at + call.len()..];
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(number) = digits.parse::<u32>() {
                out.insert(format!("{code}{number:04}"));
            }
        }
    }
}

/// Every edge case of the workspace, each with whether a test names it.
fn cases() -> BTreeMap<String, bool> {
    let sources = texts("src");
    let tests = texts("tests");
    let stem = |file: &Path| {
        let name = file.file_name().and_then(|name| name.to_str());
        name.unwrap_or_default().to_string()
    };
    let code: Vec<(String, String)> = tests
        .iter()
        .map(|(file, text)| (stem(file), code_of(text)))
        .collect();
    let mut out = BTreeMap::new();

    let mut types = BTreeMap::new();
    for (_, text) in &sources {
        diagnostics(text, &mut types);
    }
    for declared in reforge::codes() {
        let name = types
            .get(declared.name)
            .unwrap_or_else(|| panic!("no source declares the type of {declared}"));
        let forms = [
            format!("<{name}>"),
            format!("<&{name}>"),
            format!("{name}::CODE"),
            format!("\"{}\"", declared.name),
        ];
        let named = code
            .iter()
            .any(|(_, text)| forms.iter().any(|form| text.contains(form)));
        out.insert(declared.name.to_string(), named);
    }

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for set in SETS {
        let text = fs_err::read_to_string(src.join(set.source)).expect("a set's source reads");
        let name = set.label.rsplit("::").next().unwrap_or(set.label);
        let variants = variants(&text, name);
        assert!(!variants.is_empty(), "{} declares no {name}", set.source);
        for variant in variants {
            let path = format!("{name}::{variant}");
            let named = code.iter().any(|(file, text)| {
                set.tests.iter().any(|prefix| file.starts_with(prefix)) && holds(text, &path)
            });
            out.insert(format!("{}::{variant}", set.label), named);
        }
    }

    for info in FixupSet::catalog() {
        let quoted = format!("\"{}\"", info.id);
        let named = code.iter().any(|(file, text)| {
            (file.starts_with("fixups_") || file.starts_with("load_")) && text.contains(&quoted)
        });
        out.insert(format!("fixup {}", info.id), named);
    }

    let (mut cited, mut tested) = (BTreeSet::new(), BTreeSet::new());
    for (_, text) in &sources {
        records(text, &mut cited);
    }
    for (_, text) in &tests {
        records(text, &mut tested);
    }
    for record in cited {
        let named = tested.contains(&record);
        out.insert(record, named);
    }
    out
}

#[test]
fn every_edge_case_is_named_in_a_test_or_listed_as_a_gap() {
    let cases = cases();
    let listed: Vec<&str> = GAPS.iter().map(|(case, _)| *case).collect();
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(listed, sorted, "the gaps are sorted, each once");
    for (case, why) in GAPS {
        assert!(!why.is_empty(), "the gap {case} says why");
    }

    let listed: BTreeSet<&str> = listed.into_iter().collect();
    let unnamed = cases.iter().filter(|(_, named)| !**named);
    let unnamed: BTreeSet<&str> = unnamed.map(|(case, _)| case.as_str()).collect();
    let missing: Vec<&&str> = unnamed.difference(&listed).collect();
    assert!(
        missing.is_empty(),
        "no test names these edge cases, and the gaps do not list them:\n{missing:#?}"
    );
    let stale: Vec<&&str> = listed.difference(&unnamed).collect();
    assert!(
        stale.is_empty(),
        "these gaps are named by a test, or are no edge case: take them off the list\n{stale:#?}"
    );
    assert!(
        cases.len() > 300,
        "the walk found the cases: {}",
        cases.len()
    );
}

#[test]
fn the_reader_of_an_enum_finds_its_variants() {
    let text = "/// Why.\n#[derive(Debug)]\npub enum Left {\n    /// A copy.\n    Copy {\n        /// Of.\n        of: UnitId,\n    },\n    #[error(\"no {0}\")]\n    PreRework,\n    Fixed(Slot),\n    Last\n}\npub enum Other {\n    Not,\n}";
    assert_eq!(
        variants(text, "Left"),
        ["Copy", "PreRework", "Fixed", "Last"]
    );
    assert_eq!(variants(text, "Other"), ["Not"]);
    assert!(variants(text, "Absent").is_empty());
}

#[test]
fn the_reader_of_diagnostics_finds_a_type_by_its_impl_and_by_a_macro_row() {
    let text = "impl Diag for StaleFiles {\n    const CODE: Code =\n        Code::new(\"file.stale\", Channel::Input, Severity::Note);\n}\nfinding! {\n    /// Programs.\n    Executables, \"inventory.executable\", Warning, None,\n    \"executable\", \", not examined\"\n}";
    let mut found = BTreeMap::new();
    diagnostics(text, &mut found);
    let found: Vec<(&str, &str)> = found
        .iter()
        .map(|(code, name)| (code.as_str(), name.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            ("file.stale", "StaleFiles"),
            ("inventory.executable", "Executables")
        ]
    );
}

#[test]
fn the_reader_of_records_finds_a_code_and_a_constructor() {
    let text = "// #GAME-CHANGE-0009: a tag.\nconst R: Record = Record::mod_artifact(4);\n#ADR-0045 #GAME-BEHAVIOR-12";
    let mut found = BTreeSet::new();
    records(text, &mut found);
    let found: Vec<&str> = found.iter().map(String::as_str).collect();
    assert_eq!(found, ["#GAME-CHANGE-0009", "#MOD-ARTIFACT-0004"]);
}

#[test]
fn a_path_is_held_as_a_whole() {
    assert!(holds("assert_eq!(why, Left::Copy { of });", "Left::Copy"));
    assert!(holds("judge::Outcome::Fits)", "Outcome::Fits"));
    assert!(!holds("Outcome::FitsWithFix", "Outcome::Fits"));
    assert!(!holds("NotLeft::Copy", "Left::Copy"));
}
