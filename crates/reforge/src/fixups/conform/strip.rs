//! The strip: fields the target build does not declare, removed (#GAME-BEHAVIOR-0002). A
//! field goes when its class exists at the target build, neither the class nor a base of it
//! declares the field there, and no bin of the install holds it on that class.
//!
//! A class the schema does not have at the target build is left whole, and without a schema
//! nothing is stripped. A field some build since the mod's own declared was read when the
//! author made the mod, and is lost. The others never showed.

use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{BinHash, FixupInfo, Phase, Record, StructMut};

static INFO: FixupInfo = FixupInfo::built_in(
    "strip-undeclared",
    "Fields the target build does not declare, removed",
    &[Record::game_behavior(2)],
    Phase::Strip,
);

/// The fixup that removes the fields the target build does not declare.
pub struct StripUndeclared;

impl BinFixup for StripUndeclared {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        types.schema().is_some()
    }

    fn classes(&self) -> Classes {
        Classes::Every
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let class = s.class();
        if cx.types().has_class(class) != Some(true) {
            return;
        }
        let undeclared = |field: &&BinHash| !cx.declares(class, **field);
        let undeclared: Vec<BinHash> = s.fields().keys().filter(undeclared).copied().collect();
        for field in undeclared {
            let Some(value) = s.fields_mut().shift_remove(&field) else {
                continue;
            };
            let path = s.path_of(field);
            // Without a date, or with one before what the schema describes, a build may have
            // read the field.
            let read = cx
                .types_since_source(class, field)
                .is_none_or(|types| !types.is_empty());
            if read {
                cx.lost(path, &value);
            } else {
                cx.removed(path, &value);
            }
        }
    }
}
