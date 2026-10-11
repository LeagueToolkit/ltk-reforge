//! What the window asks of a run apart from the view it ended with (#ADR-0103): hashes spelled
//! with the run's names and the inputs' tables, and the places of the values the type check
//! counts. The report keeps hex; a name is the app's, made when a person reads a finding.

use std::collections::BTreeMap;
use std::sync::Arc;

use reforge::game::{NameBook, Names, Speller};
use reforge::model::{BinHash, Spell, WadHash, clean};
use reforge::snapshot::Snapshots;
use reforge::store::MemoryStore;
use reforge::{RunError, Start};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, chain};
use crate::inputs::Inputs;
use crate::run::{Maker, RunKind, RunSettings, Unwatched};
use crate::store::AppStore;
use crate::view::through_clean;

/// A run the window asks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "of", rename_all = "kebab-case")]
pub enum RunRef {
    /// A run the store keeps, by its number.
    Kept {
        /// Its number.
        id: u64,
    },
    /// The run page's last run.
    Last,
    /// The mod explored.
    Explored,
}

/// The most hashes one ask spells: a power of two well above what a finding's detail shows at
/// once, a page of places with their fields.
pub const MAX_SPELLED: usize = 4096;

/// A name for each hash the run's names or the tables spell, by the hash as asked: eight hex
/// digits are a bin hash, tried as an entry, a field and a class in that order; sixteen are a
/// path hash. A hash none spells, and text that is no hash, are left out. Refused past
/// [`MAX_SPELLED`] hashes.
pub fn spell(
    hashes: &[String],
    book: &NameBook,
    names: Option<&Names>,
) -> Result<BTreeMap<String, String>, AppError> {
    if hashes.len() > MAX_SPELLED {
        return Err(AppError::Internal {
            detail: format!("{} hashes asked at once, past {MAX_SPELLED}", hashes.len()),
        });
    }
    let none = Names::default();
    let speller = Speller::new(book, names.unwrap_or(&none));
    let spelled = hashes.iter().filter_map(|hash| {
        let name = spell_one(&speller, hash)?;
        // A name the run learnt can be the mod's own text.
        Some((hash.clone(), clean(&name).into_owned()))
    });
    Ok(spelled.collect())
}

fn spell_one(speller: &Speller<'_>, hash: &str) -> Option<String> {
    if !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match hash.len() {
        8 => {
            let hash = BinHash(u32::from_str_radix(hash, 16).ok()?);
            (speller.entry(hash))
                .or_else(|| speller.field(hash))
                .or_else(|| speller.class(hash))
        }
        16 => speller.path(WadHash(u64::from_str_radix(hash, 16).ok()?)),
        _ => None,
    }
}

/// Where one value a type check counts is, every string cleaned: the WAD, the bin's path, the
/// object's entry as eight hex digits, and the value's path from the object's root in the hash
/// form (`.` between fields, `[i]` an index, `{key}` a map entry, a field as eight hex digits).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ValuePlaceView {
    /// The WAD's name.
    pub wad: String,
    /// The bin's path.
    pub path: String,
    /// The object's entry hash.
    pub entry: String,
    /// The value's path in the object.
    pub at: String,
}

/// The places of the values one row of a type check's finding counts, in the order found, up
/// to the library's `MAX_ROW_PLACES`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct RowPlaces {
    /// The places.
    pub places: Vec<ValuePlaceView>,
}

/// The places of every row of the type check's three findings, each list in its finding's
/// order of rows (#ADR-0103).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct TypePlacesView {
    /// The rows of `check.type-mismatch`.
    pub mismatched: Vec<RowPlaces>,
    /// The rows of `check.field-undeclared`.
    pub undeclared: Vec<RowPlaces>,
    /// The rows of `check.class-undeclared`.
    pub classes: Vec<RowPlaces>,
}

/// A run that holds its package, to read its places again: its kind and settings, its
/// snapshots through `build`, and the bytes it made that the store does not hold.
pub struct Held<'a> {
    /// A repair or a port.
    pub kind: RunKind,
    /// The settings it ran with.
    pub settings: &'a RunSettings,
    /// Its snapshots from `load` on.
    pub snapshots: Snapshots,
    /// The bytes of a run held in memory, over the store's tiers. None for a kept run, whose
    /// bytes are in the run tier.
    pub layer: Option<Arc<MemoryStore>>,
}

/// The places of the values the type check counts in a run's package: the run resumed after
/// `build` from its snapshots against the inputs, and the check's walk run again
/// (`Run::type_places`). Refused for a run that did not end `build`, and for one the inputs
/// or this program no longer resume.
pub fn type_places(
    inputs: &Inputs,
    store: Option<&AppStore>,
    held: Held<'_>,
) -> Result<TypePlacesView, AppError> {
    if held.snapshots.build.is_none() {
        return Err(AppError::NoPackageHeld);
    }
    let maker = Maker::of(inputs, held.kind, held.settings)?;
    let run = match held.layer {
        Some(layer) => maker.start_over(inputs, store, layer),
        None => maker.start(inputs, store, &mut Unwatched),
    };
    let start = Start::Build(Box::new(held.snapshots));
    let mut run = run.resume(start).map_err(|error| AppError::NotResumed {
        detail: chain(&error),
    })?;
    let places = run
        .type_places()
        .map_err(|error: RunError| AppError::Internal {
            detail: chain(&error),
        })?;
    Ok(through_clean(&places))
}
