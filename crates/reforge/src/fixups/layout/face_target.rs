//! A clip's event that turns the unit to face the camera, as the class the target declares
//! (#GAME-CHANGE-0026). The class of the event changes and every value stays. An event of the
//! old class crashes the client at load (#GAME-BEHAVIOR-0028).

use ltk_meta::PropertyValueEnum as V;
use ltk_meta::path::MapKey;
use ltk_meta::property::values;

use super::Stated;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "graph-face-target",
    "A clip's event facing the camera, as a FaceTargetEventData",
    &[Record::game_change(26)],
    Phase::Layout {
        build: Version::new(13, 8),
    },
);

/// The clips that hold events: the subclasses of `BlendableClipData`, which declares the map.
const CLIPS: [&str; 2] = ["AtomicClipData", "ParametricClipData"];
const EVENTS: &str = "mEventDataMap";
const OLD: &str = "FaceCameraEventData";
const NEW: &str = "FaceTargetEventData";
/// A field of the new class: the target reads the class when it declares it.
const ROTATION: &str = "YRotationDegrees";

/// #GAME-CHANGE-0026: the number of 13.8.5036621, the first build without the old class.
const NEW_AT: u64 = 5_036_621;

const STATED: Stated = Stated {
    classes: &[OLD, NEW],
    fields: &[EVENTS],
};

/// The fixup of a clip's events of the old class.
pub struct FaceTarget;

impl FaceTarget {
    /// Whether the mod's own build read the old class. True for a mod with no date.
    fn read_at_source(cx: &FixCx<'_>) -> bool {
        cx.source_build().is_none_or(|build| build < NEW_AT)
    }
}

impl BinFixup for FaceTarget {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        types.declares(name_hash(NEW), name_hash(ROTATION))
    }

    fn classes(&self) -> Classes {
        Classes::named(&CLIPS)
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let (events, old, new) = (name_hash(EVENTS), name_hash(OLD), name_hash(NEW));
        let at = s.path_of(events);
        let Some(V::Map(map)) = s.fields_mut().get_mut(&events) else {
            return;
        };
        let is_old = |value: &V| matches!(value, V::Struct(event) if event.class_hash == old);
        if !map.entries().iter().any(|(_, value)| is_old(value)) {
            return;
        }
        // A key that is no map key leaves the change at the map.
        let path_of =
            |key: &V| MapKey::try_from(key).map_or_else(|_| at.clone(), |key| at.key(key));
        if Self::read_at_source(cx) {
            for i in 0..map.entries().len() {
                let (key, before) = &map.entries()[i];
                if !is_old(before) {
                    continue;
                }
                let (path, before) = (path_of(key), before.clone());
                let mut slot = map
                    .slot(i)
                    .unwrap_or_else(|| panic!("the map of events has no entry {i}"));
                let Some(event) = slot.get_mut::<values::Struct>() else {
                    continue;
                };
                event.class_hash = new;
                let after = V::Struct(event.clone());
                cx.converted(path, &before, &after);
            }
        } else {
            // #ADR-0065: no build since the mod's own read the event.
            let (key_kind, value_kind) = (map.key_kind(), map.value_kind());
            let empty = values::Map::empty(key_kind, value_kind).unwrap_or_else(|error| {
                panic!("a map of {key_kind:?} to {value_kind:?} is not made: {error}")
            });
            let entries = std::mem::replace(map, empty).into_entries();
            let mut kept = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                if is_old(&value) {
                    cx.removed(path_of(&key), &value);
                } else {
                    kept.push((key, value));
                }
            }
            *map = values::Map::new(key_kind, value_kind, kept).unwrap_or_else(|error| {
                panic!("the events kept make no map of {key_kind:?} to {value_kind:?}: {error}")
            });
        }
        STATED.teach(cx);
    }
}
