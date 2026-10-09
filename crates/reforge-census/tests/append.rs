//! Appending to an index: the index of the first builds with the rest appended is the index of
//! every build, row for row, and its object bytes decoded.

mod common;
mod dump;

use camino::{Utf8Path, Utf8PathBuf};
use reforge::history::{Families, History, Life, SqliteHistory, Stamp};
use reforge::model::{FileKey, WadHash};
use reforge_census::{CensusError, Indexer};
use tempfile::TempDir;

use self::common::{A, Memory, build, memory};
use self::dump::dump;

/// The fixture and two more builds: the fourth drops path 3, one of the two places holding
/// bytes A, a mesh appears at path 5, the events bank and the animation clip are the first
/// build's, the WAD is another version of its file, and the object's text names another tag;
/// the fifth is the third again.
fn five_builds() -> Memory {
    let mut history = memory();
    let third = history.commits[2].1.clone();
    let mut fourth = third.clone();
    fourth.retain(|path, _| !path.ends_with("/0000000000000003.yaml"));
    let wad = "version: \"3.4\"\nfileId: \"0000000000000002\"\ntags: []\n";
    fourth.insert("files/data/x.wad.client/_wad.yaml".into(), wad.into());
    for (path, text) in &mut fourth {
        if path.ends_with(".rito") {
            *text = text.replace("\"Test\"", "\"Other\"");
            // The icon is a file, a field of another kind, and the object links another.
            let string = format!("string = \"{}\"", common::NAMED);
            *text = text.replace(&string, "file = 0x0000000000000005");
            *text = text.replace(
                "    0x00000001 = 0x00000002 {\n",
                "    0x00000001 = 0x00000002 {\n        0x0000abcd: link = 0x00000007\n",
            );
        }
        // The bin links another bin, so it is another content.
        if path.ends_with("/0000000000000002.yaml") {
            *text = text.replace("links: []\n", "links:\n - \"DATA/Other.bin\"\n");
            *text = text.replace(&"9".repeat(64), &"8".repeat(64));
        }
    }
    let mesh = "sha256: \"1111111111111111111111111111111111111111111111111111111111111111\"\n\
                kind: \"skn\"\nmesh:\n vertexType: \"basic\"\n vertices: 30\n indices: 60\n \
                submeshes:\n  - name: \"Body\"\n    vertices: 30\n    indices: 60\n    \
                positions: \"00000000000000b1\"\n    rounded: \"00000000000000b2\"\n";
    fourth.extend([common::entry(5, mesh.to_string())]);
    // The fourth build's bank is the first build's again, and its clip the first's.
    fourth.extend([common::entry(common::bank().0, common::events_bank('e', 1))]);
    fourth.extend([common::entry(7, common::animation('5'))]);
    history.commits.push((build("1.4.1"), fourth));
    history.commits.push((build("1.5.1"), third));
    history
}

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap().to_owned();
    (dir, root)
}

/// The index of the first `builds` builds of a history.
fn index_of(history: &Memory, builds: usize, out: &Utf8Path) {
    let start = Memory {
        commits: history.commits[..builds].to_vec(),
    };
    Indexer::new(&start, "memory")
        .build(out, Families::ALL)
        .unwrap();
}

#[test]
fn an_appended_index_is_the_index_built_whole() {
    let history = five_builds();
    let (_dir, root) = scratch();
    let whole = root.join("whole");
    let stamp = Indexer::new(&history, "memory")
        .build(&whole, Families::ALL)
        .unwrap();
    let expected = dump(&whole);
    let builds = fs_err::read_to_string(whole.join("builds.tsv")).unwrap();
    for first in 1..history.commits.len() {
        let out = root.join(format!("from-{first}"));
        index_of(&history, first, &out);
        let mut heard = 0;
        let appended = Indexer::new(&history, "memory")
            .on_progress(|_| heard += 1)
            .append(&out)
            .unwrap();
        assert_eq!(
            dump(&out),
            expected,
            "the index of {first} builds with the rest appended"
        );
        assert_eq!(
            fs_err::read_to_string(out.join("builds.tsv")).unwrap(),
            builds
        );
        assert_eq!(appended, stamp, "the stamp after {first} builds");
        assert_eq!(heard, history.commits.len() - first);
    }
}

#[test]
fn an_append_one_build_at_a_time_is_the_index_built_whole() {
    let history = five_builds();
    let (_dir, root) = scratch();
    let (whole, out) = (root.join("whole"), root.join("stepped"));
    Indexer::new(&history, "memory")
        .build(&whole, Families::ALL)
        .unwrap();
    index_of(&history, 1, &out);
    for upto in 2..=history.commits.len() {
        let so_far = Memory {
            commits: history.commits[..upto].to_vec(),
        };
        let stamp = Indexer::new(&so_far, "memory").append(&out).unwrap();
        assert_eq!(stamp.builds, upto);
    }
    assert_eq!(dump(&out), dump(&whole));
}

#[test]
fn an_index_that_holds_every_build_is_left_as_it_is() {
    let history = memory();
    let (_dir, root) = scratch();
    let out = root.join("index");
    let built = Indexer::new(&history, "memory")
        .build(&out, Families::ALL)
        .unwrap();
    let appended = Indexer::new(&history, "elsewhere").append(&out).unwrap();
    assert_eq!(appended, built);
}

#[test]
fn a_file_two_places_held_outlives_one_of_them() {
    let history = five_builds();
    let (_dir, root) = scratch();
    let out = root.join("index");
    index_of(&history, 3, &out);
    let four = Memory {
        commits: history.commits[..4].to_vec(),
    };
    Indexer::new(&four, "memory").append(&out).unwrap();
    let key = FileKey::parse(A).unwrap();
    {
        let index = SqliteHistory::open(&out).unwrap();
        assert_eq!(index.file(key).unwrap().unwrap().life, Life::new(0, 3));
        let path = index.path(WadHash(3)).unwrap().unwrap();
        assert_eq!(path.life, Life::new(2, 2));
        assert!(!index.reaches_head(path.life));
    }

    // The path comes back with the fifth build and keeps its first.
    Indexer::new(&history, "memory").append(&out).unwrap();
    let index = SqliteHistory::open(&out).unwrap();
    assert_eq!(
        index.path(WadHash(3)).unwrap().unwrap().life,
        Life::new(2, 4)
    );
    assert_eq!(index.held(WadHash(3), key).unwrap(), Some(Life::new(2, 4)));
}

#[test]
fn an_index_of_another_history_is_not_appended_to() {
    let history = five_builds();
    let (_dir, root) = scratch();
    let out = root.join("index");
    index_of(&history, 2, &out);
    let builds = out.join("builds.tsv");
    let text = fs_err::read_to_string(&builds).unwrap();
    fs_err::write(&builds, text.replace("\tc1\t", "\telsewhere\t")).unwrap();
    let error = Indexer::new(&history, "memory").append(&out).unwrap_err();
    assert!(
        matches!(&error, CensusError::Diverged { at: 1, commit } if commit == "elsewhere"),
        "{error}"
    );
    assert!(error.to_string().contains("rebuild"), "{error}");
}

#[test]
fn a_history_shorter_than_the_index_is_not_appended_to() {
    let history = memory();
    let (_dir, root) = scratch();
    let out = root.join("index");
    index_of(&history, 3, &out);
    let shorter = Memory {
        commits: history.commits[..2].to_vec(),
    };
    let error = Indexer::new(&shorter, "memory").append(&out).unwrap_err();
    assert!(
        matches!(error, CensusError::Diverged { at: 2, .. }),
        "{error}"
    );
}

#[test]
fn an_index_whose_object_bytes_another_zstd_wrote_is_not_appended_to() {
    let history = five_builds();
    let (_dir, root) = scratch();
    let out = root.join("index");
    index_of(&history, 2, &out);
    let mut stamp = Stamp::read(&out).unwrap();
    stamp.objects.zstd = "0.0.0".to_string();
    stamp.write(&out).unwrap();
    let error = Indexer::new(&history, "memory").append(&out).unwrap_err();
    assert!(
        matches!(
            &error,
            CensusError::ObjectZstd { stamped, running }
                if stamped == "0.0.0" && running == zstd::zstd_safe::version_string()
        ),
        "{error}"
    );
}

#[test]
fn a_directory_with_no_index_is_not_appended_to() {
    let (_dir, root) = scratch();
    let error = Indexer::new(&memory(), "memory")
        .append(&root.join("absent"))
        .unwrap_err();
    assert!(matches!(error, CensusError::Index(_)), "{error}");
}
