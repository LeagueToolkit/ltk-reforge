//! The object bytes: each entry text's object as `ltk_meta` writes it, an entry's versions in
//! blocks behind a keyframe, read back at every build, appended as a whole build writes them,
//! and checked against the objects the bins hold.

#[expect(
    dead_code,
    reason = "the shared fixture, of which these tests use the history in memory alone"
)]
mod common;
mod dump;

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use reforge::history::{Families, History, Position, SqliteHistory};
use reforge::model::{BinHash, WadHash};
use reforge_census::{Indexer, check_objects, format};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use self::common::{Memory, build, entry};
use self::dump::dump;

const WAD: &str = "files/data/x.wad.client";
const WAD_FILE: &str = "version: \"3.4\"\nfileId: \"0000000000000001\"\ntags: []\n";

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap().to_owned();
    (dir, root)
}

/// The path of the text of entry `entry` of the bin at `bin`.
fn text_path(bin: u64, entry: u32) -> String {
    let path = format::object_path(WadHash(bin), BinHash(entry));
    format!("{WAD}/{path}")
}

/// The `.rito` text of object `entry` of class `class`: a string of `words`, a u32 that is
/// `version`, and a list when `list`.
fn rito(entry: u32, class: u32, version: u32, words: &str, list: bool) -> String {
    let list = if list {
        "        0x00000ccc: list[u32] = {\n            1\n        }\n"
    } else {
        ""
    };
    format!(
        "#PROP_text\ntype: string = \"PROP\"\nversion: u32 = 3\nlinked: list[string] = {{}}\n\
         entries: map[hash,embed] = {{\n    {entry:#010x} = {class:#010x} {{\n        \
         0x00000aaa: string = \"{words}\"\n        0x00000bbb: u32 = {version}\n{list}    }}\n}}\n"
    )
}

/// A build's files: these texts, by path, and the WAD's file.
fn tree(texts: impl IntoIterator<Item = (String, String)>) -> BTreeMap<String, String> {
    let mut files: BTreeMap<String, String> = texts.into_iter().collect();
    files.insert(format!("{WAD}/_wad.yaml"), WAD_FILE.to_string());
    files
}

/// A history of these builds, `1.<n>.1` from 1.
fn history(trees: Vec<BTreeMap<String, String>>) -> Memory {
    let commits = trees.into_iter().enumerate();
    let commits = commits.map(|(i, files)| (build(&format!("1.{}.1", i + 1)), files));
    Memory {
        commits: commits.collect(),
    }
}

/// A text's one object as `ltk_meta` writes it.
fn bytes_of(text: &str) -> Vec<u8> {
    let (_, objects) = format::read_object_text(text).unwrap();
    assert_eq!(objects.len(), 1);
    objects[0].bytes.clone()
}

fn whole(history: &Memory, out: &Utf8Path) {
    Indexer::new(history, "memory")
        .build(out, Families::ALL)
        .unwrap();
}

fn ids(out: &Utf8Path, sql: &str) -> Vec<i64> {
    let db = Connection::open(out.join("history.sqlite")).unwrap();
    let mut statement = db.prepare(sql).unwrap();
    let rows = statement.query_map([], |r| r.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

/// Twenty builds of entry 1 of bin 2, each another version; entry 3 of bin 2, the same through
/// every build; and entry 4 of bin 2, whose first version comes back after the second.
fn twenty_versions() -> (Memory, Vec<String>) {
    let versions: Vec<String> = (0..20).map(|v| rito(1, 2, v, "Ahri", false)).collect();
    let trees = versions.iter().enumerate().map(|(at, text)| {
        let back = if (5..10).contains(&at) { 1 } else { 0 };
        tree([
            (text_path(2, 1), text.clone()),
            (text_path(2, 3), rito(3, 2, 0, "Lux", false)),
            (text_path(2, 4), rito(4, 2, back, "Zed", false)),
        ])
    });
    (history(trees.collect()), versions)
}

#[test]
fn an_entrys_versions_take_a_keyframe_every_eighth_and_read_back_at_every_build() {
    let (history, versions) = twenty_versions();
    let (_dir, root) = scratch();
    let out = root.join("index");
    whole(&history, &out);
    let index = SqliteHistory::open(&out).unwrap();
    for (at, text) in (0u16..).zip(&versions) {
        let found = index.object_bytes(WadHash(2), BinHash(1), Position(at));
        let found = found.unwrap().unwrap();
        assert_eq!(found.bytes, bytes_of(text), "build {at}");
        assert_eq!(found.class, BinHash(2));
    }
    // Entry 1's twenty versions are rows 1 to 20, then entry 3's one and entry 4's two.
    let keyframes = ids(
        &out,
        "SELECT id FROM object_bytes WHERE keyframe = id ORDER BY id",
    );
    assert_eq!(keyframes, [1, 9, 17, 21, 22]);
    let behind = ids(
        &out,
        "SELECT keyframe FROM object_bytes WHERE keyframe <> id ORDER BY id",
    );
    let mut expected = [1; 7].to_vec();
    expected.extend([9; 7]);
    expected.extend([17; 3]);
    expected.push(22);
    assert_eq!(behind, expected);
    // Entry 4's first version, back at build 10, is the row it was at build 0.
    let back = index.stored_object(&common::blob_id(&rito(4, 2, 0, "Zed", false)));
    assert_eq!(back.unwrap().unwrap().row, 22);
    assert_eq!(index.stamp().unwrap().objectless_texts, 0);
}

#[test]
fn texts_whose_objects_write_the_same_bytes_under_two_classes_share_a_row() {
    let history = history(vec![tree([
        (text_path(2, 1), rito(1, 2, 0, "Ahri", false)),
        (text_path(4, 1), rito(1, 5, 0, "Ahri", false)),
    ])]);
    let (_dir, root) = scratch();
    let out = root.join("index");
    whole(&history, &out);
    assert_eq!(ids(&out, "SELECT id FROM object_bytes"), [1]);
    let index = SqliteHistory::open(&out).unwrap();
    let read = |bin: u64| {
        let found = index.object_bytes(WadHash(bin), BinHash(1), Position(0));
        found.unwrap().unwrap()
    };
    let (two, five) = (read(2), read(4));
    assert_eq!((two.class, five.class), (BinHash(2), BinHash(5)));
    assert_eq!(two.bytes, five.bytes);
}

#[test]
fn a_text_of_two_objects_or_none_that_parses_has_no_bytes_and_is_counted() {
    let two = rito(1, 2, 0, "Ahri", false).replace(
        "entries: map[hash,embed] = {\n",
        "entries: map[hash,embed] = {\n    0x00000009 = 0x00000002 {\n    }\n",
    );
    let history = history(vec![tree([
        (text_path(2, 1), two),
        (text_path(2, 3), "not a text".to_string()),
        (text_path(2, 4), rito(4, 2, 0, "Lux", false)),
    ])]);
    let (_dir, root) = scratch();
    let out = root.join("index");
    let stamp = Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    assert_eq!((stamp.objectless_texts, stamp.unparsed_texts), (1, 1));
    let index = SqliteHistory::open(&out).unwrap();
    for entry in [1, 3] {
        let found = index.object_bytes(WadHash(2), BinHash(entry), Position(0));
        assert_eq!(found.unwrap(), None, "entry {entry}");
    }
    let check = check_objects(&out).unwrap();
    assert_eq!((check.texts, check.stored, check.objects), (3, 1, 1));
}

/// The index of the first `builds` builds of a history.
fn index_of(history: &Memory, builds: usize, out: &Utf8Path) {
    let start = Memory {
        commits: history.commits[..builds].to_vec(),
    };
    whole(&start, out);
}

#[test]
fn an_append_across_keyframes_holds_the_bytes_and_blocks_of_the_whole_build() {
    let (history, _) = twenty_versions();
    let (_dir, root) = scratch();
    let out = root.join("whole");
    whole(&history, &out);
    let expected = dump(&out);
    for first in [1, 7, 8, 9, 16, 19] {
        let appended = root.join(format!("from-{first}"));
        index_of(&history, first, &appended);
        Indexer::new(&history, "memory").append(&appended).unwrap();
        assert_eq!(
            dump(&appended),
            expected,
            "{first} builds and the rest appended"
        );
    }
    let stepped = root.join("stepped");
    index_of(&history, 1, &stepped);
    for upto in 2..=history.commits.len() {
        let so_far = Memory {
            commits: history.commits[..upto].to_vec(),
        };
        Indexer::new(&so_far, "memory").append(&stepped).unwrap();
    }
    assert_eq!(dump(&stepped), expected);
}

/// About four kilobytes of words, a few hundred of them, chosen by `n`.
fn words(n: u32) -> String {
    let names = ["Ahri", "Lux", "Zed", "Jinx", "Vi", "Ekko", "Sona"];
    let parts = ["Particles", "Skins", "Base", "Glow", "Trail", "Mesh", "Tex"];
    let mut out = String::new();
    let mut i = n;
    while out.len() < 4000 {
        let name = names[(i % 7) as usize];
        let part = parts[(i / 7 % 7) as usize];
        out.push_str(&format!("{name}/{part}_{}/", i % 31));
        i = i.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 65_536;
    }
    out
}

/// Ten builds of 420 entries of class 2 with four kilobytes of words each, enough keyframe
/// bytes for a dictionary of the class's own, and entry 1000, which changes every build.
fn many_entries() -> Memory {
    let trees = (0..10).map(|at| {
        let mut texts: Vec<(String, String)> = (1..=420)
            .map(|e| (text_path(2, e), rito(e, 2, 0, &words(e), true)))
            .collect();
        texts.push((text_path(2, 1000), rito(1000, 2, at, &words(1000), true)));
        tree(texts)
    });
    history(trees.collect())
}

#[test]
fn a_class_trains_a_dictionary_once_and_an_append_compresses_its_keyframes_with_it() {
    let history = many_entries();
    let (_dir, root) = scratch();
    let out = root.join("whole");
    whole(&history, &out);
    let classes = ids(
        &out,
        "SELECT class FROM object_dicts WHERE class IS NOT NULL",
    );
    assert_eq!(classes, [2]);
    let dict = ids(&out, "SELECT id FROM object_dicts WHERE class = 2");
    let keyframe_dicts = ids(
        &out,
        "SELECT DISTINCT dict FROM object_bytes WHERE keyframe = id",
    );
    assert_eq!(keyframe_dicts, dict);

    // Entry 1000's keyframe at build 8 is appended, with the dictionary of five builds.
    let appended = root.join("appended");
    index_of(&history, 5, &appended);
    Indexer::new(&history, "memory").append(&appended).unwrap();
    assert_eq!(dump(&appended), dump(&out));
    let keyframes = ids(
        &appended,
        "SELECT dict FROM object_bytes WHERE keyframe = id ORDER BY id DESC LIMIT 1",
    );
    assert_eq!(
        keyframes,
        ids(&appended, "SELECT id FROM object_dicts WHERE class = 2")
    );
    let index = SqliteHistory::open(&appended).unwrap();
    let at_nine = index.object_bytes(WadHash(2), BinHash(1000), Position(9));
    let expected = rito(1000, 2, 9, &words(1000), true);
    assert_eq!(at_nine.unwrap().unwrap().bytes, bytes_of(&expected));

    // A second whole build writes the same file, its dictionaries and frames among it.
    let again = root.join("again");
    whole(&history, &again);
    let read = |dir: &Utf8Path| fs_err::read(dir.join("history.sqlite")).unwrap();
    assert!(read(&out) == read(&again), "two whole builds differ");
}

/// The file of the bin at `path` holding one object, entry 1 of class 2, of this key.
fn bin_file(path: u64, key: &str) -> (String, String) {
    entry(
        path,
        format!(
            "sha256: \"{}\"\nkind: \"bin\"\nlinks: []\nobjects:\n \"00000001\":\n  class: \
             \"00000002\"\n  content: \"{key}\"\n  norm: \"0000000000000007\"\n",
            "9".repeat(64)
        ),
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn an_object_whose_digest_is_no_object_key_is_named_with_where_it_first_appeared() {
    let (keyed, unkeyed) = (rito(1, 2, 0, "Ahri", true), rito(1, 2, 1, "Ahri", true));
    let key = hex(&Sha256::digest(bytes_of(&keyed)));
    let mut history = history(vec![
        tree([(text_path(2, 1), keyed), bin_file(2, &key)]),
        tree([
            (text_path(5, 1), unkeyed.clone()),
            bin_file(5, &"a".repeat(64)),
        ]),
    ]);
    history.commits[1].0.details.legacy_bins = true;
    let (_dir, root) = scratch();
    let out = root.join("index");
    whole(&history, &out);
    let check = check_objects(&out).unwrap();
    assert_eq!((check.stored, check.keyed, check.orphans), (2, 1, 0));
    let [named] = check.unkeyed.as_slice() else {
        panic!("{:?}", check.unkeyed)
    };
    assert_eq!(named.blob, common::blob_id(&unkeyed));
    assert_eq!(
        (named.bin, named.entry, named.first),
        (WadHash(5), BinHash(1), Position(1))
    );
    assert_eq!(named.class, BinHash(2));
    assert!(named.legacy && named.renumbered, "{named:?}");
}
