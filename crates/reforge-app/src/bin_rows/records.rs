//! The patch records of a `PTCH` bin, as rows under the object each record targets.
//!
//! A target row is its entry hash and `#`, a record row is `#n`, the record's position in the
//! file's record list, and a row inside a record's value continues with the hash path's
//! segments, each with its separator: `#12.0badf00d`.
//!
//! Copied from LTK Manager's `crates/ltk-manager-bin/src/bin_document/records.rs`, which
//! LTK Manager's ADR-0041 decides. Cut: the schema read for a record's class.

use indexmap::IndexMap;
use ltk_meta::PropertyPatch;
use reforge::model::BinHash;

use super::path::{Step, parse_steps};
use super::rows::{Names, cleaned, hex};
use super::types::{BinRow, BinRows, BinValue, RowNode};

/// The hash path of a target row, which holds the records one object takes.
pub const TARGET_PATH: &str = "#";

/// The hash path of the record at `index` of the file's record list.
fn record_path(index: usize) -> String {
    format!("{TARGET_PATH}{index}")
}

/// A record address in two halves: the record's position, and the hash path segments under
/// its value.
///
/// `#12.0badf00d` is record 12 and `.0badf00d`. `None` for an object's path, for the target
/// path alone, and for a position written with a leading zero.
pub(super) fn record_address(path: &str) -> Option<(usize, &str)> {
    let after = path.strip_prefix(TARGET_PATH)?;
    let end = after
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(after.len());
    let digits = &after[..end];
    if digits.is_empty() || (digits.len() > 1 && digits.starts_with('0')) {
        return None;
    }
    Some((digits.parse().ok()?, &after[end..]))
}

/// The steps under a record's value, or `None` where the text is not one.
///
/// Every segment keeps its separator, so a field opens with `.` even as the first step.
pub(super) fn steps_under(rest: &str) -> Option<Vec<Step>> {
    match rest.as_bytes().first() {
        None => Some(Vec::new()),
        Some(b'.') => {
            let fields = &rest[1..];
            if fields.starts_with(['[', '{']) {
                return None;
            }
            parse_steps(fields).filter(|steps| !steps.is_empty())
        }
        Some(b'[' | b'{') => parse_steps(rest),
        Some(_) => None,
    }
}

/// The position of every record, under the object it targets.
///
/// The targets keep the order of each one's first record, and the positions keep file order.
pub(super) fn targets(records: &[PropertyPatch]) -> IndexMap<BinHash, Vec<usize>> {
    let mut targets: IndexMap<BinHash, Vec<usize>> = IndexMap::new();
    for (index, record) in records.iter().enumerate() {
        targets.entry(record.object_hash).or_default().push(index);
    }
    targets
}

/// The row of one target: the object's name and how many records it takes.
pub(super) fn target_row(target: BinHash, records: usize, names: &Names<'_>) -> BinRow {
    let (name, unnamed) = names.entry(target);
    BinRow {
        entry: hex(target),
        path: TARGET_PATH.to_owned(),
        label: String::new(),
        node: RowNode::Target,
        name,
        unnamed,
        kind: None,
        value: BinValue::Records { len: records },
    }
}

/// The rows of the records at `positions`: `offset` in, at most `limit` of them, and the total.
pub(super) fn records_of(
    entry: BinHash,
    records: &[PropertyPatch],
    positions: &[usize],
    offset: usize,
    limit: usize,
    names: &Names<'_>,
) -> BinRows {
    let window = positions.get(offset..).unwrap_or(&[]).iter().take(limit);
    let entry = hex(entry);
    let rows = window
        .map(|&index| {
            let record = &records[index];
            let path = cleaned(record.path.as_str().to_owned());
            BinRow {
                entry: entry.clone(),
                path: record_path(index),
                label: path.clone(),
                node: RowNode::Record,
                name: path,
                unnamed: false,
                kind: Some(record.value.kind().into()),
                value: names.value_of(&record.value),
            }
        })
        .collect();
    BinRows {
        rows,
        total: positions.len(),
    }
}
