//! Retype: every value in the type the target build reads (#GAME-BEHAVIOR-0002). A value of
//! another type is converted by the catalogue, removed where it never showed, or left and
//! reported where no conversion takes it.
//!
//! A value never showed when no build from the mod's own to the target's read its kind. It is
//! removed and not converted: converting it would show what the author never saw. The
//! conversions the mod manager makes at install are the exception (#MOD-ARTIFACT-0022): the
//! author may have seen the value converted so, and it is converted.

use ltk_meta::{PropertyKind as Kind, PropertyValueEnum as V};

use super::conversion::{Conversion, PathNames, conform};
use super::never_shown;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes, TypeSource};
use crate::model::{BinHash, FixupInfo, Phase, Record, StructMut};

static INFO: FixupInfo = FixupInfo::built_in(
    "retype",
    "Values in the type the target build reads",
    &[
        Record::game_behavior(2),
        Record::game_change(13),
        Record::game_change(19),
        Record::game_change(20),
        Record::game_change(21),
        Record::mod_artifact(21),
        Record::mod_artifact(22),
    ],
    Phase::Conform,
);

/// What becomes of a value that is not what the build reads.
enum Verdict {
    /// The value in the type the build reads.
    Converted(V),
    /// The value never showed: it is removed.
    Dead,
    /// No conversion takes the value to this kind: it stays.
    NoRoad(Kind),
}

/// The fixup that brings every value to the type the target build reads.
pub struct Retype;

impl Retype {
    /// What becomes of the value of a field of a struct of a class. None when the value is
    /// what the build reads, or the build's type of the field is not known.
    fn judge(
        cx: &FixCx<'_>,
        (class, field): (BinHash, BinHash),
        value: &V,
        names: &mut PathNames<'_>,
    ) -> Option<Verdict> {
        let (want, source) = cx.type_and_source(class, field)?;
        let from = value.kind();
        // A type the install's bins witness has no history to judge by.
        let dead = from != want.kind
            && source == TypeSource::Schema
            && never_shown(cx, class, field, from);
        let road = Conversion::between(from, want.kind);
        if dead && !road.is_some_and(|road| road.at_install(from)) {
            return Some(Verdict::Dead);
        }
        match conform(value, &want, names) {
            Ok(converted) => converted.map(Verdict::Converted),
            // A list whose items have no conversion stays, under either list's tag, as in
            // ltk-forge (`docs/design/ROADMAP.md`, "Open"). Only a value with no conversion of
            // its own kind is removed.
            Err(wants) if dead && wants == want.kind => Some(Verdict::Dead),
            Err(wants) => Some(Verdict::NoRoad(wants)),
        }
    }
}

impl BinFixup for Retype {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        types.schema().is_some() || types.witnessed_len() > 0
    }

    fn classes(&self) -> Classes {
        Classes::Every
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let class = s.class();
        let mut names = PathNames::of(cx);
        let verdicts: Vec<(BinHash, Verdict)> = s
            .fields()
            .iter()
            .filter_map(|(field, value)| {
                let verdict = Self::judge(cx, (class, *field), value, &mut names)?;
                Some((*field, verdict))
            })
            .collect();
        let (hashed, fault) = names.finish();
        for path in hashed {
            cx.learn_path(&path);
        }
        if let Some(fault) = fault {
            cx.fault(fault);
            return;
        }
        for (field, verdict) in verdicts {
            let path = s.path_of(field);
            match verdict {
                Verdict::Converted(after) => {
                    if let Some(held) = s.fields_mut().get_mut(&field) {
                        let before = std::mem::replace(held, after);
                        cx.converted(path, &before, held);
                    }
                }
                Verdict::Dead => {
                    if let Some(value) = s.fields_mut().shift_remove(&field) {
                        cx.removed(path, &value);
                    }
                }
                Verdict::NoRoad(wants) => {
                    if let Some(value) = s.fields().get(&field) {
                        cx.unconvertible(path, value, wants);
                    }
                }
            }
        }
    }
}
