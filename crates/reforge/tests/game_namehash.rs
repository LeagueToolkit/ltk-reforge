//! The name-hash index: built from a path table of the hash cache, read back by name hash, and
//! built again when the table's stamp changes or the file is no index.

use std::io::Cursor;

use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_hashdb::{Compression, HashDb, HashDbWriter, KeyWidth};
use reforge::game::{NameHashIndex, NameHashSlot};
use reforge::model::{WadHash, name_hash, path_hash};
use tempfile::TempDir;

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path =
        Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("the directory is UTF-8");
    (dir, path)
}

/// A path table of these paths, each under its path hash. `under` adds paths under the keys
/// given.
fn table(paths: &[&str], under: &[(u64, &str)]) -> HashDb {
    let mut writer = HashDbWriter::new(KeyWidth::U64, Compression::None);
    for path in paths {
        writer.insert(path_hash(path).0, path);
    }
    for (key, path) in under {
        writer.insert(*key, path);
    }
    let mut out = Cursor::new(Vec::new());
    writer.build(&mut out).unwrap();
    HashDb::open_bytes(out.into_inner()).unwrap()
}

#[test]
fn a_path_is_found_by_its_name_hash() {
    let (_keep, dir) = scratch();
    let a = "assets/characters/test/particles/a.tex";
    let index =
        NameHashIndex::open_or_build(&dir, "game-1.lhdb", &table(&[a, "assets/b.tex"], &[]))
            .unwrap();
    assert_eq!(
        index.get(name_hash(a)).unwrap(),
        Some(NameHashSlot::One(path_hash(a)))
    );
    assert_eq!(index.get(name_hash("assets/none.tex")).unwrap(), None);
}

#[test]
fn every_path_of_a_table_is_found() {
    let (_keep, dir) = scratch();
    let paths: Vec<String> = (0..200)
        .map(|i| format!("assets/characters/test/skins/skin{i}/particles/glow.tex"))
        .collect();
    let borrowed: Vec<&str> = paths.iter().map(String::as_str).collect();
    let index = NameHashIndex::open_or_build(&dir, "game-1.lhdb", &table(&borrowed, &[])).unwrap();
    for path in &paths {
        assert_eq!(
            index.get(name_hash(path)).unwrap(),
            Some(NameHashSlot::One(path_hash(path))),
            "{path}"
        );
    }
}

#[test]
fn a_name_hash_several_paths_share_names_none() {
    // Two keys for one spelling: the name hash cannot tell them apart.
    let (_keep, dir) = scratch();
    let twice = table(&[], &[(1, "assets/same.tex"), (2, "assets/same.tex")]);
    let index = NameHashIndex::open_or_build(&dir, "game-1.lhdb", &twice).unwrap();
    assert_eq!(
        index.get(name_hash("assets/same.tex")).unwrap(),
        Some(NameHashSlot::Several)
    );
}

#[test]
fn a_new_table_rebuilds_the_index() {
    let (_keep, dir) = scratch();
    let (old, new) = ("assets/old.tex", "assets/new.tex");
    NameHashIndex::open_or_build(&dir, "game-1.lhdb", &table(&[old], &[])).unwrap();
    let index = NameHashIndex::open_or_build(&dir, "game-2.lhdb", &table(&[new], &[])).unwrap();
    assert_eq!(
        index.get(name_hash(new)).unwrap(),
        Some(NameHashSlot::One(path_hash(new)))
    );
    assert_eq!(index.get(name_hash(old)).unwrap(), None);
}

#[test]
fn an_index_of_the_same_stamp_is_read_and_not_built_again() {
    let (_keep, dir) = scratch();
    let (first, second) = ("assets/first.tex", "assets/second.tex");
    NameHashIndex::open_or_build(&dir, "game-1.lhdb", &table(&[first], &[])).unwrap();
    // The stamp says the table is the same, so the table given is not read.
    let index = NameHashIndex::open_or_build(&dir, "game-1.lhdb", &table(&[second], &[])).unwrap();
    assert_eq!(
        index.get(name_hash(first)).unwrap(),
        Some(NameHashSlot::One(path_hash(first)))
    );
    assert_eq!(index.get(name_hash(second)).unwrap(), None);
}

#[test]
fn a_file_that_is_no_index_is_built_again() {
    let (_keep, dir) = scratch();
    let path = "assets/characters/test/particles/a.tex";
    let paths = table(&[path, "assets/b.tex"], &[]);
    let file = dir.join("name-hash.idx");
    let found = |index: NameHashIndex| index.get(name_hash(path)).unwrap();
    let one = Some(NameHashSlot::One(path_hash(path)));

    drop(NameHashIndex::open_or_build(&dir, "game-1.lhdb", &paths).unwrap());
    let whole = fs::read(&file).unwrap();

    // Cut short in its records, in its head, and to nothing.
    for length in [whole.len() - 5, 20, 3, 0] {
        fs::write(&file, &whole[..length]).unwrap();
        let index = NameHashIndex::open_or_build(&dir, "game-1.lhdb", &paths).unwrap();
        assert_eq!(found(index), one, "cut to {length} bytes");
        assert_eq!(fs::read(&file).unwrap(), whole, "cut to {length} bytes");
    }

    // Another file under the index's name.
    fs::write(&file, vec![0x41; whole.len()]).unwrap();
    let index = NameHashIndex::open_or_build(&dir, "game-1.lhdb", &paths).unwrap();
    assert_eq!(found(index), one);
    assert_eq!(fs::read(&file).unwrap(), whole);
}

#[test]
fn the_index_is_the_same_bytes_for_the_same_table() {
    let (_keep_a, a) = scratch();
    let (_keep_b, b) = scratch();
    let paths = ["assets/b.tex", "assets/a.tex", "assets/c.tex"];
    let reversed = [paths[2], paths[1], paths[0]];
    drop(NameHashIndex::open_or_build(&a, "game-1.lhdb", &table(&paths, &[])).unwrap());
    drop(NameHashIndex::open_or_build(&b, "game-1.lhdb", &table(&reversed, &[])).unwrap());
    let bytes = fs::read(a.join("name-hash.idx")).unwrap();
    assert_eq!(bytes, fs::read(b.join("name-hash.idx")).unwrap());
    // Head, stamp, count, then three records of twelve bytes.
    assert_eq!(bytes.len(), 16 + "game-1.lhdb".len() + 8 + 3 * 12);
    assert_eq!(&bytes[..8], b"FORGENHX");
    // No temporary file is left beside it.
    let left: Vec<_> = fs::read_dir(&a)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(left, ["name-hash.idx"]);
}

#[test]
fn the_index_is_made_in_a_directory_that_does_not_exist_yet() {
    let (_keep, dir) = scratch();
    let nested = dir.join("names").join("index");
    let path = "assets/a.tex";
    let index = NameHashIndex::open_or_build(&nested, "game-1.lhdb", &table(&[path], &[])).unwrap();
    assert_eq!(
        index.get(name_hash(path)).unwrap(),
        Some(NameHashSlot::One(WadHash(path_hash(path).0)))
    );
}
