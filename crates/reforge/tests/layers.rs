//! A module uses only the modules of earlier stages (#ADR-0001). The build does not enforce it,
//! so this reads every `crate::` path in the sources.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The modules of the crate, each using only the ones before it.
/// `docs/design/ARCHITECTURE.md`, "Crates and modules".
const ORDER: &[&str] = &[
    "model", "diag", "archive", "game", "history", "env", "load", "fixups", "facts", "port",
    "package", "report", "porter",
];

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs_err::read_dir(dir)
        .expect("the source directory reads")
        .map(|e| e.expect("an entry reads").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The modules a source text names through `crate::`: `crate::model::x`, and each first name of
/// `crate::{model::x, diag::y}`.
fn used(text: &str) -> BTreeSet<String> {
    let name = |s: &str| {
        s.chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
    };
    let mut out = BTreeSet::new();
    for (at, _) in text.match_indices("crate::") {
        let rest = &text[at + "crate::".len()..];
        if let Some(group) = rest.strip_prefix('{') {
            let end = group.find(';').unwrap_or(group.len());
            let mut depth = 0usize;
            let mut start = true;
            for (i, c) in group[..end].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' if depth == 0 => break,
                    '}' => depth -= 1,
                    ',' if depth == 0 => start = true,
                    c if start && depth == 0 && !c.is_whitespace() => {
                        out.insert(name(&group[i..]));
                        start = false;
                    }
                    _ => {}
                }
            }
        } else {
            out.insert(name(rest));
        }
    }
    out.remove("");
    out
}

#[test]
fn a_module_uses_only_the_modules_before_it() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    sources(&src, &mut files);
    let mut wrong = Vec::new();
    let mut seen = BTreeSet::new();
    for file in files {
        let relative = file.strip_prefix(&src).expect("a source is under src");
        let Some(module) = relative
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
        else {
            continue;
        };
        let module = module.trim_end_matches(".rs");
        if module == "lib" {
            continue;
        }
        seen.insert(module.to_string());
        let Some(place) = ORDER.iter().position(|m| *m == module) else {
            wrong.push(format!("{module} is no module of the order in this test"));
            continue;
        };
        let text = fs_err::read_to_string(&file).expect("a source reads");
        for other in used(&text) {
            match ORDER.iter().position(|m| *m == other) {
                Some(p) if p > place => {
                    wrong.push(format!(
                        "{} uses {other}, a later module than {module}",
                        relative.display()
                    ));
                }
                Some(_) => {}
                None => wrong.push(format!(
                    "{} uses crate::{other}, which is no module",
                    relative.display()
                )),
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(
        seen.contains("model") && seen.contains("diag"),
        "the walk found the modules: {seen:?}"
    );
}

/// #ADR-0080: the facts and the decision ask the game typed questions (`GameSource`), and no
/// question gives a chunk's bytes. An install's are read by `GameView`, which answers them.
#[test]
fn facts_and_port_read_no_game_chunk() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut wrong = Vec::new();
    for module in ["facts", "port"] {
        let mut files = Vec::new();
        sources(&src.join(module), &mut files);
        assert!(!files.is_empty(), "{module} has sources");
        for file in files {
            let text = fs_err::read_to_string(&file).expect("a source reads");
            // The code alone: a comment may name the call.
            let code = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"));
            let code: String = code.collect::<Vec<_>>().join("\n");
            let spaceless: String = code.split_whitespace().collect();
            for call in ["Game::read", "game().read(", "game.read(", ".game.read("] {
                if spaceless.contains(call) {
                    wrong.push(format!("{} calls {call}", file.display()));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_reader_of_crate_paths_finds_plain_and_grouped_uses() {
    let text = "use crate::model::{Slot, WadHash};\nuse crate::{diag::Sink, model::entity::Mod};\nlet x = crate::game::Install::open();";
    let found: Vec<String> = used(text).into_iter().collect();
    assert_eq!(found, ["diag", "game", "model"]);
}
