//! A bin entry's text read back by the blob the index names: the text the build's tree holds, from
//! the first WAD in path order when two WADs hold the bin.

mod common;
mod repo;

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use reforge::history::{Families, History, Life, Position, SqliteHistory};
use reforge::model::{BinHash, WadHash};
use reforge_census::{ChangeSource, GitRepo, Indexer};
use tempfile::TempDir;

use self::common::{Memory, blob_id, memory};

/// The object's text in the fixture's WAD.
const TEXT: &str = "files/data/x.wad.client/00/0000000000000002.bin/00/00000001.rito";

/// A WAD before the fixture's in path order.
const FIRST_WAD: &str = "files/data/a.wad.client";

const BIN: WadHash = WadHash(2);
const ENTRY: BinHash = BinHash(1);

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap().to_owned();
    (dir, root)
}

/// The fixture with the object's text changed in the second build and back in the third, where a
/// WAD before the fixture's holds the same bin with the second build's text.
fn history() -> Memory {
    let mut history = memory();
    let other = history.commits[1].1[TEXT].replace("\"Test\"", "\"Other\"");
    history.commits[1].1.insert(TEXT.into(), other.clone());
    let wad = "version: \"3.4\"\nfileId: \"0000000000000009\"\ntags: []\n";
    let third: &mut BTreeMap<String, String> = &mut history.commits[2].1;
    third.insert(format!("{FIRST_WAD}/_wad.yaml"), wad.into());
    let text = format!("{FIRST_WAD}/00/0000000000000002.bin/00/00000001.rito");
    third.insert(text, other);
    history
}

/// A bare repository at `dir` holding every build of a history.
fn repository(history: &Memory, dir: &Utf8Path) -> GitRepo {
    let repo = gix::init_bare(dir.as_std_path()).unwrap();
    let mut parent: Option<String> = None;
    for (build, files) in &history.commits {
        parent = Some(repo::commit(&repo, parent.as_deref(), build, files));
    }
    GitRepo::open(dir).unwrap()
}

#[test]
fn an_entrys_text_at_a_build_is_the_blob_of_its_first_wad() {
    let (_dir, root) = scratch();
    let history = history();
    let out = root.join("index");
    Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    let (test, other) = (&history.commits[0].1[TEXT], &history.commits[1].1[TEXT]);
    let at = |build: u16| {
        let held = index
            .text_blob(BIN, ENTRY, Position(build))
            .unwrap()
            .unwrap();
        (held.wad, held.blob, held.life)
    };
    let x = "data/x.wad.client".to_string();
    assert_eq!(at(0), (x.clone(), blob_id(test), Life::new(0, 0)));
    assert_eq!(at(1), (x, blob_id(other), Life::new(1, 1)));
    // Both WADs hold the bin at the third build, and the first in path order answers.
    assert_eq!(
        at(2),
        ("data/a.wad.client".into(), blob_id(other), Life::new(2, 2))
    );
    let read = history.read(&at(2).1).unwrap();
    assert_eq!(String::from_utf8(read).unwrap(), *other);
    assert_eq!(index.text_blob(BIN, BinHash(9), Position(0)).unwrap(), None);
    assert_eq!(
        index.text_blob(WadHash(9), ENTRY, Position(0)).unwrap(),
        None
    );
}

#[test]
fn a_text_read_by_its_blob_is_the_one_the_builds_tree_holds() {
    let (_dir, root) = scratch();
    let repo = repository(&history(), &root.join("history"));
    let commits = repo.commits().unwrap();
    for families in [Families::ALL, Families::parse("files").unwrap()] {
        let out = root.join(format!("index-{}", families.iter().count()));
        Indexer::new(&repo, "history")
            .build(&out, families)
            .unwrap();
        let index = SqliteHistory::open(&out).unwrap();
        assert_eq!(index.builds()[2].details, history().commits[2].0.details);
        for (at, commit) in (0u16..).zip(&commits) {
            let by_blob = repo
                .entry_text_at(&index, BIN, ENTRY, Position(at))
                .unwrap();
            let in_tree = repo.entry_text(BIN, ENTRY, commit).unwrap();
            assert!(in_tree.is_some(), "the build at {at} holds the text");
            assert_eq!(
                by_blob, in_tree,
                "the text at {at}, indexed with {families:?}"
            );
        }
        let missing = repo.entry_text_at(&index, BIN, BinHash(9), Position(0));
        assert_eq!(missing.unwrap(), None);
    }
}
