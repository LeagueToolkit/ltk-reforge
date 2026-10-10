//! The blob store (#ADR-0093): the memory store and a layer over another store.

use std::sync::Arc;

use reforge::model::ContentId;
use reforge::store::{BlobStore, ByteView, Layered, MemoryStore};

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
