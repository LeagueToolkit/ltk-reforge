//! A stage's steps: the cancel a stage reads between two of them (#ADR-0097).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Stage;

/// A cancel a caller hands a run. Its clones share one flag: the caller keeps one and sets it,
/// from any thread, and the run reads it between the steps of each stage.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// A cancel that is not set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the flag: the stage under way ends after the step it is on.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether the flag is set.
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A stage ended at a cancel, between two of its steps. The stage gives no result, and no
/// stage after it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the run was cancelled in {0}")]
pub struct Cancelled(pub Stage);
