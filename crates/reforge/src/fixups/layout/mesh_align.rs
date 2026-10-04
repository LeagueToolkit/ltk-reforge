//! A primitive mesh's camera alignment (#GAME-CHANGE-0017). No rename: a mesh aligned on
//! either axis comes out aligned on both, so the two old fields are read together.

use ltk_meta::property::values;
use ltk_meta::{PropertyKind as Kind, PropertyValueEnum as V};

use super::Stated;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{BinHash, FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "vfx-mesh-align",
    "A primitive mesh aligned to the camera on either axis, aligned on both",
    &[Record::game_change(17)],
    Phase::Layout {
        build: Version::new(13, 8),
    },
);

/// The classes that held the old fields. The new ones are on their base.
const CLASSES: [&str; 2] = ["VfxPrimitiveMesh", "VfxPrimitiveAttachedMesh"];

/// The alignment on one axis: its field up to 13.7, and from 13.8.
struct Axis {
    old: &'static str,
    new: &'static str,
}

const AXES: [Axis; 2] = [
    Axis {
        old: "mAlignYawToCamera",
        new: "AlignYawToCamera",
    },
    Axis {
        old: "mAlignPitchToCamera",
        new: "AlignPitchToCamera",
    },
];

const STATED: Stated = Stated {
    classes: &CLASSES,
    fields: &[AXES[0].old, AXES[0].new, AXES[1].old, AXES[1].new],
};

fn truth(value: &V) -> bool {
    match value {
        V::Bool(flag) => flag.value,
        V::BitBool(flag) => flag.value,
        _ => false,
    }
}

/// The fixup of a primitive mesh's camera alignment.
pub struct MeshAlign;

impl MeshAlign {
    /// Whether the target build reads both new fields on a class.
    fn reads_new(types: &TargetTypes<'_>, class: BinHash) -> bool {
        let mut new = AXES.iter().map(|axis| name_hash(axis.new));
        new.all(|field| types.declares(class, field))
    }

    /// An alignment in the kind the target build reads the field as.
    fn aligned(cx: &FixCx<'_>, class: BinHash, field: BinHash) -> V {
        match cx.type_of(class, field).map(|field_type| field_type.kind) {
            Some(Kind::BitBool) => V::BitBool(values::BitBool::new(true)),
            _ => V::Bool(values::Bool::new(true)),
        }
    }
}

impl BinFixup for MeshAlign {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        let mut classes = CLASSES.iter().map(|class| name_hash(class));
        classes.any(|class| Self::reads_new(types, class))
    }

    fn classes(&self) -> Classes {
        Classes::named(&CLASSES)
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let class = s.class();
        if !Self::reads_new(cx.types(), class) {
            return;
        }
        // Whether each axis was on, among the old fields the mod's own build read.
        let read: Vec<Option<bool>> = AXES
            .iter()
            .map(|axis| {
                let old = name_hash(axis.old);
                let value = s.fields().get(&old)?;
                cx.read_at_source(class, old, value.kind())
                    .then(|| truth(value))
            })
            .collect();
        if read.iter().all(Option::is_none) {
            return;
        }
        let aligned = read.contains(&Some(true));
        // Both old fields go before a new one is set: the new ones take the last places, yaw
        // first.
        let old: Vec<Option<V>> = AXES
            .iter()
            .map(|axis| s.fields_mut().shift_remove(&name_hash(axis.old)))
            .collect();
        for ((axis, was), on) in AXES.iter().zip(old).zip(read) {
            let from = s.path_of(name_hash(axis.old));
            let new = name_hash(axis.new);
            let held = s.fields().contains_key(&new);
            match was {
                // The value the mesh holds under the new name is the one the target reads.
                Some(was) if on == Some(true) && held => cx.lost(from, &was),
                Some(was) if on == Some(true) => cx.moved(from, s.path_of(new), &was),
                // False is the default of the new field, and a value the mod's own build did
                // not read never showed.
                Some(was) => cx.removed(from, &was),
                None => {}
            }
            if aligned && !held {
                let value = Self::aligned(cx, class, new);
                if on != Some(true) {
                    cx.added(s.path_of(new), &value);
                }
                s.fields_mut().insert(new, value);
            }
        }
        STATED.teach(cx);
    }
}
