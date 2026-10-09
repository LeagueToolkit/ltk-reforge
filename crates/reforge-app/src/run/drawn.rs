//! What the views draw of a run as far as it got, read from its report as it stands: at the
//! end of the run, and after each stage that gives a part, so the window fills in as the run
//! goes. The report lists the changes fixup by fixup in the order the run made them, which is
//! how a change gets the class, the field and the outcome the report does not hold.

use std::collections::{BTreeMap, HashSet};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use reforge::Run;
use reforge::fixups::{Change, Outcome};
use reforge::model::entity::ObjectId;
use reforge::model::{Spell, clean};
use reforge::package::Thumbnail;
use reforge::report::Report;
use serde::Serialize;

use super::StageName;
use super::views::{
    ChangeView, FixupChanges, FixupRow, ModView, ObjectView, OutcomeView, PackageRow, PackageView,
    PartView, ReportParts, TargetsView,
};
use crate::view::{Finding, read_clean, through_clean};

/// What the views draw of a run as far as it got.
pub(super) struct Drawn {
    pub meta: Option<ModView>,
    pub halted: Option<u32>,
    pub findings: Vec<Finding>,
    pub changes: Vec<FixupChanges>,
    pub objects: Vec<ObjectView>,
    pub targets: Option<TargetsView>,
    pub package: Option<PackageView>,
    pub names: BTreeMap<String, String>,
}

impl Drawn {
    /// The run's report as it stands, read through `clean`.
    pub fn of(run: &Run<'_>) -> Self {
        let mut parts: ReportParts = read_clean(&run.report().to_json());
        // The report names the mod once `load` made it; a run stopped after `read` has the
        // archive's word for it.
        let meta = parts
            .meta
            .take()
            .or_else(|| run.archive().map(|archive| through_clean(&archive.meta)));
        let targets = parts.named_targets();
        let changes = changes_of(run, parts.changes.take().unwrap_or_default());
        let package = parts.package.take().map(|row| package_view(run, row));
        let objects = objects_of(run);
        let mut names = parts.names();
        names.extend(objects.iter().map(|o| (o.id.clone(), o.entry.clone())));
        Self {
            meta,
            halted: parts.halted,
            findings: parts.diagnostics,
            changes,
            objects,
            targets,
            package,
            names,
        }
    }

    /// What a stage that ended gave the views, read from a report of that part alone, as
    /// `Run::report` builds the part. None for a stage that gives no part.
    pub fn part(run: &Run<'_>, stage: StageName) -> Option<PartView> {
        match stage {
            StageName::Fix => {
                let (changes, m) = (run.changes()?, run.module()?);
                let report = Report::builder().changes(changes, m, &run.speller());
                let rows: Option<Vec<FixupRow>> = through_clean(&report.build().changes);
                Some(PartView::Changes {
                    changes: changes_of(run, rows.unwrap_or_default()),
                    objects: objects_of(run),
                })
            }
            StageName::Decide => {
                let report = Report::builder().module(run.module()?);
                let report = report.targets(run.targets()?).build();
                let mut parts = ReportParts {
                    characters: through_clean(&report.characters),
                    targets: through_clean(&report.targets),
                    ..ReportParts::default()
                };
                let targets = parts.named_targets()?;
                Some(PartView::Targets { targets })
            }
            StageName::Build => {
                let report = Report::builder().package_applied(run.package()?, None);
                let row: Option<PackageRow> = through_clean(&report.build().package);
                let package = package_view(run, row?);
                Some(PartView::Package { package })
            }
            _ => None,
        }
    }
}

/// The package as the views show it, with its thumbnail.
fn package_view(run: &Run<'_>, row: PackageRow) -> PackageView {
    let thumbnail = run.package().and_then(|package| package.thumbnail.as_ref());
    PackageView {
        name: row.name,
        version: row.version,
        chunks: row.chunks.len(),
        patches: row.patches.len(),
        thumbnail: thumbnail.map(data_url),
    }
}

/// A thumbnail as a `data:` URL the window shows as an image.
pub(super) fn data_url(thumbnail: &Thumbnail) -> String {
    format!(
        "data:image/webp;base64,{}",
        BASE64.encode(thumbnail.bytes())
    )
}

/// The report's rows of changes as views, each change with the class, the field and the
/// outcome of the run's change at its place.
///
/// # Panics
///
/// When the report and the run list the fixups' changes otherwise: the report is written from
/// the run's changes, fixup by fixup in their order.
fn changes_of(run: &Run<'_>, rows: Vec<FixupRow>) -> Vec<FixupChanges> {
    let Some(changes) = run.changes() else {
        return Vec::new();
    };
    let speller = run.speller();
    let made = changes.by_fixup().map(|(_, made)| made);
    rows.into_iter()
        .zip(made)
        .map(|(row, made)| {
            assert_eq!(
                row.changes.len(),
                made.len(),
                "the report lists {} changes of {} and the run {}",
                row.changes.len(),
                row.fixup,
                made.len()
            );
            let views = row.changes.into_iter().zip(made);
            let views = views.map(|(change_row, change)| ChangeView {
                object: change_row.object,
                entry: change_row.entry,
                field: field_of(&speller, change),
                outcome: match change.outcome() {
                    Outcome::Changed => OutcomeView::Changed,
                    Outcome::Removed => OutcomeView::Removed,
                    Outcome::Lost => OutcomeView::Lost,
                    Outcome::Unconvertible => OutcomeView::Unconvertible,
                },
                reached: change_row.reached,
                path: change_row.path,
                before: change_row.before,
                after: change_row.after,
            });
            FixupChanges {
                fixup: row.fixup,
                changed: row.changed,
                removed: row.removed,
                lost: row.lost,
                unconvertible: row.unconvertible,
                changes: views.collect(),
            }
        })
        .collect()
}

/// The class and the field a change is at, `Class.field`. Empty for the object itself.
fn field_of(speller: &impl Spell, change: &Change) -> String {
    let Some((class, field)) = change.path.last_field() else {
        return String::new();
    };
    let spelled = format!(
        "{}.{}",
        speller.class_or_hex(class),
        speller.field_or_hex(field)
    );
    // A name the run learnt can be the mod's own text.
    clean(&spelled).into_owned()
}

/// An object a fixup changed, as the run spells it.
#[derive(Serialize)]
struct ObjectRow {
    id: ObjectId,
    entry: String,
    class: String,
    bin: String,
}

/// The objects the run's fixups changed, in the order of their first change.
fn objects_of(run: &Run<'_>) -> Vec<ObjectView> {
    let (Some(changes), Some(m)) = (run.changes(), run.module()) else {
        return Vec::new();
    };
    let speller = run.speller();
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for change in changes.iter() {
        if !seen.insert(change.object.index()) {
            continue;
        }
        let node = &m.objects[change.object];
        let file = &m.files[m.bins[node.bin].file];
        let bin = file
            .path
            .as_ref()
            .map_or_else(|| speller.path_or_hex(file.hash), ToString::to_string);
        rows.push(ObjectRow {
            id: change.object,
            entry: speller.entry_or_hex(node.entry),
            class: speller.class_or_hex(node.class),
            bin,
        });
    }
    through_clean(&rows)
}
