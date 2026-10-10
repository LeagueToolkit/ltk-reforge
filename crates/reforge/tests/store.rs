//! The blob store (#ADR-0093): the memory store, a layer over another store, and the ingest
//! store on disk.

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
