//! A material's default technique (#GAME-CHANGE-0015). The field went and nothing took its
//! place. No removal rule says it: `"normal"` names the one technique the client uses anyway,
//! and any other value chose a technique, which is lost where the mod's own build read it.

use super::Stated;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::value::as_str;
use crate::model::{FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "material-default-technique",
    "A material's default technique, removed",
    &[Record::game_change(15)],
    Phase::Layout {
        build: Version::new(13, 5),
    },
);

const MATERIAL: &str = "StaticMaterialDef";
const DEFAULT_TECHNIQUE: &str = "defaultTechnique";

/// The technique every material of the game named, and the only one it has.
const NORMAL: &str = "normal";

const STATED: Stated = Stated {
    classes: &[MATERIAL],
    fields: &[DEFAULT_TECHNIQUE],
};

/// The fixup of a material's default technique.
pub struct DefaultTechnique;

impl BinFixup for DefaultTechnique {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        !types.declares(name_hash(MATERIAL), name_hash(DEFAULT_TECHNIQUE))
    }

    fn classes(&self) -> Classes {
        Classes::named(&[MATERIAL])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let field = name_hash(DEFAULT_TECHNIQUE);
        let Some(value) = s.fields_mut().shift_remove(&field) else {
            return;
        };
        let normal = as_str(&value).is_some_and(|name| name.eq_ignore_ascii_case(NORMAL));
        // A choice the mod's own build did not read was copied from an older mod: the author
        // never saw it made.
        if normal || !cx.read_at_source(s.class(), field, value.kind()) {
            cx.removed(s.path_of(field), &value);
        } else {
            cx.lost(s.path_of(field), &value);
        }
        STATED.teach(cx);
    }
}
