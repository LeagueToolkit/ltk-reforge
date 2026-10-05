//! An emitter shape a tool wrote where no build reads it, as the `SpawnShape` the build reads
//! (#MOD-ARTIFACT-0031): a shape of a class of 14.1 under the emitter's old `shape`, and a
//! `VfxShapePointDoNotUse` that holds its offset under `constantValue`.
//!
//! No build held either value. The classes are of 14.1, the build that took `shape` off the
//! emitter, and the point declares only `emitOffset`. The fixup needs no date, and runs before
//! the layouts, which would read a `shape` as the author's.

use ltk_meta::PropertyValueEnum as V;
use ltk_meta::property::values;

use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{FixupInfo, Phase, Record, StructMut, name_hash};

const EMITTER: &str = "VfxEmitterDefinitionData";
const SHAPE: &str = "shape";
const SPAWN_SHAPE: &str = "SpawnShape";
const EMITTER_POSITION: &str = "EmitterPosition";
const LEGACY: &str = "VfxShapeLegacy";
const POINT: &str = "VfxShapePointDoNotUse";
const BIRTH_TRANSLATION: &str = "birthTranslation";
const EMIT_OFFSET: &str = "emitOffset";
const CONSTANT_VALUE: &str = "constantValue";

/// The fixup `vfx-tool-shape`.
pub struct ToolShape;

/// Whether a value is a struct of no fields: a `ValueVector3` of nothing.
fn is_empty_struct(value: &V) -> bool {
    match value {
        V::Embedded(values::Embedded(inner)) | V::Struct(inner) => inner.properties.is_empty(),
        _ => false,
    }
}

/// Takes a `shape` of a class of `SpawnShape` off the emitter, and gives its fields to the
/// emitter's `SpawnShape` and `EmitterPosition` where the emitter holds none. Whether it
/// changed the emitter.
fn undo_shape(cx: &mut FixCx<'_>, s: &mut StructMut<'_>) -> bool {
    let (shape, spawn_shape) = (name_hash(SHAPE), name_hash(SPAWN_SHAPE));
    let old = match s.fields().get(&shape) {
        Some(V::Embedded(values::Embedded(old)) | V::Struct(old)) => old.clone(),
        _ => return false,
    };
    if ![name_hash(LEGACY), name_hash(POINT)].contains(&old.class_hash) {
        return false;
    }
    let Some(held) = s.fields_mut().shift_remove(&shape) else {
        return false;
    };
    let from = s.path_of(shape);
    let class = old.class_hash;
    let mut rest = old.properties;
    let translation = rest.shift_remove(&name_hash(BIRTH_TRANSLATION));
    if translation.is_none() && rest.is_empty() {
        cx.removed(from.clone(), &held);
    }
    if s.fields().contains_key(&spawn_shape) {
        // No build read the tool's value: nothing is lost by it.
        for (field, value) in &rest {
            cx.removed(from.field(class, *field), value);
        }
    } else if !rest.is_empty() {
        let to = s.path_of(spawn_shape);
        for (field, value) in &rest {
            cx.moved(from.field(class, *field), to.field(class, *field), value);
        }
        let made = values::Struct {
            class_hash: class,
            properties: rest,
        };
        s.fields_mut().insert(spawn_shape, V::Struct(made));
    }
    if let Some(translation) = translation {
        let position = name_hash(EMITTER_POSITION);
        let was = from.field(class, name_hash(BIRTH_TRANSLATION));
        // The tool empties the translation whose constant it gives the position.
        if s.fields().contains_key(&position) || is_empty_struct(&translation) {
            cx.removed(was, &translation);
        } else {
            cx.moved(was, s.path_of(position), &translation);
            s.fields_mut().insert(position, translation);
        }
    }
    true
}

/// Moves the offset of a `VfxShapePointDoNotUse` in `SpawnShape` from `constantValue` to
/// `emitOffset`, where the point holds none. Whether it changed the emitter.
fn undo_point_offset(cx: &mut FixCx<'_>, s: &mut StructMut<'_>) -> bool {
    let spawn_shape = name_hash(SPAWN_SHAPE);
    let (point, offset) = (name_hash(POINT), name_hash(EMIT_OFFSET));
    let constant = name_hash(CONSTANT_VALUE);
    let at = s.path_of(spawn_shape);
    let Some(V::Embedded(values::Embedded(held)) | V::Struct(held)) =
        s.fields_mut().get_mut(&spawn_shape)
    else {
        return false;
    };
    let misplaced = matches!(held.properties.get(&constant), Some(V::Vector3(_)));
    if held.class_hash != point || !misplaced || held.properties.contains_key(&offset) {
        return false;
    }
    let Some(value) = held.properties.shift_remove(&constant) else {
        return false;
    };
    cx.moved(at.field(point, constant), at.field(point, offset), &value);
    held.properties.insert(offset, value);
    true
}

impl BinFixup for ToolShape {
    fn info(&self) -> &FixupInfo {
        static INFO: FixupInfo = FixupInfo::built_in(
            "vfx-tool-shape",
            "An emitter shape a tool wrote under shape, or a point's offset under constantValue",
            &[Record::mod_artifact(31)],
            Phase::Undo,
        );
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        let emitter = name_hash(EMITTER);
        types.declares(emitter, name_hash(SPAWN_SHAPE))
            && !types.declares(emitter, name_hash(SHAPE))
            && types.declares(name_hash(POINT), name_hash(EMIT_OFFSET))
    }

    fn classes(&self) -> Classes {
        Classes::named(&[EMITTER])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let shape = undo_shape(cx, &mut s);
        let offset = undo_point_offset(cx, &mut s);
        if !shape && !offset {
            return;
        }
        for class in [EMITTER, LEGACY, POINT] {
            cx.learn_class(class);
        }
        let fields = [
            SHAPE,
            SPAWN_SHAPE,
            EMITTER_POSITION,
            BIRTH_TRANSLATION,
            EMIT_OFFSET,
            CONSTANT_VALUE,
        ];
        for field in fields {
            cx.learn_field(field);
        }
    }
}
