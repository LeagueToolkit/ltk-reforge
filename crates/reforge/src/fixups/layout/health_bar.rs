//! The health bar style of a champion's skin entry (#GAME-CHANGE-0023): the champion's bar of
//! the target's numbering, in place of the champion's bar of an older numbering, or of none.
//!
//! A champion's own entry's value names its numbering, so it is converted whatever the mod's
//! date (#ADR-0065). A sub-character's value is left (#ADR-0067).

use ltk_meta::PropertyValueEnum as V;
use ltk_meta::property::values;

use super::Stated;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::model::entity::CharKind;
use crate::model::{FixupInfo, Phase, Record, StructMut, Version, name_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "health-bar-style",
    "A champion's skin's health bar style, as the target numbers the champion's bar",
    &[Record::game_change(23)],
    Phase::Layout {
        build: Version::new(13, 4),
    },
);

const ENTRY: &str = "SkinCharacterDataProperties";
const HEALTH_BAR: &str = "healthBarData";
const RECORD: &str = "CharacterHealthBarDataRecord";
const STYLE: &str = "unitHealthBarStyle";

const STATED: Stated = Stated {
    classes: &[ENTRY, RECORD],
    fields: &[HEALTH_BAR, STYLE],
};

/// The champion's bar of each numbering, with the build that brought the numbering in. Before
/// the first, a champion's skin entry holds no style, which reads as the default `0`.
const CHAMPION_BAR: [(Version, u8); 4] = [
    (Version::new(13, 4), 9),
    (Version::new(14, 7), 10),
    (Version::new(14, 17), 11),
    (Version::new(15, 4), 12),
];

/// The numbering in force at a version: how many of [`CHAMPION_BAR`] it has, 0 before 13.4.
fn numbering_at(version: Version) -> usize {
    let started = |(from, _): &&(Version, u8)| *from <= version;
    CHAMPION_BAR.iter().take_while(started).count()
}

/// The numbering whose champion's bar a style is. None for a value no numbering gives the
/// champion's bar.
fn numbering_of(style: Option<u8>) -> Option<usize> {
    match style {
        None | Some(0) => Some(0),
        Some(value) => CHAMPION_BAR
            .iter()
            .position(|&(_, bar)| bar == value)
            .map(|at| at + 1),
    }
}

/// The fixup of the health bar style of a champion's skin entry.
pub struct HealthBarStyle;

impl BinFixup for HealthBarStyle {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        let numbered = types.version().is_some_and(|v| numbering_at(v) > 0);
        numbered
            && types.declares(name_hash(ENTRY), name_hash(HEALTH_BAR))
            && types.declares(name_hash(RECORD), name_hash(STYLE))
    }

    fn classes(&self) -> Classes {
        Classes::named(&[ENTRY])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let Some(target) = cx.types().version().map(numbering_at) else {
            return;
        };
        let champion = cx.entry_of().map(|c| &c.kind);
        if target == 0 || !matches!(champion, Some(CharKind::Champion)) {
            return;
        }
        let (holder, style) = (name_hash(HEALTH_BAR), name_hash(STYLE));
        let bar = V::U8(values::U8::new(CHAMPION_BAR[target - 1].1));
        let at = s.path_of(holder);
        match s.fields_mut().get_mut(&holder) {
            // #GAME-CHANGE-0023: Riot gave such an entry a record holding the style alone.
            None => {
                let record = values::Struct {
                    class_hash: name_hash(RECORD),
                    properties: [(style, bar)].into_iter().collect(),
                };
                let made = V::Embedded(values::Embedded(record));
                cx.added(at, &made);
                s.fields_mut().insert(holder, made);
            }
            Some(V::Embedded(values::Embedded(record))) => {
                let held = match record.properties.get(&style) {
                    None => None,
                    Some(V::U8(held)) => Some(held.value),
                    Some(_) => return,
                };
                if numbering_of(held).is_none_or(|numbering| numbering >= target) {
                    return;
                }
                let path = at.field(record.class_hash, style);
                match record.properties.insert(style, bar.clone()) {
                    Some(before) => cx.converted(path, &before, &bar),
                    None => cx.added(path, &bar),
                }
            }
            Some(_) => return,
        }
        STATED.teach(cx);
    }
}
