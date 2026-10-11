use std::env;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fs_err as fs;

/// The four series of records under `docs/`, in the order the bundle lists them: each folder
/// and the code its records are cited by (`docs/agents/records.md`).
const SERIES: [(&str, &str); 4] = [
    ("adr", "ADR"),
    ("game-change", "GAME-CHANGE"),
    ("game-behavior", "GAME-BEHAVIOR"),
    ("mod-artifact", "MOD-ARTIFACT"),
];

fn main() {
    bundle_records();

    // tauri-build embeds its manifest in the app's binary alone. A test binary without the
    // common controls it names does not start on Windows (STATUS_ENTRYPOINT_NOT_FOUND), so the
    // linker embeds the same manifest in every binary of the crate.
    let msvc = env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|target| target == "msvc");
    let mut windows = tauri_build::WindowsAttributes::new();
    if msvc {
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    let attributes = tauri_build::Attributes::new().windows_attributes(windows);
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri-build failed: {error:#}");
    }
}

/// Writes `$OUT_DIR/records.rs`: every record of the four series, in series order then number
/// order, its text included from its file (#ADR-0091). A file that is not shaped as a record
/// fails the build, since the app would list it wrong.
fn bundle_records() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let mut generated = String::from("static RECORDS: &[Bundled] = &[\n");
    for (folder, prefix) in SERIES {
        let dir = docs.join(folder);
        println!("cargo:rerun-if-changed={}", dir.display());
        for path in files_in(&dir) {
            println!("cargo:rerun-if-changed={}", path.display());
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            let number = record_number(name).unwrap_or_else(|| {
                panic!(
                    "{}: a record's file is named NNNN-<slug>.md (docs/agents/records.md)",
                    path.display()
                )
            });
            let code = format!("{prefix}-{number}");
            let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{error}"));
            let title = record_title(&text, &code).unwrap_or_else(|| {
                panic!(
                    "{}: a record's heading is `# {code}: <title>` (docs/agents/records.md)",
                    path.display()
                )
            });
            let source = path.display().to_string();
            writeln!(
                generated,
                "    Bundled {{ code: {code:?}, title: {title:?}, body: include_str!({source:?}) }},"
            )
            .expect("a write to a String does not fail");
        }
    }
    generated.push_str("];\n");
    let out = env::var("OUT_DIR").expect("cargo sets OUT_DIR for a build script");
    fs::write(Path::new(&out).join("records.rs"), generated)
        .unwrap_or_else(|error| panic!("{error}"));
}

/// The files of a folder, by name.
fn files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{error}"))
        .map(|entry| entry.unwrap_or_else(|error| panic!("{error}")).path())
        .collect();
    files.sort();
    files
}

/// The number of a record's file name: `0005` of `0005-<slug>.md`.
fn record_number(name: &str) -> Option<&str> {
    let (number, slug) = name.strip_suffix(".md")?.split_once('-')?;
    let number_fits = number.len() == 4 && number.bytes().all(|byte| byte.is_ascii_digit());
    let slug_fits = !slug.is_empty()
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    (number_fits && slug_fits).then_some(number)
}

/// The title a record's heading gives after its code: `# ADR-0001: <title>`.
fn record_title<'a>(text: &'a str, code: &str) -> Option<&'a str> {
    let title = text
        .lines()
        .next()?
        .strip_prefix("# ")?
        .strip_prefix(code)?
        .strip_prefix(": ")?
        .trim();
    (!title.is_empty()).then_some(title)
}
