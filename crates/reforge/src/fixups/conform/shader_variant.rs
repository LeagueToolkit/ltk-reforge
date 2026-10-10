//! A material that asks a game shader for a variant the target's shader cache lacks, moved to
//! the nearest variant the cache holds (#ADR-0092): the one define that, set on the material,
//! makes every variant of its passes one the cache holds. The client crashes at the draw of the
//! material as it is (#GAME-BEHAVIOR-0029). A material no one define brings there is left, and
//! the package refuses it.
//!
//! The id puts it after `retype` in the conform phase, so that the switches and macros it
//! reads are those the target build reads.

use std::collections::BTreeSet;

use ltk_meta::property::values;
use ltk_meta::{PropertyKind as Kind, PropertyValueEnum as V};

use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::game::{AskedVariant, GameBin, SHADERS_BIN, asked_variants, is_switch_of};
use crate::model::material::Material;
use crate::model::value::{as_str, field_of};
use crate::model::{FieldPath, FixupInfo, Phase, Record, StructMut, name_hash, path_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "shader-variant",
    "A material asking for a shader variant the cache lacks, to the nearest it holds",
    &[Record::game_behavior(29)],
    Phase::Conform,
);

const MATERIAL: &str = "StaticMaterialDef";
const SWITCHES: &str = "switches";
const SWITCH: &str = "StaticMaterialSwitchDef";
const NAME: &str = "name";
const ON: &str = "on";
const SHADER_MACROS: &str = "shaderMacros";

/// The one define that moves a material to a compiled variant, and where the material sets it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Move {
    /// A static switch of the shader, in the material's `switches`.
    Switch { name: String, on: bool },
    /// Any other define, in the material's `shaderMacros`.
    Macro { name: String, value: String },
}

/// The fixup `shader-variant`.
pub struct ShaderVariantNearest;

impl ShaderVariantNearest {
    /// The move that makes every variant the material asks for one the cache holds. None where
    /// it holds them all, or where no one define gets there. Where several would, the first by
    /// name and value.
    fn nearest(material: &Material<'_>, shaders: &GameBin, cx: &mut FixCx<'_>) -> Option<Move> {
        let asked = Self::asked(material, shaders, cx)?;
        let lacking: Vec<&AskedVariant> = asked.iter().filter(|asked| !asked.held).collect();
        if lacking.is_empty() {
            return None;
        }
        let mut candidates = BTreeSet::new();
        for asked in &lacking {
            for (name, value) in asked.file.base() {
                candidates.insert((name.to_owned(), value.to_owned()));
            }
        }
        for (name, value) in candidates {
            let as_switch = is_switch_of(material, shaders, &name) && matches!(&*value, "0" | "1");
            let (moved, tried) = if as_switch {
                let on = value == "1";
                (
                    Move::Switch {
                        name: name.clone(),
                        on,
                    },
                    material.with_switch(&name, on),
                )
            } else {
                (
                    Move::Macro {
                        name: name.clone(),
                        value: value.clone(),
                    },
                    material.with_macro(&name, &value),
                )
            };
            let held = Self::asked(&tried, shaders, cx)?;
            if held.iter().all(|asked| asked.held) {
                return Some(moved);
            }
        }
        None
    }

    fn asked(
        material: &Material<'_>,
        shaders: &GameBin,
        cx: &mut FixCx<'_>,
    ) -> Option<Vec<AskedVariant>> {
        match asked_variants(material, shaders, cx.game(), &|_| false) {
            Ok(asked) => Some(asked),
            Err(error) => {
                cx.fault(error);
                None
            }
        }
    }

    /// Sets a switch in the material's `switches`: the switch's `on` where the list holds the
    /// name, else a switch added to the list, made in the kind the target reads.
    fn set_switch(cx: &mut FixCx<'_>, s: &mut StructMut<'_>, name: &str, on: bool) {
        let (field, class) = (name_hash(SWITCHES), name_hash(SWITCH));
        let (name_field, on_field) = (name_hash(NAME), name_hash(ON));
        let at = s.path_of(field);
        let list_kind = cx.type_of(s.class(), field).map(|held| held.kind);
        let fields = s.fields_mut();
        if !fields.contains_key(&field) {
            let empty = values::Container::empty(Kind::Embedded)
                .unwrap_or_else(|error| panic!("a list of embeds is not made: {error}"));
            let list = match list_kind {
                Some(Kind::Container) => V::Container(empty),
                _ => V::UnorderedContainer(values::UnorderedContainer(empty)),
            };
            fields.insert(field, list);
        }
        let list = match fields.get_mut(&field) {
            Some(V::Container(list)) => list,
            Some(V::UnorderedContainer(list)) => &mut list.0,
            _ => return,
        };
        let after = V::Bool(values::Bool::new(on));
        let named = |item: &V| field_of(item, NAME).and_then(as_str) == Some(name);
        if let Some(index) = list.items().iter().position(named) {
            let before = field_of(&list.items()[index], ON).cloned();
            let mut slot = list
                .slot(index)
                .unwrap_or_else(|| panic!("the list of switches has no item {index}"));
            let Some(switch) = switch_of(&mut slot) else {
                return;
            };
            switch.properties.insert(on_field, after.clone());
            let path = at.item(index).field(class, on_field);
            match before {
                Some(before) => cx.converted(path, &before, &after),
                None => cx.added(path, &after),
            }
            return;
        }
        let switch = values::Struct {
            class_hash: class,
            properties: [
                (name_field, V::String(values::String::from(name))),
                (on_field, after),
            ]
            .into_iter()
            .collect(),
        };
        let made = V::Embedded(values::Embedded(switch));
        let index = list.len();
        // A list of another kind than embeds is one the client drops whole, and the fixup that
        // retypes it ran first: nothing is added to what it left.
        if list.push(made.clone()).is_ok() {
            cx.added(at.item(index), &made);
        }
    }

    /// Sets a macro in the material's `shaderMacros`: its value where the map holds the name,
    /// else an entry added.
    fn set_macro(cx: &mut FixCx<'_>, s: &mut StructMut<'_>, name: &str, value: &str) {
        let field = name_hash(SHADER_MACROS);
        let at = s.path_of(field);
        let fields = s.fields_mut();
        if !fields.contains_key(&field) {
            let empty = values::Map::empty(Kind::String, Kind::String)
                .unwrap_or_else(|error| panic!("a map of strings is not made: {error}"));
            fields.insert(field, V::Map(empty));
        }
        let Some(V::Map(map)) = fields.get_mut(&field) else {
            return;
        };
        let key = V::String(values::String::from(name));
        let after = V::String(values::String::from(value));
        let path = key_path(&at, &key);
        let mut entries = map.entries().iter();
        if let Some(index) = entries.position(|(held, _)| as_str(held) == Some(name)) {
            let before = map.entries()[index].1.clone();
            let mut slot = map
                .slot(index)
                .unwrap_or_else(|| panic!("the map of macros has no entry {index}"));
            if slot.set(after.clone()).is_ok() {
                cx.converted(path, &before, &after);
            }
            return;
        }
        if map.push(key, after.clone()).is_ok() {
            cx.added(path, &after);
        }
    }
}

/// The struct a list item is, embedded or pointed to.
fn switch_of<'s>(
    slot: &'s mut ltk_meta::property::ValueSlot<'_>,
) -> Option<&'s mut values::Struct> {
    if slot.get_mut::<values::Embedded>().is_some() {
        return slot.get_mut::<values::Embedded>().map(|embed| &mut embed.0);
    }
    slot.get_mut::<values::Struct>()
}

/// The path of a map's value under a key, or the map's where the key is of no kind a path
/// spells.
fn key_path(at: &FieldPath, key: &V) -> FieldPath {
    ltk_meta::path::MapKey::try_from(key).map_or_else(|_| at.clone(), |key| at.key(key))
}

impl BinFixup for ShaderVariantNearest {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        types.declares(name_hash(MATERIAL), name_hash(SWITCHES))
            || types.declares(name_hash(MATERIAL), name_hash(SHADER_MACROS))
    }

    fn classes(&self) -> Classes {
        Classes::named(&[MATERIAL])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        if !s.is_root() {
            return;
        }
        let Some(shaders) = cx.game_bin(path_hash(SHADERS_BIN)) else {
            return;
        };
        let moved = Self::nearest(&Material::of(s.fields()), &shaders, cx);
        match moved {
            Some(Move::Switch { name, on }) => Self::set_switch(cx, &mut s, &name, on),
            Some(Move::Macro { name, value }) => Self::set_macro(cx, &mut s, &name, &value),
            None => return,
        }
        for class in [MATERIAL, SWITCH] {
            cx.learn_class(class);
        }
        for field in [SWITCHES, NAME, ON, SHADER_MACROS] {
            cx.learn_field(field);
        }
    }
}
