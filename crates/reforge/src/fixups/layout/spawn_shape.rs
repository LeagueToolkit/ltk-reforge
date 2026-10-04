//! An emitter's spawn shape (#GAME-CHANGE-0006). The old `shape` splits in two: its
//! `birthTranslation` is the emitter's `EmitterPosition`, and its other fields are a
//! `VfxShapeLegacy`, the one class of `SpawnShape` that holds them as they were. No shape is
//! rewritten as a volume, as Riot rewrote some: the record does not establish which.

use ltk_meta::PropertyValueEnum as V;
use ltk_meta::property::values;

use super::Stated;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "vfx-spawn-shape",
    "An emitter's shape, as SpawnShape and EmitterPosition",
    &[Record::game_change(6)],
    Phase::Layout {
        build: Version::new(14, 1),
    },
);

pub(super) const EMITTER: &str = "VfxEmitterDefinitionData";
pub(super) const SPAWN_SHAPE: &str = "SpawnShape";
pub(super) const EMITTER_POSITION: &str = "EmitterPosition";
pub(super) const BIRTH_TRANSLATION: &str = "birthTranslation";
pub(super) const LEGACY: &str = "VfxShapeLegacy";
const SHAPE: &str = "shape";

const STATED: Stated = Stated {
    classes: &[LEGACY],
    fields: &[SHAPE, BIRTH_TRANSLATION, SPAWN_SHAPE, EMITTER_POSITION],
};

/// Whether the target build reads an emitter's shape as 14.1 laid it out.
pub(super) fn target_reads_spawn_shape(types: &TargetTypes<'_>) -> bool {
    let emitter = name_hash(EMITTER);
    types.declares(emitter, name_hash(SPAWN_SHAPE)) && !types.declares(emitter, name_hash(SHAPE))
}

/// The struct an old `shape` is: an embed, or a pointer to something.
fn struct_of(shape: &mut V) -> Option<&mut values::Struct> {
    match shape {
        V::Embedded(values::Embedded(shape)) => Some(shape),
        V::Struct(shape) if shape.class_hash.0 != 0 => Some(shape),
        _ => None,
    }
}

/// The fixup of an emitter's spawn shape.
pub struct SpawnShape;

impl BinFixup for SpawnShape {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        target_reads_spawn_shape(types)
    }

    fn classes(&self) -> Classes {
        Classes::named(&[EMITTER])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let emitter = s.class();
        let (shape, spawn_shape) = (name_hash(SHAPE), name_hash(SPAWN_SHAPE));
        let (position, legacy) = (name_hash(EMITTER_POSITION), name_hash(LEGACY));
        let Some(mut held) = s.fields().get(&shape).cloned() else {
            return;
        };
        let read = cx.read_at_source(emitter, shape, held.kind());
        let Some(old) = struct_of(&mut held).filter(|_| read) else {
            return;
        };
        s.fields_mut().shift_remove(&shape);
        let from = s.path_of(shape);
        let old_class = old.class_hash;
        let translation = old.properties.shift_remove(&name_hash(BIRTH_TRANSLATION));
        let rest = std::mem::take(&mut old.properties);
        if translation.is_none() && rest.is_empty() {
            // An empty shape is the default, as no shape is.
            cx.removed(from.clone(), &held);
        }
        // The spawn shape and the position the emitter holds are the ones the target reads.
        if s.fields().contains_key(&spawn_shape) {
            for (field, value) in &rest {
                cx.lost(from.field(old_class, *field), value);
            }
        } else if !rest.is_empty() {
            let to = s.path_of(spawn_shape);
            for (field, value) in &rest {
                cx.moved(
                    from.field(old_class, *field),
                    to.field(legacy, *field),
                    value,
                );
            }
            let made = values::Struct {
                class_hash: legacy,
                properties: rest,
            };
            s.fields_mut().insert(spawn_shape, V::Struct(made));
        }
        if let Some(translation) = translation {
            let was = from.field(old_class, name_hash(BIRTH_TRANSLATION));
            if s.fields().contains_key(&position) {
                cx.lost(was, &translation);
            } else {
                cx.moved(was, s.path_of(position), &translation);
                s.fields_mut().insert(position, translation);
            }
        }
        STATED.teach(cx);
    }
}
