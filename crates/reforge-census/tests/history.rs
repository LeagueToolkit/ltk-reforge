//! The census history itself, when `REFORGE_CENSUS_REPO` names a clone: every file at the head
//! reads. Ignored by default. Run with `--ignored`.

use std::collections::{BTreeMap, HashSet};

use camino::Utf8Path;
use rayon::prelude::*;
use reforge::model::{ClipKey, FileKey};
use reforge_census::format;
use reforge_census::{ChangeSource, GitRepo};

/// What an entry's file at the head reads as.
struct EntryRead {
    kind: String,
    /// Whether it has what the index reads beyond its content.
    more: bool,
    content: FileKey,
    clips: Vec<ClipKey>,
}

/// What one file at the head reads as.
enum Read {
    /// An entry's file. None when it does not parse.
    Entry(Option<EntryRead>),
    /// An entry text, and whether it parses.
    Text(bool),
}

fn read(path: &str, text: &str) -> Option<Read> {
    if format::entry_file(path).is_some() {
        let entry = format::parse_entry(text).map(|e| EntryRead {
            more: e.skeleton.is_some()
                || e.mesh.is_some()
                || e.texture.is_some()
                || !e.clips.is_empty()
                || !e.objects.is_empty(),
            clips: e.clips.iter().map(|c| c.clip).collect(),
            content: e.content,
            kind: e.kind,
        });
        return Some(Read::Entry(entry));
    }
    format::object_file(path)?;
    Some(Read::Text(format::parse_object_text(text).is_some()))
}

#[test]
#[ignore = "reads the census history named by REFORGE_CENSUS_REPO"]
fn every_file_at_the_head_reads() {
    let repo = std::env::var("REFORGE_CENSUS_REPO")
        .expect("REFORGE_CENSUS_REPO names a clone of the census history");
    let git = GitRepo::open(Utf8Path::new(&repo)).unwrap();
    let head = git.head();
    git.build(&head).unwrap();
    let changes = git.changes(&head, None, true).unwrap();
    let all: Vec<(&str, Read)> = changes
        .par_iter()
        .filter_map(|change| {
            let bytes = git.read(change.new.as_ref()?).unwrap();
            let read = read(&change.path, &String::from_utf8_lossy(&bytes))?;
            Some((change.path.as_str(), read))
        })
        .collect();
    let (mut entries, mut texts) = (0usize, 0usize);
    let mut unread: Vec<&str> = Vec::new();
    let mut kinds: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let (mut contents, mut clips) = (HashSet::new(), HashSet::new());
    for (path, read) in all {
        match read {
            Read::Entry(Some(entry)) => {
                entries += 1;
                contents.insert(entry.content);
                clips.extend(entry.clips);
                let row = kinds.entry(entry.kind).or_default();
                row.0 += 1;
                row.1 += usize::from(entry.more);
            }
            Read::Entry(None) => {
                entries += 1;
                unread.push(path);
            }
            Read::Text(parses) => {
                texts += 1;
                if !parses {
                    unread.push(path);
                }
            }
        }
    }
    for (kind, (n, with)) in &kinds {
        eprintln!("{kind:>12} {n:>8} entries, {with:>8} with what the index reads");
    }
    eprintln!(
        "{entries} entries and {texts} texts, {} unread",
        unread.len()
    );
    eprintln!(
        "{} distinct contents, {} distinct clips",
        contents.len(),
        clips.len()
    );
    for path in unread.iter().take(20) {
        eprintln!("unread: {path}");
    }
    assert!(unread.is_empty());
}
