//! The fixups that take a tool's damage out, before the layouts it would confuse.

pub mod artifacts;
pub mod strays;

use std::sync::Arc;

pub use self::artifacts::KnownArtifacts;
pub use self::strays::TextureMultStrays;
use super::BinFixup;

/// Every built-in fixup of this file, in no order: the set orders them by phase and build.
pub fn all() -> Vec<Arc<dyn BinFixup>> {
    let rules = artifacts::rules().into_iter();
    let rules = rules.map(|rule| Arc::new(rule) as Arc<dyn BinFixup>);
    let strays: Arc<dyn BinFixup> = Arc::new(TextureMultStrays::new());
    rules.chain([strays]).collect()
}
