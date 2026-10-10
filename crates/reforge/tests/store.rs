//! The blob store (#ADR-0093): the memory store, a layer over another store, and the ingest
//! store on disk; and each one's spool (#ADR-0100).

use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;

use camino::Utf8PathBuf;
use reforge::model::{ByteView, ContentId};
use reforge::store::{BlobStore, IngestStore, Layered, MemoryStore};

/// A scratch directory, and the path of a store inside it that does not exist yet.
fn scratch() -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let path = Utf8PathBuf::try_from(dir.path().join("ingest")).expect("a UTF-8 path");
    (dir, path)
}

/// Every file under a directory.
fn files(dir: &camino::Utf8Path) -> Vec<Utf8PathBuf> {
    let mut found = Vec::new();
    let mut open = vec![dir.to_path_buf()];
    while let Some(dir) = open.pop() {
        for entry in fs_err::read_dir(&dir).expect("the directory reads") {
            let path = Utf8PathBuf::try_from(entry.expect("an entry").path()).expect("UTF-8");
            if path.is_dir() {
                open.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found
}

#[test]
fn the_ingest_store_keeps_bytes_on_disk_under_their_content_id() {
    let (_scratch, dir) = scratch();
    let store = IngestStore::new(&dir);
    let bytes = b"the bytes of a file of a mod, more than twenty of them";
    let id = store.put(ByteView::from(&bytes[..]));
    assert_eq!(id, ContentId::of(bytes));
    assert!(store.contains(id));
    assert_eq!(store.get(id).as_deref(), Some(&bytes[..]));
    let reopened = IngestStore::new(&dir);
    assert_eq!(
        reopened.get(id).as_deref(),
        Some(&bytes[..]),
        "another handle"
    );
    let held = files(&dir);
    assert_eq!(held.len(), 1, "one file per blob and no index: {held:?}");
    assert_eq!(held[0].file_name(), Some(&id.hex()[4..]), "named by the id");
}

#[test]
fn the_ingest_store_writes_the_same_bytes_once() {
    let (_scratch, dir) = scratch();
    let store = IngestStore::new(&dir);
    let first = store.put(ByteView::from(b"twice on disk"));
    let second = store.put(ByteView::from(b"twice on disk"));
    assert_eq!(first, second);
    assert_eq!(files(&dir).len(), 1);
    let empty = store.put(ByteView::from(&b""[..]));
    assert_eq!(store.get(empty).as_deref(), Some(&b""[..]), "an empty blob");
}

#[test]
fn the_empty_blobs_id_is_the_sha256_of_no_bytes() {
    assert_eq!(ContentId::of(&[]), ContentId::EMPTY);
    assert_eq!(
        ContentId::EMPTY.hex(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn every_store_answers_the_empty_blob_without_holding_it() {
    let (_scratch, dir) = scratch();
    let ingest = IngestStore::new(&dir);
    let memory = MemoryStore::new();
    let layered = Layered::new(MemoryStore::new(), MemoryStore::new());
    let stores: [&dyn BlobStore; 3] = [&ingest, &memory, &layered];
    for store in stores {
        assert!(store.contains(ContentId::EMPTY), "before any put");
        assert_eq!(store.get(ContentId::EMPTY).as_deref(), Some(&b""[..]));
        assert_eq!(
            store.get_unchecked(ContentId::EMPTY).as_deref(),
            Some(&b""[..])
        );
        assert_eq!(store.put(ByteView::from(&b""[..])), ContentId::EMPTY);
    }
    assert!(!dir.exists(), "no file is written for it");
    assert!(memory.is_empty());
    assert!(layered.top().is_empty());
}

#[test]
fn a_head_is_the_first_bytes_and_the_whole_size_in_every_store() {
    let (_scratch, dir) = scratch();
    let ingest = IngestStore::new(&dir);
    let memory = MemoryStore::new();
    let layered = Layered::new(MemoryStore::new(), MemoryStore::new());
    let bytes = b"a blob longer than the head it is asked for";
    let stores: [&dyn BlobStore; 3] = [&ingest, &memory, &layered];
    for store in stores {
        let id = store.put(ByteView::from(&bytes[..]));
        let head = store.get_head(id, 7).expect("the store holds it");
        assert_eq!((&*head.bytes, head.size), (&bytes[..7], bytes.len() as u64));
        let whole = store.get_head(id, 4096).expect("the store holds it");
        assert_eq!(&*whole.bytes, &bytes[..], "a short blob is all head");
        assert_eq!(store.get_head(ContentId::of(b"absent"), 7), None);
        let empty = store.get_head(ContentId::EMPTY, 7).expect("the empty blob");
        assert_eq!((empty.bytes.len(), empty.size), (0, 0));
    }
    let under = layered.under().put(ByteView::from(&b"under the layer"[..]));
    let head = layered.get_head(under, 5).expect("a layer reads through");
    assert_eq!(&*head.bytes, b"under");
}

#[test]
fn the_ingest_store_answers_nothing_for_an_id_it_lacks() {
    let (_scratch, dir) = scratch();
    let store = IngestStore::new(&dir);
    let absent = ContentId::of(b"never put");
    assert!(!store.contains(absent));
    assert_eq!(store.get(absent), None, "before the directory exists");
    store.put(ByteView::from(b"something else"));
    assert_eq!(store.get(absent), None, "after");
}

#[test]
fn a_blob_that_fails_its_hash_answers_nothing_and_is_written_again() {
    let (_scratch, dir) = scratch();
    let store = IngestStore::new(&dir);
    let id = store.put(ByteView::from(b"bytes a disk will spoil"));
    let held = files(&dir);
    fs_err::write(&held[0], b"spoilt").expect("the blob's file writes");
    assert_eq!(store.get(id), None);
    assert!(!store.contains(id), "the spoilt file is removed");
    store.put(ByteView::from(b"bytes a disk will spoil"));
    assert_eq!(
        store.get(id).as_deref(),
        Some(&b"bytes a disk will spoil"[..])
    );
}

#[test]
fn a_get_unchecked_of_the_ingest_store_reads_the_blobs_file_as_it_is() {
    // #ADR-0100: a run reads what it put or found whole without hashing it again.
    let (_scratch, dir) = scratch();
    let store = IngestStore::new(&dir);
    let id = store.put(ByteView::from(b"bytes a run reads back unchecked"));
    assert_eq!(
        store.get_unchecked(id).as_deref(),
        Some(&b"bytes a run reads back unchecked"[..])
    );
    assert!(BlobStore::contains(&store, id));
    let held = files(&dir);
    fs_err::write(&held[0], b"spoilt").expect("the blob's file writes");
    assert_eq!(store.get_unchecked(id).as_deref(), Some(&b"spoilt"[..]));
    assert_eq!(store.get(id), None, "a checked get refuses it");
    assert_eq!(store.get_unchecked(ContentId::of(b"never put")), None);
}

/// Writes bytes into a store's spool in two parts and reads them back from the second byte,
/// with what the store's directory holds while the reader is open.
fn spooled(store: &dyn BlobStore, dir: &camino::Utf8Path, bytes: &[u8]) -> (Vec<u8>, usize) {
    let mut spool = store.spool().expect("a spool opens");
    let (first, rest) = bytes.split_at(bytes.len() / 2);
    spool.write_all(first).expect("the spool writes");
    spool.write_all(rest).expect("the spool writes");
    let mut read = spool.finish().expect("the spool reads");
    read.seek(SeekFrom::Start(1)).expect("the bytes seek");
    let mut back = Vec::new();
    read.read_to_end(&mut back).expect("the bytes read");
    let held = if dir.exists() { files(dir).len() } else { 0 };
    (back, held)
}

#[test]
fn a_spool_of_the_ingest_store_is_a_scratch_file_removed_once_read() {
    // #ADR-0100: a large entry inflated once onto disk, never kept as a blob.
    let (_scratch, dir) = scratch();
    let store = IngestStore::new(&dir);
    let bytes = b"an entry too large to hold in memory, in two parts";
    let (back, held) = spooled(&store, &dir, bytes);
    assert_eq!(back, &bytes[1..]);
    assert_eq!(held, 1, "the scratch file, while it is read");
    assert!(!store.contains(ContentId::of(bytes)), "no blob");
    assert_eq!(
        files(&dir),
        Vec::<Utf8PathBuf>::new(),
        "removed when dropped"
    );

    let mut spool = store.spool().expect("a spool opens");
    spool.write_all(bytes).expect("the spool writes");
    drop(spool);
    assert_eq!(files(&dir), Vec::<Utf8PathBuf>::new(), "removed unfinished");
}

/// The id of a process that ran and is gone.
fn dead_process() -> u32 {
    let exe = std::env::current_exe().expect("the test's own program");
    let mut child = std::process::Command::new(exe)
        .arg("--list")
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("the test's program starts");
    child.wait().expect("it ends");
    child.id()
}

#[test]
fn a_scratch_file_whose_process_is_gone_is_removed_when_the_store_opens() {
    let (_scratch, dir) = scratch();
    let scratch = dir.join("scratch");
    fs_err::create_dir_all(&scratch).expect("the directory is made");
    fs_err::write(scratch.join(format!("{}-gone", dead_process())), b"left")
        .expect("the file writes");
    fs_err::write(scratch.join(".tmp-unnamed"), b"left").expect("the file writes");
    let store = IngestStore::new(&dir);
    assert_eq!(files(&dir), Vec::<Utf8PathBuf>::new());
    store.put(ByteView::from(b"a blob"));
    IngestStore::new(&dir);
    assert_eq!(files(&dir).len(), 1, "the blobs stay");
}

#[test]
fn a_scratch_file_of_a_running_process_survives_an_open_for_a_day() {
    let (_scratch, dir) = scratch();
    let scratch = dir.join("scratch");
    fs_err::create_dir_all(&scratch).expect("the directory is made");
    let live = scratch.join(format!("{}-reading", std::process::id()));
    fs_err::write(&live, b"read by this process").expect("the file writes");
    IngestStore::new(&dir);
    assert!(live.exists(), "a running process's file stays");

    let two_days = std::time::Duration::from_secs(2 * 24 * 60 * 60);
    let file = fs_err::File::options()
        .write(true)
        .open(&live)
        .expect("it opens");
    file.set_modified(std::time::SystemTime::now() - two_days)
        .expect("its time is set");
    drop(file);
    IngestStore::new(&dir);
    assert!(
        !live.exists(),
        "a file older than a day goes whatever its name says"
    );
}

#[test]
fn a_spool_of_the_ingest_store_is_named_by_its_process() {
    let (_scratch, dir) = scratch();
    let store = IngestStore::new(&dir);
    let mut spool = store.spool().expect("a spool opens");
    spool.write_all(b"spooled").expect("the spool writes");
    let named = files(&dir);
    assert_eq!(named.len(), 1);
    let name = named[0].file_name().expect("a name");
    let prefix = format!("{}-", std::process::id());
    assert!(name.starts_with(&prefix), "{name}");
    assert!(name.len() > prefix.len(), "a random part: {name}");
    IngestStore::new(&dir);
    assert_eq!(files(&dir).len(), 1, "another open leaves it");
}

#[test]
fn a_spool_of_a_store_in_memory_keeps_nothing_in_it() {
    let (_scratch, dir) = scratch();
    let memory = MemoryStore::new();
    let bytes = b"held by the spool alone";
    assert_eq!(spooled(&memory, &dir, bytes), (bytes[1..].to_vec(), 0));
    assert!(memory.is_empty());
    let layered = Layered::new(MemoryStore::new(), IngestStore::new(&dir));
    let (back, _) = spooled(&layered, &dir, bytes);
    assert_eq!(back, &bytes[1..], "the layer's own");
    assert!(layered.top().is_empty());
    assert!(!dir.exists(), "nothing reaches the store under the layer");
}

#[test]
fn two_ingest_stores_on_one_directory_write_and_read_from_two_threads() {
    let (_scratch, dir) = scratch();
    let blobs: Vec<Vec<u8>> = (0..64u32)
        .map(|n| {
            format!("blob {n} ")
                .repeat(1 + n as usize * 997)
                .into_bytes()
        })
        .collect();
    let ids: Vec<ContentId> = blobs.iter().map(|b| ContentId::of(b)).collect();
    std::thread::scope(|scope| {
        for turn in 0..2 {
            let (dir, blobs, ids) = (&dir, &blobs, &ids);
            scope.spawn(move || {
                let store = IngestStore::new(dir);
                // The two threads put the same blobs, in opposite orders.
                let order: Vec<usize> = match turn {
                    0 => (0..blobs.len()).collect(),
                    _ => (0..blobs.len()).rev().collect(),
                };
                for at in order {
                    assert_eq!(store.put(ByteView::from(&blobs[at][..])), ids[at]);
                    let got = store.get(ids[at]).expect("a blob put is held");
                    assert_eq!(&*got, &blobs[at][..]);
                }
            });
        }
    });
    let store = IngestStore::new(&dir);
    for (id, bytes) in ids.iter().zip(&blobs) {
        assert_eq!(store.get(*id).as_deref(), Some(&bytes[..]));
    }
    assert_eq!(files(&dir).len(), blobs.len(), "one file per blob");
}

#[test]
fn a_put_gives_the_content_id_and_a_get_by_it_gives_the_bytes() {
    let store = MemoryStore::new();
    assert!(store.is_empty());
    let id = store.put(ByteView::from(b"some bytes of a file"));
    assert_eq!(id, ContentId::of(b"some bytes of a file"));
    assert_eq!(store.get(id).as_deref(), Some(&b"some bytes of a file"[..]));
    assert_eq!(store.get(ContentId::of(b"other bytes")), None);
}

#[test]
fn the_same_bytes_put_twice_are_held_once() {
    let store = MemoryStore::new();
    let first = store.put(ByteView::from(b"twice"));
    let second = store.put(ByteView::from(b"twice"));
    assert_eq!(first, second);
    assert_eq!(store.len(), 1);
    assert_eq!(store.ids(), [first]);
}

#[test]
fn a_get_is_a_view_of_the_blob_held() {
    let store = MemoryStore::new();
    // A blob over 20 bytes is on the heap: a shorter one is inlined in the view.
    let id = store.put(ByteView::from(b"a view of a blob, and not a copy of it"));
    let got = store.get(id).expect("held");
    assert_eq!(
        got.ref_count(),
        2,
        "the store and the caller share one blob"
    );
    assert_eq!(&*got.slice(2..6), b"view");
    assert_eq!(store.into_blobs().into_keys().collect::<Vec<_>>(), [id]);
}

#[test]
fn a_layer_answers_from_its_own_blobs_then_from_the_store_under_it() {
    let under = MemoryStore::new();
    let below = under.put(ByteView::from(b"under"));
    let layer = Layered::new(MemoryStore::new(), &under);
    let above = layer.put(ByteView::from(b"on top"));
    assert_eq!(layer.get(below).as_deref(), Some(&b"under"[..]));
    assert_eq!(layer.get(above).as_deref(), Some(&b"on top"[..]));
    assert_eq!(layer.get(ContentId::of(b"nowhere")), None);
    // A put goes into the layer's own store, and nothing reaches the one under it.
    assert_eq!(under.get(above), None);
    assert_eq!(layer.top().ids(), [above]);
    assert_eq!(layer.under().ids(), [below]);
    let (top, _) = layer.into_parts();
    assert_eq!(top.into_blobs().len(), 1);
}

#[test]
fn a_layer_over_a_layer_and_a_shared_store_answer_alike() {
    let shared: Arc<dyn BlobStore> = Arc::new(MemoryStore::new());
    let bottom = shared.put(ByteView::from(b"bottom"));
    let middle = Layered::new(MemoryStore::new(), Arc::clone(&shared));
    let kept = middle.put(ByteView::from(b"middle"));
    let top = Layered::new(MemoryStore::new(), middle);
    assert_eq!(top.get(bottom).as_deref(), Some(&b"bottom"[..]));
    assert_eq!(top.get(kept).as_deref(), Some(&b"middle"[..]));
    let boxed: Box<dyn BlobStore> = Box::new(top);
    assert_eq!(boxed.get(bottom).as_deref(), Some(&b"bottom"[..]));
}
