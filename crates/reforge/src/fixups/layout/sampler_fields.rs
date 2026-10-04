//! A material sampler's fields (#GAME-CHANGE-0014). The slot's name moved into `TextureName`,
//! the field that held the texture, so no rename rule says it: the target build declares the
//! name the texture leaves, and the two moves are one change in a fixed order.
//!
//! `samplerName` had two meanings. Up to 15.3 it is the slot's name. From 15.21 to 16.12 it
//! is the name of a sampler state, beside a slot's name in `TextureName`. The two are told
//! apart by `TextureName`: a file or nothing there, and `samplerName` is the slot's name.
//! A sampler state's name has no field at the target build and is left for the strip.

use super::Stated;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::value::{as_file, as_str};
use crate::model::{FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "material-sampler-fields",
    "A material sampler's slot in TextureName, its texture in texturePath",
    &[Record::game_change(14)],
    Phase::Layout {
        build: Version::new(15, 3),
    },
);

const SAMPLER: &str = "StaticMaterialShaderSamplerDef";

/// The field that marks the old layouts: the slot's name, which no later layout holds.
const MARKER: &str = "samplerName";

/// The field that holds the texture up to 15.2 and the slot's name from 15.4.
const TEXTURE_NAME: &str = "TextureName";

/// One field's value taken to another field.
struct Move {
    from: &'static str,
    to: &'static str,
}

/// The moves, in order: the texture leaves `TextureName` before the slot's name takes it.
const MOVES: [Move; 2] = [
    Move {
        from: TEXTURE_NAME,
        to: "texturePath",
    },
    Move {
        from: MARKER,
        to: TEXTURE_NAME,
    },
];

const STATED: Stated = Stated {
    classes: &[SAMPLER],
    fields: &[MARKER, TEXTURE_NAME, "texturePath"],
};

/// The fixup of a material sampler's fields.
pub struct SamplerFields;

impl SamplerFields {
    /// Whether a sampler is of a layout from before 15.4, which the mod's own build read: it
    /// holds a `samplerName` some build since the mod's own declared, and no slot's name in
    /// `TextureName`.
    fn old_layout(cx: &FixCx<'_>, s: &StructMut<'_>) -> bool {
        let Some(marker) = s.fields().get(&name_hash(MARKER)) else {
            return false;
        };
        // An empty string names no slot: a sampler of the old layout with no texture.
        let named = |held| as_str(held).is_some_and(|name| !name.is_empty());
        let slot_named = s.fields().get(&name_hash(TEXTURE_NAME));
        let slot_named = slot_named.is_some_and(|held| named(held) && as_file(held).is_none());
        !slot_named && cx.read_at_source(s.class(), name_hash(MARKER), marker.kind())
    }
}

impl BinFixup for SamplerFields {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        let sampler = name_hash(SAMPLER);
        let mut to = MOVES.iter().map(|row| name_hash(row.to));
        !types.declares(sampler, name_hash(MARKER))
            && to.all(|field| types.declares(sampler, field))
    }

    fn classes(&self) -> Classes {
        Classes::named(&[SAMPLER])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        if !Self::old_layout(cx, &s) {
            return;
        }
        for row in &MOVES {
            let (from, to) = (name_hash(row.from), name_hash(row.to));
            let Some(value) = s.fields_mut().shift_remove(&from) else {
                continue;
            };
            // A sampler of 15.3 holds its texture in `texturePath` already: that one is the
            // texture the target reads.
            if s.fields().contains_key(&to) {
                cx.lost(s.path_of(from), &value);
            } else {
                cx.moved(s.path_of(from), s.path_of(to), &value);
                s.fields_mut().insert(to, value);
            }
        }
        STATED.teach(cx);
    }
}
