//! An emitter's simple-particle fields of before 12.19 (#GAME-CHANGE-0004): moved into
//! `LegacySimple`, and removed from an emitter a system lists as complex (#ADR-0068). The
//! client reads no `LegacySimple` on a complex emitter (#GAME-BEHAVIOR-0024).
//!
//! The move is the table of `rules::legacy_simple`. The list that holds the emitter is the last
//! field of its path, which no table rule reads.

use super::Stated;
use crate::fixups::rule::Rule;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes, rules};
use crate::model::{FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "vfx-legacy-simple",
    "An emitter's simple-particle fields, in LegacySimple, or removed from a complex emitter",
    &[Record::game_change(4)],
    Phase::Layout {
        build: Version::new(12, 19),
    },
);

const SYSTEM: &str = "VfxSystemDefinitionData";
const COMPLEX: &str = "complexEmitterDefinitionData";

const STATED: Stated = Stated {
    classes: &[SYSTEM],
    fields: &[COMPLEX],
};

/// The fixup `vfx-legacy-simple`.
pub struct LegacySimple {
    table: Rule,
}

impl Default for LegacySimple {
    fn default() -> Self {
        Self::new()
    }
}

impl LegacySimple {
    /// The fixup, on the table of #GAME-CHANGE-0004.
    pub fn new() -> Self {
        Self {
            table: rules::legacy_simple(),
        }
    }

    /// Whether a system lists the emitter as complex.
    fn complex(s: &StructMut<'_>) -> bool {
        s.path().last_field() == Some((name_hash(SYSTEM), name_hash(COMPLEX)))
    }

    /// Removes the table's fields the target's emitter does not declare. The client draws a
    /// complex emitter the same with them: nothing is lost.
    fn remove(&self, cx: &mut FixCx<'_>, s: &mut StructMut<'_>) {
        let emitter = self.table.class().hash();
        let mut changed = false;
        for field in self.table.sources() {
            if cx.declares(emitter, field.hash()) {
                continue;
            }
            if let Some(value) = s.fields_mut().shift_remove(&field.hash()) {
                cx.removed(s.path_of(field.hash()), &value);
                changed = true;
            }
        }
        if changed {
            self.table.teach(cx.book());
            STATED.teach(cx);
        }
    }
}

impl BinFixup for LegacySimple {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        self.table.applies(types)
    }

    fn classes(&self) -> Classes {
        self.table.classes()
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let sources = self.table.sources();
        if !sources
            .iter()
            .any(|field| s.fields().contains_key(&field.hash()))
        {
            return;
        }
        if Self::complex(&s) {
            self.remove(cx, &mut s);
        } else {
            self.table.fix(cx, s);
        }
    }
}
