//! The last run of the run page, held in memory until the next replaces it, so the page
//! browses its files whether or not the store kept it (`docs/design/ROADMAP-UX.md`, "Two
//! views of one run"). It holds the run's snapshots and the bytes its stages made that the
//! store did not take: a kept run's bytes are in the run tier, and its layer here is empty.

use std::collections::BTreeMap;
use std::sync::Arc;

use reforge::game::Names;
use reforge::model::{ByteView, ContentId};
use reforge::snapshot::Snapshots;
use reforge::store::{BlobStore, MemoryStore};

use super::{RunRequest, StageName};
use crate::error::AppError;
use crate::store::{self, AppStore, FileView, KeptSnapshots, StageFiles, Tiers};

/// A run that ended, as the app holds it until the next.
pub struct HeldRun {
    request: RunRequest,
    snapshots: Option<Snapshots>,
    layer: Arc<MemoryStore>,
}

impl HeldRun {
    /// A run that ended with its request, its snapshots from `load` on, and the bytes of its
    /// layer the store did not take.
    pub(super) fn new(
        request: RunRequest,
        snapshots: Option<Snapshots>,
        layer: BTreeMap<ContentId, ByteView>,
    ) -> Self {
        let store = MemoryStore::new();
        for bytes in layer.into_values() {
            store.put(bytes);
        }
        Self {
            request,
            snapshots,
            layer: Arc::new(store),
        }
    }

    /// The request the run ran.
    pub fn request(&self) -> &RunRequest {
        &self.request
    }

    /// The snapshots from `load` on. None for a run that did not end `load`.
    pub fn snapshots(&self) -> Option<&Snapshots> {
        self.snapshots.as_ref()
    }

    /// The bytes its stages made that the store did not take.
    pub fn layer(&self) -> &Arc<MemoryStore> {
        &self.layer
    }

    /// The files at a stage, marked as a kept run's are, over the run's layer and the store's
    /// tiers. None for a run that did not end `load`; refused for a stage whose snapshot names
    /// no files, or one the run did not reach.
    pub fn files(
        &self,
        store: Option<&AppStore>,
        stage: StageName,
    ) -> Result<Option<StageFiles>, AppError> {
        let Some(snapshots) = &self.snapshots else {
            return Ok(None);
        };
        let tiers = Tiers::layered(&*self.layer, store);
        KeptSnapshots::of(snapshots).files(stage, &tiers).map(Some)
    }

    /// One file by the id of its bytes, over the run's layer and the store's tiers, a bin's
    /// names spelled through the run's load names and `names`.
    pub fn file(
        &self,
        store: Option<&AppStore>,
        content: ContentId,
        names: Option<&Names>,
    ) -> Result<FileView, AppError> {
        let tiers = Tiers::layered(&*self.layer, store);
        let book = || {
            let load = self.snapshots.as_ref().map(|snapshots| &snapshots.load);
            Ok(load.map(|load| load.names().clone()).unwrap_or_default())
        };
        store::file_view(&tiers, content, book, names)
    }
}

/// One file by the id of its bytes, from the last run when there is one, else from the store's
/// tiers alone. Refused when none holds the bytes.
pub fn held_file(
    held: Option<&HeldRun>,
    store: Option<&AppStore>,
    content: ContentId,
    names: Option<&Names>,
) -> Result<FileView, AppError> {
    match (held, store) {
        (Some(held), store) => held.file(store, content, names),
        (None, Some(store)) => {
            store::file_view(&Tiers::of(store), content, || Ok(Default::default()), names)
        }
        (None, None) => Err(AppError::BlobMissing { id: content.hex() }),
    }
}
