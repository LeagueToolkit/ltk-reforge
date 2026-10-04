//! An emission surface (#GAME-CHANGE-0018). The surface's own fields move into the struct
//! `EmissionSurface` points to, whose class is chosen from the schema: the first of the
//! surface classes that declares every field the surface holds. The field that chose the
//! source of the emission goes with the move, as Riot's conversion dropped it.

use ltk_meta::PropertyValueEnum as V;
use ltk_meta::property::values;

use super::Stated;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{BinHash, Fields, FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "vfx-emission-surface",
    "An emission surface's own fields, in EmissionSurface",
    &[Record::game_change(18)],
    Phase::Layout {
        build: Version::new(15, 18),
    },
);

const SURFACE: &str = "VfxEmissionSurfaceData";
const EMISSION_SURFACE: &str = "EmissionSurface";

/// A field of the old layout, which the target's surface does not declare.
const MESH_NAME: &str = "meshName";

/// What chose emission from the avatar's pose: `useAvatarPose` up to 15.17, `EmissionSource`
/// from 15.18 to 15.21. No class holds either from 15.22.
const RETIRED: [&str; 2] = ["EmissionSource", "useAvatarPose"];

/// The classes a surface's fields move into. A surface takes the first that declares all of
/// its fields, and the mesh's when none does.
const CLASSES: [&str; 2] = ["VfxEmissionMeshData", "VfxEmissionSkeletonData"];

const STATED: Stated = Stated {
    classes: &CLASSES,
    fields: &[EMISSION_SURFACE, RETIRED[0], RETIRED[1]],
};

/// The fixup of an emission surface.
pub struct EmissionSurface;

impl EmissionSurface {
    /// The fields of a surface that move: the target's surface does not declare them, the
    /// mod's own build read them, and a surface class declares them.
    fn movable(cx: &FixCx<'_>, s: &StructMut<'_>) -> Vec<BinHash> {
        let surface = s.class();
        let retired = RETIRED.map(name_hash);
        let classes = CLASSES.map(name_hash);
        let moves = |field: BinHash, value: &V| {
            !retired.contains(&field)
                && !cx.declares(surface, field)
                && cx.read_at_source(surface, field, value.kind())
                && classes.iter().any(|class| cx.declares(*class, field))
        };
        let fields = s.fields().iter();
        fields
            .filter(|(field, value)| moves(**field, value))
            .map(|(field, _)| *field)
            .collect()
    }

    /// Removes what chose the source of the emission. Whether any was there.
    fn retire(cx: &mut FixCx<'_>, s: &mut StructMut<'_>) -> bool {
        let mut changed = false;
        for field in RETIRED.map(name_hash) {
            if let Some(value) = s.fields_mut().shift_remove(&field) {
                cx.removed(s.path_of(field), &value);
                changed = true;
            }
        }
        changed
    }
}

impl BinFixup for EmissionSurface {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        let surface = name_hash(SURFACE);
        types.declares(surface, name_hash(EMISSION_SURFACE))
            && !types.declares(surface, name_hash(MESH_NAME))
    }

    fn classes(&self) -> Classes {
        Classes::named(&[SURFACE])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let emission_surface = name_hash(EMISSION_SURFACE);
        if s.fields().contains_key(&emission_surface) {
            return;
        }
        let movable = Self::movable(cx, &s);
        if movable.is_empty() {
            // A surface that holds only what Riot retired gets no struct of its own.
            if Self::retire(cx, &mut s) {
                STATED.teach(cx);
            }
            return;
        }
        let classes = CLASSES.map(name_hash);
        let holds_all = |class: &&BinHash| movable.iter().all(|field| cx.declares(**class, *field));
        let class = *classes.iter().find(holds_all).unwrap_or(&classes[0]);
        let to = s.path_of(emission_surface);
        let mut inner = Fields::new();
        for field in movable {
            // A field only the other class declares stays on the surface.
            if !cx.declares(class, field) {
                continue;
            }
            if let Some(value) = s.fields_mut().shift_remove(&field) {
                cx.moved(s.path_of(field), to.field(class, field), &value);
                inner.insert(field, value);
            }
        }
        Self::retire(cx, &mut s);
        let made = values::Struct {
            class_hash: class,
            properties: inner,
        };
        s.fields_mut().insert(emission_surface, V::Struct(made));
        STATED.teach(cx);
    }
}
