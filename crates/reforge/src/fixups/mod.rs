//! The stage `fix`: a loaded mod's bin values brought to what the target build reads.
//!
//! A fixup is an object that describes itself ([`FixupInfo`]) and fixes one struct at a time
//! ([`BinFixup`]). The engine owns the frame: [`FixupSet::apply`] walks every object of the
//! set once per fixup, in the set's order, and hands each struct of the fixup's classes to it
//! with a [`FixCx`]. A fixup changes the struct's fields and records each change through the
//! context. It writes no sentence and sets no flag: the edited state of an object, the counts
//! of a report and the `fixup.*` findings are read off the [`Changes`].
//!
//! A [`Rule`] is a fixup that is data. A [`FixupSelection`] is the choice of fixups as data.
//!
//! The contract of a fixup: applied twice, it changes nothing the second time; it converts a
//! property whole or leaves it; it has the run's names learn each name it hashes away.
//!
//! `docs/design/ARCHITECTURE.md`, "Fixups".

mod changes;
pub mod conform;
mod context;
mod findings;
pub mod layout;
pub mod rule;
pub mod rules;
mod scope;
mod set;
mod types;
pub mod undo;

use std::sync::Arc;

pub use self::changes::{After, Change, Changes, Counts, Outcome, Snapshot};
pub use self::context::FixCx;
pub use self::findings::{Changed, FieldCount, Lost, Removed, Tally, Unconvertible};
pub use self::rule::Rule;
pub use self::scope::ObjectSet;
pub use self::set::{
    FixError, FixInputs, FixedMod, FixupSelection, FixupSet, SelectionError, Unfixed,
};
pub use self::types::{MAX_CHAIN, TargetTypes, TypeSource};
use crate::diag::{Code, Diag};
use crate::model::{BinHash, FixupInfo, StructMut, name_hash};

/// Every diagnostic code this module raises.
pub const CODES: &[Code] = &[
    Changed::CODE,
    Removed::CODE,
    Lost::CODE,
    Unconvertible::CODE,
];

/// The structs a fixup is called for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classes {
    /// Every struct of every object: a fixup that conforms whole objects to the schema.
    Every,
    /// The structs of these classes.
    Only(Vec<BinHash>),
}

impl Classes {
    /// The structs of the classes of these names.
    pub fn named(names: &[&str]) -> Self {
        Classes::Only(names.iter().map(|name| name_hash(name)).collect())
    }

    /// Whether a fixup is called for a struct of this class.
    pub fn holds(&self, class: BinHash) -> bool {
        match self {
            Classes::Every => true,
            Classes::Only(classes) => classes.contains(&class),
        }
    }
}

/// A fixup on bin values: it brings the structs of its classes to what the target build
/// reads.
pub trait BinFixup: Send + Sync {
    /// What a caller lists and toggles it by.
    fn info(&self) -> &FixupInfo;

    /// Whether the target build has the shape the fixup brings values to. A fixup that does
    /// not apply is skipped for the whole run.
    fn applies(&self, types: &TargetTypes<'_>) -> bool;

    /// The structs it is called for.
    fn classes(&self) -> Classes;

    /// Fixes one struct, and records each value it changes, removes or cannot convert through
    /// the context. The walk goes on into the struct's fields as the fixup leaves them.
    fn fix(&self, cx: &mut FixCx<'_>, s: StructMut<'_>);
}

/// Every built-in fixup on bin values. The order they run in is each one's phase, build and
/// id (#ADR-0007), not their order here.
pub fn built_in() -> Vec<Arc<dyn BinFixup>> {
    let rules = rules::all().into_iter();
    rules
        .map(|rule| Arc::new(rule) as Arc<dyn BinFixup>)
        .chain(undo::all())
        .chain(layout::all())
        .chain(conform::all())
        .collect()
}
