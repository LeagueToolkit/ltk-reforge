//! The census history itself, when `REFORGE_CENSUS_REPO` names a clone: every file at the head
//! reads, and an index of it that `REFORGE_HISTORY_INDEX` names agrees with its trees. Ignored by
//! default. Run with `--ignored`.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;
use reforge::history::{BinEntry, DB_FILE, History, Position, SqliteHistory, wad_order};
use reforge::model::{BinHash, ClipKey, FileKey, WadHash};
use reforge_census::format;
use reforge_census::{ChangeSource, GitRepo};
use rusqlite::{Connection, OpenFlags, params};

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

/// A file a WAD held at a path: the path, the WAD, the content and its checksum.
type Placement = (u64, String, [u8; 16], u64);

/// A bin entry's text in a WAD: the bin, the entry, the WAD and the text's blob.
type Text = (u64, u32, String, [u8; 20]);

/// A bin's links and its objects, by entry hash.
type BinRow = (Vec<WadHash>, Vec<BinEntry>);

/// What one file of a build's tree is, as the index holds it.
enum InTree {
    Placement(Placement, Option<BinRow>),
    Text(Text),
}

fn in_tree(repo: &GitRepo, path: &str, blob: &[u8; 20]) -> Option<InTree> {
    let wad = format::wad_of(path)?.to_string();
    if let Some((bin, entry)) = format::object_file(path) {
        return Some(InTree::Text((bin.0, entry.0, wad, *blob)));
    }
    let at = format::entry_file(path)?;
    let entry = format::parse_entry(&String::from_utf8_lossy(&repo.read(blob).unwrap()))?;
    let bin = entry.links.clone().map(|links| {
        let objects = entry.objects.iter().map(|o| BinEntry {
            entry: o.entry,
            class: o.class,
            object: o.object,
            norm: o.norm,
        });
        let mut objects: Vec<BinEntry> = objects.collect();
        objects.sort_by_key(|o| o.entry);
        objects.dedup_by_key(|o| o.entry);
        (links, objects)
    });
    let placement = (at.0, wad, entry.content.0, entry.checksum);
    Some(InTree::Placement(placement, bin))
}

/// The rows of a table of spans alive at a build, as `select` reads them.
fn alive<T: Ord>(
    db: &Connection,
    select: &str,
    at: Position,
    row: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> BTreeSet<T> {
    let mut statement = db.prepare(select).unwrap();
    let rows = statement.query_map(params![at.0], row).unwrap();
    rows.map(Result::unwrap).collect()
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort_unstable();
    times.get(times.len() / 2).copied().unwrap_or_default()
}

#[test]
#[ignore = "reads the census history named by REFORGE_CENSUS_REPO and an index of it named by \
            REFORGE_HISTORY_INDEX"]
fn an_index_agrees_with_the_trees_of_its_builds() {
    let var = |name: &str| {
        let value = std::env::var(name).unwrap_or_else(|_| panic!("{name} names a folder"));
        Utf8PathBuf::from(value)
    };
    let repo = GitRepo::open(&var("REFORGE_CENSUS_REPO")).unwrap();
    let dir = var("REFORGE_HISTORY_INDEX");
    let index = SqliteHistory::open(&dir).unwrap();
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY;
    let db = Connection::open_with_flags(dir.join(DB_FILE), flags).unwrap();
    let head = index.head().unwrap();
    for at in [Position(head.0 / 2), head] {
        let commit = index.build(at).unwrap().commit.clone();
        let changes = repo.changes(&commit, None, true).unwrap();
        let read: Vec<InTree> = changes
            .par_iter()
            .filter_map(|c| in_tree(&repo, &c.path, c.new.as_ref()?))
            .collect();
        let (mut placements, mut texts) = (BTreeSet::new(), BTreeSet::new());
        let mut bins: BTreeMap<u64, Vec<(String, BinRow)>> = BTreeMap::new();
        for file in read {
            match file {
                InTree::Placement(placement, bin) => {
                    if let Some(bin) = bin {
                        let wad = placement.1.clone();
                        bins.entry(placement.0).or_default().push((wad, bin));
                    }
                    placements.insert(placement);
                }
                InTree::Text(text) => {
                    texts.insert(text);
                }
            }
        }

        let held = alive(
            &db,
            "SELECT p.path, w.path, p.key, p.checksum FROM placements p JOIN wads w \
             ON w.id = p.wad WHERE p.first <= ?1 AND p.last >= ?1",
            at,
            |r| {
                Ok((
                    r.get::<_, i64>(0)? as u64,
                    r.get(1)?,
                    r.get(2)?,
                    r.get::<_, i64>(3)? as u64,
                ))
            },
        );
        assert_eq!(held.len(), placements.len(), "placements at {at}");
        assert!(held == placements, "the placements at {at} are the tree's");
        let indexed = alive(
            &db,
            "SELECT t.bin, t.entry, w.path, t.blob FROM entry_texts t JOIN wads w \
             ON w.id = t.wad WHERE t.first <= ?1 AND t.last >= ?1",
            at,
            |r| Ok((r.get::<_, i64>(0)? as u64, r.get(1)?, r.get(2)?, r.get(3)?)),
        );
        assert_eq!(indexed.len(), texts.len(), "entry texts at {at}");
        assert!(indexed == texts, "the entry texts at {at} are the tree's");

        // A bin at a path is the first WAD's, with its links and its objects.
        for (path, mut held) in bins {
            held.sort_by(|a, b| wad_order(&a.0, &b.0));
            let (wad, (links, entries)) = held.swap_remove(0);
            let bin = index.bin(WadHash(path), at).unwrap().unwrap();
            assert_eq!((bin.wad, bin.links, bin.entries), (wad, links, entries));
        }

        // Every 97th text, read by its blob and by a search of the build's tree.
        let (mut by_blob, mut by_search) = (Vec::new(), Vec::new());
        let sample: BTreeSet<(u64, u32)> = texts.iter().map(|t| (t.0, t.1)).step_by(97).collect();
        for (bin, entry) in &sample {
            let (bin, entry) = (WadHash(*bin), BinHash(*entry));
            let started = Instant::now();
            let read = repo.entry_text_at(&index, bin, entry, at).unwrap();
            by_blob.push(started.elapsed());
            let started = Instant::now();
            let searched = repo.entry_text(bin, entry, &commit).unwrap();
            by_search.push(started.elapsed());
            let read = read.unwrap_or_else(|| panic!("no text of {entry:?} of {bin:?} at {at}"));
            assert_eq!(
                Some(&read),
                searched.as_ref(),
                "{entry:?} of {bin:?} at {at}"
            );
            let parsed = format::parse_object_text(&read);
            assert!(parsed.is_some(), "{entry:?} of {bin:?} at {at} parses");
        }
        eprintln!(
            "build {at}: {} placements, {} texts, {} read both ways: {:?} by blob, {:?} by \
             search (medians)",
            placements.len(),
            texts.len(),
            sample.len(),
            median(by_blob),
            median(by_search)
        );
    }
}
