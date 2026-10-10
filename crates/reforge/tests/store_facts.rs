//! The facts per blob beside a store (#ADR-0100): a blob's size and what its head says of its
//! kind, kept in a table under the matcher's generation and read in place of the head.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use camino::Utf8PathBuf;
use reforge::diag::Stage;
use reforge::model::{BlobFacts, Blobs, ByteView, ContentId, FactAccess, FactRow, Head};
use reforge::snapshot::RawCache;
use reforge::store::{BlobStore, FactTable, MemoryFacts, MemoryStore};
use reforge::{Porter, RunError};
use reforge_magic::{GENERATION, HeadKind, Kind};
use reforge_testkit::bins::{bin, blob, entry};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::mods::FILE_NAME;
use reforge_testkit::skins::{self, skin_bin};

/// A scratch directory, and the path of a directory inside it that does not exist yet.
fn scratch() -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let path = Utf8PathBuf::try_from(dir.path().join("ingest")).expect("a UTF-8 path");
    (dir, path)
}

/// A store in memory that counts the heads it reads.
#[derive(Default)]
struct Counted {
    blobs: MemoryStore,
    heads: Mutex<usize>,
}

impl Counted {
    fn heads(&self) -> usize {
        *self.heads.lock().expect("the lock")
    }
}

impl BlobStore for Counted {
    fn get(&self, id: ContentId) -> Option<ByteView> {
        self.blobs.get(id)
    }

    fn get_head(&self, id: ContentId, len: usize) -> Option<Head> {
        *self.heads.lock().expect("the lock") += 1;
        self.blobs.get_head(id, len)
    }

    fn put(&self, bytes: ByteView) -> ContentId {
        self.blobs.put(bytes)
    }
}

const BIN: &[u8] = b"PROP\x03\0\0\0 and the rest of a bin";

fn row(kind: &HeadKind, size: u64, generation: u32) -> FactRow {
    FactRow {
        size,
        kind: serde_json::to_string(kind).expect("a kind writes"),
        generation,
    }
}

/// The run's view of a store that counts its heads, over a table.
fn blobs(table: &Arc<dyn FactAccess>) -> (Blobs, Arc<Counted>, ContentId) {
    let store = Arc::new(Counted::default());
    let id = store.put(ByteView::from(BIN));
    let blobs = Blobs::new(Arc::clone(&store) as Arc<_>).with_facts(Arc::clone(table));
    (blobs, store, id)
}

#[test]
fn a_blob_with_no_row_is_read_from_its_head_and_its_row_written() {
    let (_scratch, dir) = scratch();
    let tables: [Arc<dyn FactAccess>; 2] = [
        Arc::new(MemoryFacts::new()),
        Arc::new(FactTable::in_dir(&dir)),
    ];
    for table in tables {
        let (blobs, store, id) = blobs(&table);
        let facts = blobs.facts(id).expect("the store holds it");
        let said = BlobFacts {
            kind: HeadKind::Magic(Kind::PropertyBin),
            size: BIN.len() as u64,
        };
        assert_eq!((facts, store.heads()), (said, 1));
        assert_eq!(
            table.row(id),
            None,
            "the rows are written together, at the end"
        );
        blobs.keep_facts();
        let written = row(&said.kind, said.size, GENERATION);
        assert_eq!(table.row(id), Some(written));
    }
}

#[test]
fn a_row_of_this_generation_is_read_with_no_fetch() {
    let (_scratch, dir) = scratch();
    let tables: [Arc<dyn FactAccess>; 2] = [
        Arc::new(MemoryFacts::new()),
        Arc::new(FactTable::in_dir(&dir)),
    ];
    for table in tables {
        let id = ContentId::of(BIN);
        // A row that says other than the head does shows where the answer came from.
        table.keep_rows(&[(id, row(&HeadKind::Text, 3, GENERATION))]);
        let (blobs, store, _) = blobs(&table);
        let facts = blobs.facts(id).expect("the table holds it");
        assert_eq!((facts.kind, facts.size), (HeadKind::Text, 3));
        assert_eq!(store.heads(), 0, "no head is read");
    }
}

#[test]
fn a_row_of_another_generation_or_kind_is_read_again() {
    let (_scratch, dir) = scratch();
    let tables: [Arc<dyn FactAccess>; 2] = [
        Arc::new(MemoryFacts::new()),
        Arc::new(FactTable::in_dir(&dir)),
    ];
    let id = ContentId::of(BIN);
    let stale = [
        row(&HeadKind::Text, 3, GENERATION - 1),
        row(&HeadKind::Text, 3, GENERATION + 1),
        FactRow {
            size: 3,
            kind: "\"a kind no generation wrote\"".into(),
            generation: GENERATION,
        },
    ];
    for table in tables {
        for kept in &stale {
            table.keep_rows(&[(id, kept.clone())]);
            let (blobs, store, _) = blobs(&table);
            let facts = blobs.facts(id).expect("the store holds it");
            assert_eq!(facts.kind, HeadKind::Magic(Kind::PropertyBin), "{kept:?}");
            assert_eq!(store.heads(), 1);
            blobs.keep_facts();
            let row = table.row(id).expect("a row");
            assert_eq!((row.generation, row.size), (GENERATION, BIN.len() as u64));
        }
    }
}

#[test]
fn a_table_on_disk_is_shared_by_handles_and_made_at_the_first_write() {
    let (_scratch, dir) = scratch();
    let table = FactTable::in_dir(&dir);
    let id = ContentId::of(b"a blob");
    assert_eq!(table.row(id), None);
    assert!(!dir.exists(), "a read makes nothing");
    table.keep_rows(&[(id, row(&HeadKind::Unknown, 6, GENERATION))]);
    let other = FactTable::in_dir(&dir);
    assert_eq!(other.row(id), Some(row(&HeadKind::Unknown, 6, GENERATION)));
    assert_eq!(other.len(), 1);
}

#[test]
fn two_tables_on_one_file_writing_at_once_both_land() {
    let (_scratch, dir) = scratch();
    let writers: Vec<_> = (0..2u8)
        .map(|writer| {
            let dir = dir.clone();
            std::thread::spawn(move || {
                // A connection of its own, as another process's.
                let table = FactTable::in_dir(&dir);
                for batch in 0..20u8 {
                    let rows: Vec<_> = (0..25u8)
                        .map(|n| {
                            let id = ContentId::of(&[writer, batch, n]);
                            (id, row(&HeadKind::Text, u64::from(n), GENERATION))
                        })
                        .collect();
                    table.keep_rows(&rows);
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().expect("a writer ends");
    }
    let table = FactTable::in_dir(&dir);
    assert_eq!(table.len(), 2 * 20 * 25);
    let landed =
        |writer: u8| (0..20u8).all(|b| table.row(ContentId::of(&[writer, b, 24])).is_some());
    assert!(landed(0) && landed(1));
}

#[test]
fn a_run_in_a_cache_writes_the_facts_its_load_learnt_and_the_next_reads_them() {
    let env = env(skins::game(), reforge::history::NoHistory);
    let graph = "Characters/Test/Animations/Skin0";
    let resources = "Characters/Test/Skins/Skin0/Resources";
    let mut files = own_files("a");
    let objects = vec![
        entry(
            "Characters/Test/Skins/Skin0",
            &own_skin("a", graph, resources),
        ),
        own_graph("a", graph),
    ];
    files.push((skin_bin("test", 0), bin(objects)));
    let bytes = archive(&files).build();
    let (_scratch, dir) = scratch();
    let cache = RawCache::new(dir.clone());
    let porter = Porter::new(&env);
    let mut run = porter.run().in_cache(cache.clone());
    run.read(&bytes, FILE_NAME).expect("it reads");
    run.load().expect("the store answers");
    let m = run.module().expect("the mod");
    let ids: BTreeMap<ContentId, u64> = m.files.values().map(|f| (f.content, f.size)).collect();
    let table = cache.facts();
    assert_eq!(table.len(), ids.len(), "a row per blob the files name");
    for (&id, &size) in &ids {
        let kept = table.row(id).expect("a row");
        assert_eq!((kept.size, kept.generation), (size, GENERATION));
    }
    assert_eq!(
        blob("assets/mod/a/mesh.skn").len() as u64,
        ids[&ContentId::of(&blob("assets/mod/a/mesh.skn"))]
    );

    // A second run of the archive finds every row and writes none.
    let mut again = porter.run().in_cache(cache.clone());
    again.read(&bytes, FILE_NAME).expect("it reads");
    match again.load() {
        Ok(_) | Err(RunError::Halted(_)) => {}
        Err(error) => panic!("the second load: {error}"),
    }
    assert_eq!(cache.facts().len(), ids.len());
    assert_eq!(again.ran(Stage::Load), run.ran(Stage::Load));
}
