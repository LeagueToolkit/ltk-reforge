//! A delta written between two indexes and applied to the first makes the second, byte for
//! byte, and is refused by any other base (#ADR-0109).

mod common;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge::history::{BUILDS_FILE, DB_FILE, Families, Stamp};
use reforge_census::{
    CensusError, DELTA_FORMAT, DeltaStep, Indexer, apply_delta, read_note, write_delta,
};
use tempfile::TempDir;

use self::common::{Memory, memory};

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap().to_owned();
    (dir, root)
}

/// The index of the first `builds` builds of a history.
fn index_of(history: &Memory, builds: usize, out: &Utf8Path) -> Stamp {
    let start = Memory {
        commits: history.commits[..builds].to_vec(),
    };
    Indexer::new(&start, "memory")
        .build(out, Families::ALL)
        .unwrap()
}

#[test]
fn a_delta_applied_to_its_base_makes_the_appended_index_byte_for_byte() {
    let history = memory();
    let (_dir, root) = scratch();
    let base = root.join("base");
    index_of(&history, 1, &base);
    let head = root.join("head");
    fs::create_dir_all(&head).unwrap();
    for file in [DB_FILE, BUILDS_FILE, "stamp.json"] {
        fs::copy(base.join(file), head.join(file)).unwrap();
    }
    let appended = Indexer::new(&history, "memory").append(&head).unwrap();
    assert_eq!(appended.builds, 3);

    let delta = root.join("delta.tar");
    let note = write_delta(&delta, &base, &head).unwrap();
    assert_eq!(note.format, DELTA_FORMAT);
    assert_eq!(note.base_head, "c0");
    assert_eq!(note.head, "c2");
    assert_eq!(note.builds, 2);
    assert_eq!(read_note(&delta).unwrap(), note);

    let out = root.join("applied");
    let mut steps = Vec::new();
    let stamp = apply_delta(&delta, &base, &out, &mut |step| steps.push(step)).unwrap();

    assert_eq!(stamp, appended);
    assert_eq!(
        fs::read(out.join(DB_FILE)).unwrap(),
        fs::read(head.join(DB_FILE)).unwrap()
    );
    assert_eq!(
        fs::read_to_string(out.join(BUILDS_FILE)).unwrap(),
        fs::read_to_string(head.join(BUILDS_FILE)).unwrap()
    );
    assert_eq!(Stamp::read(&out).unwrap(), appended);
    assert_eq!(steps.last(), Some(&DeltaStep::Check));
    assert!(matches!(steps[0], DeltaStep::Write { done: 0, .. }));
}

#[test]
fn a_delta_is_refused_by_an_index_that_is_not_its_base() {
    let history = memory();
    let (_dir, root) = scratch();
    let base = root.join("base");
    index_of(&history, 1, &base);
    let other = root.join("other");
    index_of(&history, 2, &other);
    let head = root.join("head");
    Indexer::new(&history, "memory")
        .build(&head, Families::ALL)
        .unwrap();
    let delta = root.join("delta.tar");
    write_delta(&delta, &base, &head).unwrap();

    let refused = apply_delta(&delta, &other, &root.join("applied"), &mut |_| {});

    assert!(
        matches!(&refused, Err(CensusError::DeltaBase { expected, found }) if expected == "c0" && found == "c1"),
        "{refused:?}"
    );
    assert!(!root.join("applied").exists());
}
