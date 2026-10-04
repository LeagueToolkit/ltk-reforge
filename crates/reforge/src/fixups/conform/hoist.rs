//! The hoist: a struct a tool nested in itself, taken out (#MOD-ARTIFACT-0018). A struct sits
//! in the same-named field of a struct of its own class, where the build reads that field as
//! a plain value. The inner struct's fields take the field's place, and the outer struct
//! keeps its own value where both hold one.
//!
//! A struct that never showed in the field is left for retype to remove.

use ltk_meta::property::values;
use ltk_meta::{PropertyKind as Kind, PropertyValueEnum as V};

use super::never_shown;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{BinHash, FixupInfo, Phase, Record, StructMut};

static INFO: FixupInfo = FixupInfo::built_in(
    "reshape-hoist",
    "A struct nested in the same-named field of a struct of its own class, taken out",
    &[Record::mod_artifact(18)],
    Phase::Conform,
);

/// The struct a pointer or an embed holds, when it is of a class.
fn nested(value: &V, class: BinHash) -> Option<&values::Struct> {
    let held = match value {
        V::Struct(held) => held,
        V::Embedded(embed) => &embed.0,
        _ => return None,
    };
    (held.class_hash == class).then_some(held)
}

/// The fixup that takes a struct out of the same-named field of a struct of its own class.
pub struct Hoist;

impl Hoist {
    /// Whether a field of a struct holds a struct to take out: one of the struct's own class,
    /// where the build reads no struct, and one the mod's own build may have read.
    fn nests(cx: &FixCx<'_>, s: &StructMut<'_>, field: BinHash) -> bool {
        let class = s.class();
        let plain = cx
            .type_of(class, field)
            .is_some_and(|want| !matches!(want.kind, Kind::Struct | Kind::Embedded));
        let held = s.fields().get(&field);
        plain
            && held.is_some_and(|value| {
                nested(value, class).is_some() && !never_shown(cx, class, field, value.kind())
            })
    }

    /// Takes out the struct in a field, and the struct that one holds in the same field, to
    /// the first value that is no struct of the class.
    fn take_out(cx: &mut FixCx<'_>, s: &mut StructMut<'_>, field: BinHash) {
        let class = s.class();
        let at = s.path_of(field);
        let Some(wrapper) = s.fields().get(&field).cloned() else {
            return;
        };
        // Where the struct being taken out was before any was.
        let mut was = at.clone();
        while let Some(inner) = s
            .fields()
            .get(&field)
            .and_then(|value| nested(value, class))
        {
            let inner = inner.properties.clone();
            // A struct with no value under its own field's name leaves nothing in its place.
            if !inner.contains_key(&field) {
                s.fields_mut().shift_remove(&field);
            }
            for (other, value) in inner {
                let from = was.field(class, other);
                if other == field {
                    s.fields_mut().insert(field, value);
                } else if s.fields().contains_key(&other) {
                    cx.removed(from, &value);
                } else {
                    cx.moved(from, s.path_of(other), &value);
                    s.fields_mut().insert(other, value);
                }
            }
            was = was.field(class, field);
        }
        match s.fields().get(&field) {
            Some(value) => cx.converted(at, &wrapper, value),
            None => cx.removed(at, &wrapper),
        }
    }
}

impl BinFixup for Hoist {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        types.schema().is_some() || types.witnessed_len() > 0
    }

    fn classes(&self) -> Classes {
        Classes::Every
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let held: Vec<BinHash> = s.fields().keys().copied().collect();
        for field in held {
            if Self::nests(cx, &s, field) {
                Self::take_out(cx, &mut s, field);
            }
        }
    }
}
