//! A contextual rule's gear condition, as the class the target declares (#GAME-CHANGE-0027).
//! The class and one field's name change, and every value and the condition's place stay.

use ltk_meta::PropertyValueEnum as V;
use ltk_meta::property::values;

use super::Stated;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::{BinHash, FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "contextual-gear-equip",
    "A contextual rule's gear condition, as a ContextualConditionGearIdEquip",
    &[Record::game_change(27)],
    Phase::Layout {
        build: Version::new(15, 18),
    },
);

const RULE: &str = "ContextualRule";
const CONDITIONS: &str = "mConditions";
const OLD: &str = "ContextualConditionGearId";
const NEW: &str = "ContextualConditionGearIdEquip";
/// The old class's bool, which no name table spells: `CHECK_NEW` in the new class.
const CHECK: BinHash = BinHash(0x58ec_fcaa);
const CHECK_NEW: &str = "CheckNewGearId";

/// #GAME-CHANGE-0027: the number of 15.18.7095052, the first build without the old class.
const NEW_AT: u64 = 7_095_052;

const STATED: Stated = Stated {
    classes: &[OLD, NEW],
    fields: &[CONDITIONS, CHECK_NEW],
};

/// The fixup of a contextual rule's conditions of the old class.
pub struct GearEquip;

impl GearEquip {
    /// Whether the mod's own build read the old class. True for a mod with no date.
    fn read_at_source(cx: &FixCx<'_>) -> bool {
        cx.source_build().is_none_or(|build| build < NEW_AT)
    }

    /// The condition as the new class, its bool under the new name in the same place.
    fn equip(condition: &mut values::Struct) {
        condition.class_hash = name_hash(NEW);
        if let Some((index, _, value)) = condition.properties.shift_remove_full(&CHECK) {
            condition
                .properties
                .shift_insert(index, name_hash(CHECK_NEW), value);
        }
    }
}

impl BinFixup for GearEquip {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        types.declares(name_hash(NEW), name_hash(CHECK_NEW))
    }

    fn classes(&self) -> Classes {
        Classes::named(&[RULE])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let (conditions, old) = (name_hash(CONDITIONS), name_hash(OLD));
        let at = s.path_of(conditions);
        let Some(V::Container(list)) = s.fields_mut().get_mut(&conditions) else {
            return;
        };
        let is_old =
            |value: &V| matches!(value, V::Struct(condition) if condition.class_hash == old);
        if !list.items().iter().any(is_old) {
            return;
        }
        if Self::read_at_source(cx) {
            for i in 0..list.len() {
                let before = &list.items()[i];
                if !is_old(before) {
                    continue;
                }
                let before = before.clone();
                let mut slot = list
                    .slot(i)
                    .unwrap_or_else(|| panic!("the list of conditions has no item {i}"));
                let Some(condition) = slot.get_mut::<values::Struct>() else {
                    continue;
                };
                Self::equip(condition);
                let after = V::Struct(condition.clone());
                cx.converted(at.item(i), &before, &after);
            }
        } else {
            // #ADR-0065: no build since the mod's own read the condition.
            let item_kind = list.item_kind();
            let empty = values::Container::empty(item_kind)
                .unwrap_or_else(|error| panic!("a list of {item_kind:?} is not made: {error}"));
            let items = std::mem::replace(list, empty).into_items();
            let mut kept = Vec::with_capacity(items.len());
            for (i, item) in items.into_iter().enumerate() {
                if is_old(&item) {
                    cx.removed(at.item(i), &item);
                } else {
                    kept.push(item);
                }
            }
            *list = values::Container::new(item_kind, kept).unwrap_or_else(|error| {
                panic!("the conditions kept make no list of {item_kind:?}: {error}")
            });
        }
        STATED.teach(cx);
    }
}
