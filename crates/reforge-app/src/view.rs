//! What the front end shows of a run, read from the report. Every string of the report passes
//! through `clean` on its way into a view, so a mod's text reaches the screen only through it
//! (`docs/design/ROADMAP-UX.md`, "Two views of one run"). The JSON `Report::to_json` writes is
//! not enough alone: its escapes read back as the characters they stand for. The types here are
//! the parts a view draws; the advanced view shows the whole report as its JSON.

use reforge::diag::{Diagnostic, Render};
use reforge::model::clean;
use reforge::report::DiagnosticRow;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How much a finding costs the output, as the report names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Nothing is lost.
    Note,
    /// The output may differ from what the author saw, or an input limits the run.
    Warning,
    /// Something of the mod is not in the output, or is wrong in it.
    Error,
    /// There is no output.
    Fatal,
}

/// What a finding is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Subject {
    /// The kind of thing: `wad`, `file`, `object`, `input`.
    pub kind: String,
    /// The id the report's tables join on.
    pub id: String,
    /// The name for a person.
    pub name: String,
}

/// One finding of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Finding {
    /// Its place in the run's log.
    pub id: u32,
    /// Its code: `docs/design/CODES.md` lists every one.
    pub code: String,
    /// What the output loses.
    pub severity: Severity,
    /// What it speaks to: `run`, `input`, `decision`, `output`.
    pub channel: String,
    /// The stage that raised it.
    pub stage: String,
    /// What it is about.
    pub subject: Subject,
    /// The fixup or check that raised it.
    #[serde(default)]
    pub by: Option<String>,
    /// The record it rests on, by its code.
    #[serde(default)]
    pub record: Option<String>,
    /// One line for a person.
    pub message: String,
    /// The finding's fields, as `docs/design/CODES.md` names them for its code. A hash in
    /// them is written as hex, unspelled.
    #[serde(default)]
    #[specta(type = specta_typescript::Unknown)]
    pub data: Value,
}

impl Finding {
    /// A diagnostic as the report writes it, its text cleaned.
    pub fn of(diagnostic: &Diagnostic, cx: &Render<'_>) -> Self {
        through_clean(&DiagnosticRow::of(diagnostic, cx))
    }
}

/// A value of the report's as a view, every string in it cleaned.
///
/// # Panics
///
/// When the view does not read what the report wrote: a view type out of step with the
/// report's schema, which the crate's tests hold.
pub fn through_clean<T: Serialize, V: DeserializeOwned>(value: &T) -> V {
    let value = serde_json::to_value(value)
        .unwrap_or_else(|error| panic!("a report value does not write: {error}"));
    from_value(value)
}

/// A view read from the JSON of a report, every string in it cleaned.
///
/// # Panics
///
/// As [`through_clean`], and when the text is not JSON.
pub fn read_clean<V: DeserializeOwned>(json: &str) -> V {
    let value =
        serde_json::from_str(json).unwrap_or_else(|error| panic!("a report is not JSON: {error}"));
    from_value(value)
}

fn from_value<V: DeserializeOwned>(mut value: Value) -> V {
    clean_strings(&mut value);
    serde_json::from_value(value).unwrap_or_else(|error| {
        panic!(
            "a view of {} does not read the report: {error}",
            std::any::type_name::<V>()
        )
    })
}

/// Every string of a JSON value, keys included, cleaned in place.
pub fn clean_strings(value: &mut Value) {
    match value {
        Value::String(text) => {
            if let std::borrow::Cow::Owned(cleaned) = clean(text) {
                *text = cleaned;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(clean_strings),
        Value::Object(fields) => {
            let taken = std::mem::take(fields);
            for (key, mut field) in taken {
                clean_strings(&mut field);
                fields.insert(clean(&key).into_owned(), field);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}
