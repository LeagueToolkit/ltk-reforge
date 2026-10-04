//! The wrap: a value Riot moved into a struct of its own, where no table rule names the move
//! (#GAME-CHANGE-0005). A field holds a plain value where the build reads a struct whose
//! class has a field of the same name: the value becomes that struct's field, and the fields
//! beside it that the struct's class declares and the outer class does not move in with it.
//!
//! A value that never showed is left for retype to remove.

use ltk_meta::property::values;
use ltk_meta::{PropertyKind as Kind, PropertyValueEnum as V};

use super::never_shown;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{BinHash, Fields, FixupInfo, Phase, Record, StructMut};

static INFO: FixupInfo = FixupInfo::built_in(
    "reshape-wrap",
    "A value where the build reads a struct that holds it, wrapped in that struct",
    &[Record::game_change(5)],
    Phase::Conform,
);

/// Whether a struct can be made around a value: it is no struct, no collection and not
/// nothing.
fn plain(value: &V) -> bool {
    !matches!(
        value,
        V::None(_)
            | V::Struct(_)
            | V::Embedded(_)
            | V::Container(_)
            | V::UnorderedContainer(_)
            | V::Optional(_)
            | V::Map(_)
    )
}

/// The struct the build reads in a field.
struct Child {
    class: BinHash,
    /// A pointer or an embed.
    kind: Kind,
}

impl Child {
    /// The struct to make around the value of a field of a class. None when the build reads
    /// no struct there, the struct's class has no field of the same name, or the value never
    /// showed.
    fn around(cx: &FixCx<'_>, class: BinHash, field: BinHash, value: &V) -> Option<Self> {
        let want = cx.type_of(class, field)?;
        let child = Self {
            class: want.class?,
            kind: want.kind,
        };
        // A struct made in a struct of its own class would be wrapped again inside it.
        let wraps = matches!(child.kind, Kind::Struct | Kind::Embedded)
            && child.class != class
            && plain(value)
            && cx.declares(child.class, field)
            && !never_shown(cx, class, field, value.kind());
        wraps.then_some(child)
    }

    fn holding(&self, fields: Fields) -> V {
        let made = values::Struct {
            class_hash: self.class,
            properties: fields,
        };
        match self.kind {
            Kind::Struct => V::Struct(made),
            _ => V::Embedded(values::Embedded(made)),
        }
    }
}

/// The fixup that wraps a value in the struct the target build reads it in.
pub struct Wrap;

impl BinFixup for Wrap {
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
        let class = s.class();
        let held: Vec<BinHash> = s.fields().keys().copied().collect();
        for field in held {
            // A field an earlier wrap moved is gone from the struct.
            let Some(value) = s.fields().get(&field) else {
                continue;
            };
            let Some(child) = Child::around(cx, class, field, value) else {
                continue;
            };
            let moving = |other: &&BinHash| {
                **other != field
                    && cx.declares(child.class, **other)
                    && !cx.declares(class, **other)
            };
            let moving: Vec<BinHash> = s.fields().keys().filter(moving).copied().collect();
            let at = s.path_of(field);
            let mut inner = Fields::new();
            cx.moved(at.clone(), at.field(child.class, field), value);
            inner.insert(field, value.clone());
            for other in moving {
                if let Some(value) = s.fields_mut().shift_remove(&other) {
                    cx.moved(s.path_of(other), at.field(child.class, other), &value);
                    inner.insert(other, value);
                }
            }
            s.fields_mut().insert(field, child.holding(inner));
        }
    }
}
