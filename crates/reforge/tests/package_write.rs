//! The writers: a package as a mod project directory and as a `.modpkg`, each written beside
//! its place and renamed into it, with the report. A write that fails leaves nothing.

use std::io::Cursor;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use ltk_modpkg::Modpkg as Packed;
use reforge::diag::{Diagnostics, Render, Stage};
use reforge::model::entity::FileId;
use reforge::model::{ByteView, ChunkPath, Spelled, WadName, path_hash};
use reforge::package::{
    ArchiveRef, Chunk, ChunkKey, EVIDENCE_FORMAT, EvidenceDating, EvidenceDoc, EvidenceInputs,
    Layer, MAX_NAME_BYTES, Made, Modpkg, Package, PackageWriter, Program, ProjectConfig,
    ProjectDir, README_FILE, REPORT_FILE, WriteError, WriterKind, Written,
};
use reforge_testkit::bins::{bin, blob, object, text};
use reforge_testkit::skins::WAD;
use serde_json::{Value, json};

const SKIN_BIN: &str = "data/characters/test/skins/skin0.bin";
const TEXTURE: &str = "assets/mod/a/body.tex";
const BANK: &str = "assets/sounds/mod/a/audio.bnk";
const REPORT: &str = "{ \"schema\": \"reforge.report/1\" }\n";

fn config() -> ProjectConfig {
    ProjectConfig {
        name: "Synthetic-tests".into(),
        display_name: Spelled::new("Synthetic \u{1b}[31m"),
        version: "1.2.3".into(),
        description: Spelled::new("A package built by hand."),
        authors: vec![Spelled::new("reforge tests")],
        champions: vec!["Test".into()],
    }
}

fn key(path: &str) -> ChunkKey {
    let wad = WadName::parse(WAD).expect("a WAD name");
    ChunkKey::new(wad, ChunkPath::parse(path).expect("a path"))
}

fn chunk(bytes: Vec<u8>) -> Chunk {
    Chunk::kept(FileId::new(0), ByteView::from(bytes))
}

fn skin_bytes() -> Vec<u8> {
    bin(vec![object(
        "A",
        "Material",
        vec![("textureName", text(TEXTURE))],
    )])
}

/// A bin, a texture and a bank in one WAD, and a path its hash table names.
fn package() -> Package {
    let mut package = Package::new(config());
    package.insert(Layer::Base, key(SKIN_BIN), chunk(skin_bytes()));
    package.insert(Layer::Base, key(TEXTURE), chunk(blob(TEXTURE)));
    package.insert(Layer::Base, key(BANK), chunk(blob(BANK)));
    package.tables.add_path("assets/mod/a/named.tex");
    package
}

/// The evidence of a run that dated nothing, as a run sets it on the package it builds.
fn evidence() -> EvidenceDoc {
    EvidenceDoc {
        format: EVIDENCE_FORMAT.into(),
        program: Program {
            name: "reforge".into(),
            version: "0.1.0".into(),
        },
        run: Made::Repaired,
        archive: ArchiveRef {
            sha256: "ab".repeat(32),
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
    }
}

/// [`package`] with the evidence of a run.
fn package_of_a_run() -> Package {
    let mut package = package();
    package.evidence = Some(evidence());
    package
}

/// A path with a part longer than a file name may be, as the game names a bin shared
/// between many skins.
fn long_path() -> String {
    let skins: String = (0..60).map(|n| format!("_skins_skin{n}")).collect();
    let path = format!("data/characters/test/test{skins}.bin");
    assert!(
        path.rsplit('/')
            .next()
            .is_some_and(|name| name.len() > MAX_NAME_BYTES)
    );
    path
}

/// A package with a path that is a file and a directory at once: no tree on disk holds it.
fn unwritable() -> Package {
    let mut package = package();
    package.insert(Layer::Base, key("assets/mod/a"), chunk(blob("a file")));
    package
}

/// A new directory for a test, as a UTF-8 path.
fn scratch() -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("a UTF-8 path");
    (dir, path)
}

/// The names in a directory, sorted.
fn names(dir: &Utf8Path) -> Vec<String> {
    let entries = fs::read_dir(dir.as_std_path()).expect("the directory reads");
    let mut names: Vec<String> = entries
        .map(|entry| entry.expect("an entry reads").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Every file under a directory, by its path from it with `/` between the parts, sorted.
fn tree(dir: &Utf8Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut open = vec![dir.to_path_buf()];
    while let Some(next) = open.pop() {
        for name in names(&next) {
            let path = next.join(&name);
            if path.is_dir() {
                open.push(path);
            } else {
                let relative = path.strip_prefix(dir).expect("under the directory");
                found.push(relative.as_str().replace('\\', "/"));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn a_project_directory_holds_the_chunks_the_tables_the_config_and_the_report() {
    let (_guard, out) = scratch();
    let package = package();
    let writer = ProjectDir::new(&out);
    let written = writer.write(&package, REPORT).expect("the project writes");
    assert_eq!(
        written,
        Written {
            kind: WriterKind::Project,
            name: "Synthetic-tests".into(),
            chunks: 3,
            hash_named: 0,
            bytes: (skin_bytes().len() + blob(TEXTURE).len() + blob(BANK).len()) as u64,
        }
    );
    // The project is the one thing in the output directory: nothing is left of the write.
    assert_eq!(names(&out), ["Synthetic-tests"]);
    let project = writer.path_of(&package);
    assert_eq!(project, out.join("Synthetic-tests"));
    assert_eq!(
        tree(&project),
        [
            format!("content/base/{WAD}/{TEXTURE}"),
            format!("content/base/{WAD}/{BANK}"),
            format!("content/base/{WAD}/{SKIN_BIN}"),
            "hashes/binentries.hashes.txt".to_string(),
            "hashes/game.hashes.txt".to_string(),
            "mod.config.json".to_string(),
            REPORT_FILE.to_string(),
        ]
    );
    let read = |path: &str| fs::read(project.join(path)).expect("the file reads");
    assert_eq!(
        read(&format!("content/base/{WAD}/{SKIN_BIN}")),
        skin_bytes()
    );
    assert_eq!(read(REPORT_FILE), REPORT.as_bytes());
    assert_eq!(read("hashes/game.hashes.txt"), b"assets/mod/a/named.tex\n");
    assert_eq!(read("hashes/binentries.hashes.txt"), b"");
    let config: Value = serde_json::from_slice(&read("mod.config.json")).expect("JSON");
    assert_eq!(config["name"], "Synthetic-tests");
    // The mod's own text is in the file as written, in a JSON string.
    assert_eq!(config["display_name"], "Synthetic \u{1b}[31m");
    assert_eq!(config["version"], "1.2.3");
    assert_eq!(config["authors"], json!(["reforge tests"]));
    assert_eq!(config["champions"], json!(["Test"]));
    assert_eq!(config["layers"][0]["name"], "base");
    assert_eq!(config["hashtables"][0]["path"], "hashes/game.hashes.txt");
}

#[test]
fn writing_raises_what_was_written() {
    let (_guard, out) = scratch();
    let mut log = Diagnostics::new();
    let writer = ProjectDir::new(&out);
    let written = package().write(&writer, REPORT, &mut log.at(Stage::Write));
    written.expect("the project writes");
    let diagnostic = log.iter().next().expect("the write is raised");
    assert_eq!(diagnostic.code.name, "write.written");
    assert_eq!(diagnostic.stage, Stage::Write);
    // The finding holds the project's name and no path of the machine.
    assert_eq!(
        diagnostic.message(&Render::plain()),
        "wrote the project Synthetic-tests (3 chunks)"
    );
    assert_eq!(
        diagnostic.data(),
        json!({
            "kind": "project", "name": "Synthetic-tests", "chunks": 3, "hash_named": 0,
            "bytes": log.of::<Written>().next().expect("the finding").bytes,
        })
    );
}

#[test]
fn a_project_written_again_takes_the_place_of_the_one_there() {
    let (_guard, out) = scratch();
    let writer = ProjectDir::new(&out);
    writer.write(&package(), REPORT).expect("the first write");
    let mut smaller = Package::new(config());
    smaller.insert(Layer::Base, key(SKIN_BIN), chunk(skin_bytes()));
    let written = writer.write(&smaller, REPORT).expect("the second write");
    assert_eq!(written.chunks, 1);
    assert_eq!(names(&out), ["Synthetic-tests"]);
    let chunks: Vec<String> = tree(&out.join("Synthetic-tests").join("content"));
    assert_eq!(chunks, [format!("base/{WAD}/{SKIN_BIN}")]);
}

#[test]
fn a_directory_that_is_no_project_is_not_replaced() {
    let (_guard, out) = scratch();
    let theirs = out.join("Synthetic-tests");
    fs::create_dir_all(&theirs).expect("a directory");
    fs::write(theirs.join("notes.txt"), "someone's own").expect("a file");
    let failed = ProjectDir::new(&out).write(&package(), REPORT);
    let Err(WriteError::Occupied(path)) = failed else {
        panic!("the write is refused: {failed:?}");
    };
    assert_eq!(path, theirs);
    assert_eq!(names(&out), ["Synthetic-tests"]);
    assert_eq!(tree(&theirs), ["notes.txt"]);
}

#[test]
fn a_project_that_fails_to_write_leaves_nothing() {
    let (_guard, out) = scratch();
    let failed = ProjectDir::new(&out).write(&unwritable(), REPORT);
    assert!(matches!(failed, Err(WriteError::Io(_))), "{failed:?}");
    assert_eq!(names(&out), Vec::<String>::new());

    // A project that was there stays as it was.
    ProjectDir::new(&out)
        .write(&package(), REPORT)
        .expect("a project writes");
    let before = tree(&out);
    let failed = ProjectDir::new(&out).write(&unwritable(), REPORT);
    assert!(failed.is_err());
    assert_eq!(tree(&out), before);
}

#[test]
fn a_chunk_with_a_part_longer_than_a_file_name_is_named_by_its_hash() {
    let (_guard, out) = scratch();
    let long = long_path();
    let mut package = package();
    package.insert(Layer::Base, key(&long), chunk(skin_bytes()));
    let written = ProjectDir::new(&out)
        .write(&package, REPORT)
        .expect("the project writes");
    assert_eq!((written.chunks, written.hash_named), (4, 1));
    let project = out.join("Synthetic-tests");
    let hashed = format!("content/base/{WAD}/{:016x}.bin", path_hash(&long).0);
    assert!(tree(&project).contains(&hashed), "{:#?}", tree(&project));
    assert_eq!(
        fs::read(project.join(&hashed)).expect("the chunk reads"),
        skin_bytes()
    );
    // The name the chunk lost on disk is in the table of paths.
    let table = fs::read_to_string(project.join("hashes/game.hashes.txt")).expect("the table");
    assert_eq!(table, format!("assets/mod/a/named.tex\n{long}\n"));
}

/// Mounts a `.modpkg` and reads every chunk of it, decompressed, by its path hash.
fn read_back(file: &Utf8Path) -> Vec<(u64, Vec<u8>)> {
    let bytes = fs::read(file).expect("the package reads");
    let mut packed = Packed::mount_from_reader(Cursor::new(bytes)).expect("the package mounts");
    let keys: Vec<_> = packed.chunks().keys().copied().collect();
    let mut chunks: Vec<(u64, Vec<u8>)> = keys
        .into_iter()
        .map(|key| {
            let bytes = packed.load_chunk_decompressed(key);
            let bytes = bytes.unwrap_or_else(|error| panic!("a chunk does not read: {error}"));
            (key.path.value(), bytes.into_vec())
        })
        .collect();
    chunks.sort();
    chunks
}

/// #MOD-ARTIFACT-0007: a chunk named by its hash in the project is at that hash in the
/// package.
#[test]
fn a_modpkg_holds_every_chunk_and_reads_back() {
    let (_guard, out) = scratch();
    let long = long_path();
    let mut package = package();
    package.insert(Layer::Base, key(&long), chunk(skin_bytes()));
    let writer = Modpkg::new(&out);
    let written = writer.write(&package, REPORT).expect("the package writes");
    assert_eq!(written.kind, WriterKind::Modpkg);
    assert_eq!(written.name, "Synthetic-tests_1.2.3.modpkg");
    assert_eq!((written.chunks, written.hash_named), (4, 1));
    // The package and the report beside it, and nothing of the project it was packed from.
    assert_eq!(
        names(&out),
        [
            "Synthetic-tests_1.2.3.modpkg",
            "Synthetic-tests_1.2.3.reforge-report.json"
        ]
    );
    assert_eq!(
        fs::read(writer.report_path_of(&package)).expect("the report reads"),
        REPORT.as_bytes()
    );

    let chunks = read_back(&writer.path_of(&package));
    let held = |path: &str| {
        let hash = path_hash(path).0;
        let mut found = chunks.iter().filter(|(key, _)| *key == hash);
        let chunk = found.next().unwrap_or_else(|| panic!("no chunk at {path}"));
        assert!(found.next().is_none(), "one chunk at {path}");
        chunk.1.clone()
    };
    assert_eq!(held(SKIN_BIN), skin_bytes());
    assert_eq!(held(TEXTURE), blob(TEXTURE));
    assert_eq!(held(BANK), blob(BANK));
    // The chunk named by its hash on disk is at the hash of its path.
    assert_eq!(held(&long), skin_bytes());
}

/// #ADR-0041: the evidence is the project's `README.md`, at its root.
#[test]
fn a_project_of_a_package_with_evidence_holds_it_as_its_readme() {
    let (_guard, out) = scratch();
    let package = package_of_a_run();
    let writer = ProjectDir::new(&out);
    let written = writer.write(&package, REPORT).expect("the project writes");
    // The README is no chunk of the package.
    assert_eq!(written.chunks, 3);
    let project = writer.path_of(&package);
    assert!(
        tree(&project).contains(&README_FILE.to_string()),
        "{:#?}",
        tree(&project)
    );
    let readme = fs::read_to_string(project.join(README_FILE)).expect("the README reads");
    assert_eq!(readme, evidence().readme());
    assert!(
        !readme.contains('\r'),
        "the README's lines end in a line feed"
    );
    assert_eq!(
        EvidenceDoc::parse(&readme).expect("the evidence reads"),
        Some(evidence())
    );
}

/// #ADR-0041: the packer packs the project's `README.md` as the readme of the `.modpkg`,
/// byte for byte.
#[test]
fn a_modpkg_of_a_package_with_evidence_holds_it_as_its_readme() {
    let (_guard, out) = scratch();
    let package = package_of_a_run();
    let writer = Modpkg::new(&out);
    writer.write(&package, REPORT).expect("the package writes");
    let bytes = fs::read(writer.path_of(&package)).expect("the package reads");
    let mut packed = Packed::mount_from_reader(Cursor::new(bytes)).expect("the package mounts");
    let readme = packed.load_readme().expect("the package has a readme");
    assert_eq!(readme, evidence().readme().into_bytes());
    let readme = String::from_utf8(readme).expect("the readme is text");
    assert_eq!(
        EvidenceDoc::parse(&readme).expect("the evidence reads"),
        Some(evidence())
    );
    // The chunks the client loads are the package's own, with none added for the readme.
    let chunks = read_back(&writer.path_of(&package));
    let held = |path: &str| chunks.iter().any(|(key, _)| *key == path_hash(path).0);
    assert!(held(SKIN_BIN) && held(TEXTURE) && held(BANK));

    // A package no run built has no readme.
    let (_guard, bare) = scratch();
    let writer = Modpkg::new(&bare);
    writer.write(&self::package(), REPORT).expect("it writes");
    let bytes = fs::read(writer.path_of(&package)).expect("the package reads");
    let mut packed = Packed::mount_from_reader(Cursor::new(bytes)).expect("the package mounts");
    assert!(packed.load_readme().is_err());
}

#[test]
fn a_modpkg_that_fails_to_write_leaves_nothing() {
    let (_guard, out) = scratch();
    let failed = Modpkg::new(&out).write(&unwritable(), REPORT);
    assert!(failed.is_err(), "{failed:?}");
    assert_eq!(names(&out), Vec::<String>::new());
}

#[test]
fn a_project_the_packer_refuses_does_not_pack_and_leaves_nothing() {
    let (_guard, out) = scratch();
    // A `.modpkg` holds a version of three numbers, and a project's config any text.
    let mut package = package();
    package.config.version = "1.2".into();
    let failed = Modpkg::new(&out).write(&package, REPORT);
    assert!(matches!(failed, Err(WriteError::Pack(_))), "{failed:?}");
    assert_eq!(names(&out), Vec::<String>::new());

    let written = ProjectDir::new(&out).write(&package, REPORT);
    assert_eq!(written.expect("the project writes").chunks, 3);
}

#[test]
fn two_writes_of_one_package_give_the_same_files() {
    let (_guard, first) = scratch();
    let (_guard, second) = scratch();
    for out in [&first, &second] {
        ProjectDir::new(out)
            .write(&package(), REPORT)
            .expect("the project writes");
        Modpkg::new(out)
            .write(&package(), REPORT)
            .expect("the package writes");
    }
    assert_eq!(tree(&first), tree(&second));
    let packed = "Synthetic-tests_1.2.3.modpkg";
    for file in tree(&first).iter().filter(|file| *file != packed) {
        let read = |out: &Utf8Path| fs::read(out.join(file)).expect("the file reads");
        assert_eq!(read(&first), read(&second), "{file}");
    }
    // `ltk_modpkg` 0.12.5 writes its chunks in the order of a hashed map, so two packages of
    // one project differ in the order of their chunks and in nothing a reader gets from them.
    assert_eq!(
        read_back(&first.join(packed)),
        read_back(&second.join(packed))
    );
}

/// Reads back the `.modpkg` the environment names: `REFORGE_MODPKG=<file> cargo test -p
/// reforge --test package_write -- --ignored --nocapture`. It prints how many chunks the
/// package holds and how many bytes they inflate to, and fails on a chunk that does not read.
#[test]
#[ignore = "reads the package REFORGE_MODPKG names"]
fn the_modpkg_the_environment_names_reads_back() {
    let file = std::env::var("REFORGE_MODPKG").expect("REFORGE_MODPKG names a .modpkg");
    let chunks = read_back(Utf8Path::new(&file));
    let bytes: usize = chunks.iter().map(|(_, bytes)| bytes.len()).sum();
    println!("{} chunks read, {bytes} bytes inflated", chunks.len());
    assert!(!chunks.is_empty());
}
