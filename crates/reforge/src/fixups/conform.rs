//! The fixups that bring values to the target build's schema, after every layout: reshape,
//! retype, and the strip of what the build does not declare. Each is read off the target
//! build's types, with no table.
//!
//! Reshape is two fixups, one per record: [`Hoist`] takes out a struct a tool nested in
//! itself, [`Wrap`] follows Riot into a struct. Both run in the conform phase and not in the
//! phase their record would give them. A layout rule joins an emitter's fields to the outer
//! of two nested structs, and the hoist then keeps the outer's value: hoisted first, the rule
//! would find the inner's value in its place and drop the emitter's. The wrap takes what no
//! table rule names, so it runs after every rule. Their ids put them before [`Retype`], which
//! types the values they moved.
//!
//! `docs/design/ARCHITECTURE.md`, "Fixups".

mod conversion;
mod hoist;
mod retype;
mod shader_variant;
mod strip;
mod wrap;

use std::sync::Arc;

use ltk_meta::PropertyKind as Kind;

pub use self::conversion::Conversion;
pub use self::hoist::Hoist;
pub use self::retype::Retype;
pub use self::shader_variant::ShaderVariantNearest;
pub use self::strip::StripUndeclared;
pub use self::wrap::Wrap;
use super::{BinFixup, FixCx};
use crate::model::BinHash;

/// Whether a value of a kind in a field never showed (#GAME-BEHAVIOR-0002): the builds from
/// the mod's own to the target's declare the field, and none reads that kind there. False
/// where that is not known.
fn never_shown(cx: &FixCx<'_>, class: BinHash, field: BinHash, kind: Kind) -> bool {
    cx.types_since_source(class, field).is_some_and(|types| {
        !types.is_empty() && types.iter().all(|field_type| field_type.kind != kind)
    })
}

/// Every built-in fixup of this file, in no order: the set orders them by phase and build.
pub fn all() -> Vec<Arc<dyn BinFixup>> {
    vec![
        Arc::new(Hoist),
        Arc::new(Wrap),
        Arc::new(Retype),
        Arc::new(ShaderVariantNearest),
        Arc::new(StripUndeclared),
    ]
}
