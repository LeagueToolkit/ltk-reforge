//! A birth translation inside a `VfxShapeLegacy` (#MOD-ARTIFACT-0019): a converter's rendering
//! of the 14.1 change, which kept the old shape's `birthTranslation` in a class that never
//! declared it. It is the emitter's `EmitterPosition`.
//!
//! The fixup is a tool's damage and still runs in the layout phase, at 14.1 and after
//! `vfx-spawn-shape` by its id. An emitter that holds both an old `shape` and a converter's
//! spawn shape takes its position from the `shape`, the one its own build read. In the undo
//! phase this fixup would run first and give the converter's value the position.

use ltk_meta::PropertyValueEnum as V;

use super::Stated;
use super::spawn_shape::{
    BIRTH_TRANSLATION, EMITTER, EMITTER_POSITION, LEGACY, SPAWN_SHAPE, target_reads_spawn_shape,
};
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "vfx-spawn-shape-birth-translation",
    "A birth translation inside a VfxShapeLegacy, as EmitterPosition",
    &[Record::mod_artifact(19)],
    Phase::Layout {
        build: Version::new(14, 1),
    },
);

const STATED: Stated = Stated {
    classes: &[LEGACY],
    fields: &[SPAWN_SHAPE, BIRTH_TRANSLATION, EMITTER_POSITION],
};

/// The fixup of a birth translation a converter left inside a `VfxShapeLegacy`.
pub struct BirthTranslation;

impl BinFixup for BirthTranslation {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        let misplaced = types.declares(name_hash(LEGACY), name_hash(BIRTH_TRANSLATION));
        target_reads_spawn_shape(types) && !misplaced
    }

    fn classes(&self) -> Classes {
        Classes::named(&[EMITTER])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let (spawn_shape, position) = (name_hash(SPAWN_SHAPE), name_hash(EMITTER_POSITION));
        let (legacy, translation) = (name_hash(LEGACY), name_hash(BIRTH_TRANSLATION));
        let Some(V::Struct(shape)) = s.fields_mut().get_mut(&spawn_shape) else {
            return;
        };
        let held = shape.properties.get(&translation);
        let read = |value: &V| cx.read_at_source(legacy, translation, value.kind());
        if shape.class_hash != legacy || !held.is_some_and(read) {
            return;
        }
        let Some(value) = shape.properties.shift_remove(&translation) else {
            return;
        };
        let was = s.path_of(spawn_shape).field(legacy, translation);
        // The position the emitter holds is the one the target reads, and no build read the
        // one in the shape: nothing is lost by it.
        if s.fields().contains_key(&position) {
            cx.removed(was, &value);
        } else {
            cx.moved(was, s.path_of(position), &value);
            s.fields_mut().insert(position, value);
        }
        STATED.teach(cx);
    }
}
