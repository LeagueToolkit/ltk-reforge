//! Two kept runs compared (#ADR-0091): what differs in their settings and inputs, their
//! findings, a port's targets, the fixups' counts and their packages. A diff of what the two
//! runs answered, which adds no rule about a mod (#ADR-0046).

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8Path;
use reforge::load::KeptFile;
use reforge::model::clean;
use reforge::snapshot::{LoadSnapshot, Program};
use rusqlite::OptionalExtension;
use serde::Serialize;

use super::files::read_snapshot;
use super::{AppStore, failed, json_of, kind_text};
use crate::error::AppError;
use crate::run::{FixupChanges, KeptFileView, PackageView, RunKind, RunSettings, RunView};
use crate::store::RunRow;
use crate::view::Finding;

/// One run of a comparison, as its header shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct RunHead {
    /// Its number.
    pub id: u64,
    /// When the store wrote it, by the store's clock.
    pub made_at: String,
    /// A repair or a port.
    pub kind: RunKind,
    /// The mod's name, as its archive says.
    pub name: Option<String>,
    /// The mod's version, as its archive says.
    pub version: Option<String>,
    /// The archive's file name.
    pub file_name: String,
    /// The file name of the `.modpkg` written.
    pub package: Option<String>,
}

/// One value that differs between the two runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Diff {
    /// What differs, as a stable id the window translates. A setting: `kind`, `target`,
    /// `anim_reuse`, `keep_stale`, `keep_dds`, `game_events_only`, `block`, `checks_off`,
    /// `fixups_off`, `fixups_on`, `rules`, `unit_targets`, `kept_files`, `keep_other_slots`.
    /// An input: `program`, `install_build`, `history_head`, `names`, `schema_build`.
    pub key: String,
    /// The value in the first run, as text: a list joined with `, `, a switch `true` or
    /// `false`, none empty.
    pub a: String,
    /// The value in the second run.
    pub b: String,
}

/// The findings of the two runs, matched by code, subject and message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct FindingsDiff {
    /// The second run's findings the first has not, in the second's order.
    pub added: Vec<Finding>,
    /// The first run's findings the second has not, in the first's order.
    pub gone: Vec<Finding>,
    /// How many are in both.
    pub same: usize,
}

/// One unit's target in either run of a port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct TargetDiff {
    /// The unit, as its character and slot name it: `Ahri skin3`.
    pub unit: String,
    /// The character's name.
    pub character_name: String,
    /// The first run's target. None: the unit left to the game, or absent from the run.
    pub a: Option<u16>,
    /// The second run's target.
    pub b: Option<u16>,
    /// Why the first run chose it, as the library names it. Empty for a unit it lacks.
    pub a_reason: String,
    /// Why the second run chose it.
    pub b_reason: String,
    /// Whether the target or the reason differs.
    pub differs: bool,
}

/// How many values a fixup changed in one run, by outcome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct Counts {
    /// Values converted to the form the target build reads.
    pub changed: usize,
    /// Values removed with nothing lost.
    pub removed: usize,
    /// Values removed that the mod's own build read.
    pub lost: usize,
    /// Values with no conversion, left as they are.
    pub unconvertible: usize,
}

impl Counts {
    fn of(changes: &FixupChanges) -> Self {
        Self {
            changed: changes.changed,
            removed: changes.removed,
            lost: changes.lost,
            unconvertible: changes.unconvertible,
        }
    }
}

/// What one fixup did in either run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct ChangeDiff {
    /// The fixup's id.
    pub fixup: String,
    /// Its counts in the first run, zero when it changed nothing there.
    pub a: Counts,
    /// Its counts in the second run.
    pub b: Counts,
    /// Whether the counts differ.
    pub differs: bool,
}

/// The packages the two runs built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct PackageDiff {
    /// The first run's.
    pub a: Option<PackageView>,
    /// The second run's.
    pub b: Option<PackageView>,
}

/// Two kept runs compared: the first, `a`, against the second, `b`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct CompareView {
    /// The first run.
    pub a: RunHead,
    /// The second run.
    pub b: RunHead,
    /// Whether both runs are of the same archive, by its SHA-256.
    pub same_mod: bool,
    /// The request's settings that differ.
    pub settings: Vec<Diff>,
    /// What the runs were judged under that differs: the program and the inputs.
    pub inputs: Vec<Diff>,
    /// The findings.
    pub findings: FindingsDiff,
    /// Every unit of either port, the first run's first. Empty unless both runs are ports.
    pub targets: Vec<TargetDiff>,
    /// Every fixup that changed something in either run, the first run's first.
    pub changes: Vec<ChangeDiff>,
    /// The packages.
    pub package: PackageDiff,
}

/// What a comparison reads of one kept run.
struct Side {
    row: RunRow,
    settings: RunSettings,
    program: Program,
    schema_build: Option<String>,
    /// None for a run opened from a package.
    load: Option<LoadSnapshot>,
    view: RunView,
}

impl AppStore {
    /// Two kept runs compared. None when either number names no run.
    pub fn compare(&self, a: u64, b: u64) -> Result<Option<CompareView>, AppError> {
        let (Some(a), Some(b)) = (self.side(a)?, self.side(b)?) else {
            return Ok(None);
        };
        let settings_a = settings_rows(a.row.kind, &a.settings);
        let settings_b = settings_rows(b.row.kind, &b.settings);
        let inputs_a = input_rows(&a);
        let inputs_b = input_rows(&b);
        Ok(Some(CompareView {
            same_mod: a.row.sha256 == b.row.sha256,
            settings: differing(settings_a, settings_b),
            inputs: differing(inputs_a, inputs_b),
            findings: findings(&a.view.findings, &b.view.findings),
            targets: match (a.row.kind, b.row.kind) {
                (RunKind::Port, RunKind::Port) => targets(&a.view, &b.view),
                // A repair decides nothing to set a port's targets against.
                _ => Vec::new(),
            },
            changes: changes(&a.view.changes, &b.view.changes),
            package: PackageDiff {
                a: a.view.package.clone(),
                b: b.view.package.clone(),
            },
            a: head(a.row),
            b: head(b.row),
        }))
    }

    fn side(&self, id: u64) -> Result<Option<Side>, AppError> {
        let (Some(row), Ok(number)) = (self.row(id)?, i64::try_from(id)) else {
            return Ok(None);
        };
        let query = "SELECT run.settings, run.schema_build, run.program_version,
               run.program_commit, load.json
             FROM run LEFT JOIN load ON load.id = run.load_id WHERE run.id = ?1";
        type Columns = (
            String,
            Option<String>,
            String,
            Option<String>,
            Option<Vec<u8>>,
        );
        let columns: Option<Columns> = {
            let rows = self.rows.lock();
            let found = rows.query_row(query, [number], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            });
            found.optional()?
        };
        let (Some(columns), Some(view)) = (columns, self.run_view(id)?) else {
            return Ok(None);
        };
        let (settings, schema_build, version, commit, load) = columns;
        Ok(Some(Side {
            row,
            settings: serde_json::from_str(&settings).map_err(failed)?,
            program: Program { version, commit },
            schema_build,
            load: load.as_deref().map(read_snapshot).transpose()?,
            view,
        }))
    }
}

fn head(row: RunRow) -> RunHead {
    let package = row.package.as_deref().map(|path| {
        let path = Utf8Path::new(path);
        path.file_name().unwrap_or(path.as_str()).to_string()
    });
    RunHead {
        id: row.id,
        made_at: row.made_at,
        kind: row.kind,
        name: row.name,
        version: row.version,
        file_name: row.file_name,
        package,
    }
}

/// A run's settings as text, by key. The settings' text is the person's own input, shown as
/// given.
fn settings_rows(kind: RunKind, settings: &RunSettings) -> Vec<(&'static str, String)> {
    let list = |items: &[String]| items.join(", ");
    let rules = settings.fixups.rules.iter().map(json_of);
    let unit_targets = settings.unit_targets.iter();
    let unit_targets = unit_targets.map(|given| given.unit_target().to_string());
    let kept_files = settings.kept_files.iter().map(kept_file);
    vec![
        ("kind", kind_text(kind).to_string()),
        ("target", settings.target.clone()),
        ("anim_reuse", settings.anim_reuse.clone()),
        ("keep_stale", settings.keep_stale.to_string()),
        ("keep_dds", settings.keep_dds.to_string()),
        ("game_events_only", settings.game_events_only.to_string()),
        ("block", list(&settings.block)),
        ("checks_off", list(&settings.checks_off)),
        ("fixups_off", list(&settings.fixups.off)),
        ("fixups_on", list(&settings.fixups.on)),
        ("rules", rules.collect::<Vec<_>>().join(", ")),
        ("unit_targets", sorted(unit_targets)),
        ("kept_files", sorted(kept_files)),
        ("keep_other_slots", settings.keep_other_slots.to_string()),
    ]
}

/// The items as text, sorted and joined: the order they were given in changes nothing.
fn sorted(items: impl Iterator<Item = String>) -> String {
    let mut items: Vec<String> = items.collect();
    items.sort();
    items.join(", ")
}

/// A kept file as `<wad>/<path>`, in the library's spelling when it takes it, so two runs that
/// name one file in two spellings do not differ.
fn kept_file(kept: &KeptFileView) -> String {
    match KeptFile::new(&kept.wad, &kept.path) {
        Ok(file) => file.to_string(),
        Err(_) => format!("{}/{}", kept.wad, kept.path),
    }
}

/// What a run was judged under, as text, by key: the program that made it, the inputs its load
/// snapshot was made under (#ADR-0094), none for a run with no load, and the schema's build.
fn input_rows(side: &Side) -> Vec<(&'static str, String)> {
    let program = &side.program;
    let program = match &program.commit {
        Some(commit) => format!("{} ({commit})", program.version),
        None => program.version.clone(),
    };
    let inputs = side.load.as_ref().map(LoadSnapshot::inputs);
    let build = inputs
        .and_then(|i| i.build.as_ref())
        .map(ToString::to_string);
    let history = inputs.and_then(|i| i.history.as_ref());
    let history = history.map(|h| format!("{} ({})", h.newest_build, h.head));
    let names = inputs.map(|inputs| json_of(&inputs.names));
    vec![
        ("program", program),
        ("install_build", build.unwrap_or_default()),
        ("history_head", history.unwrap_or_default()),
        ("names", names.unwrap_or_default()),
        (
            "schema_build",
            side.schema_build.clone().unwrap_or_default(),
        ),
    ]
}

/// The rows whose values differ, in the order of the keys, their text cleaned: a project
/// imported brings another person's settings and program (#ADR-0096).
fn differing(a: Vec<(&'static str, String)>, b: Vec<(&'static str, String)>) -> Vec<Diff> {
    let pairs = a.into_iter().zip(b);
    let differ = pairs.filter(|((_, a), (_, b))| a != b);
    differ
        .map(|((key, a), (_, b))| Diff {
            key: key.to_string(),
            a: clean(&a).into_owned(),
            b: clean(&b).into_owned(),
        })
        .collect()
}

/// What matches a finding across two runs: its code, subject and message. Its id is its place
/// in its own run's log, which another setting shifts.
fn finding_key(finding: &Finding) -> (&str, &str, &str, &str) {
    let subject = &finding.subject;
    (
        &finding.code,
        &subject.kind,
        &subject.name,
        &finding.message,
    )
}

/// The findings of `a` and `b` matched one to one by [`finding_key`].
fn findings(a: &[Finding], b: &[Finding]) -> FindingsDiff {
    let mut left: BTreeMap<_, usize> = BTreeMap::new();
    for finding in a {
        *left.entry(finding_key(finding)).or_default() += 1;
    }
    let mut added = Vec::new();
    let mut same = 0;
    let mut matched: BTreeMap<_, usize> = BTreeMap::new();
    for finding in b {
        let key = finding_key(finding);
        match left.get_mut(&key) {
            Some(count) if *count > 0 => {
                *count -= 1;
                *matched.entry(key).or_default() += 1;
                same += 1;
            }
            _ => added.push(finding.clone()),
        }
    }
    let mut gone = Vec::new();
    for finding in a {
        match matched.get_mut(&finding_key(finding)) {
            Some(count) if *count > 0 => *count -= 1,
            _ => gone.push(finding.clone()),
        }
    }
    FindingsDiff { added, gone, same }
}

/// One unit's decision in one run.
struct Decided<'a> {
    /// The unit, as its character and slot name it.
    unit: &'a str,
    character: &'a str,
    chosen: Option<u16>,
    reason: &'a str,
}

/// A port's decisions, each with its unit's name. A unit's id is its place in its own run's
/// table, so the name is what matches it across runs.
fn decisions(view: &RunView) -> Vec<Decided<'_>> {
    let Some(targets) = &view.targets else {
        return Vec::new();
    };
    let decided = targets.decisions.iter();
    decided
        .map(|decision| Decided {
            unit: view.names.get(&decision.unit).unwrap_or(&decision.unit),
            character: &decision.character_name,
            chosen: decision.chosen,
            reason: &decision.reason,
        })
        .collect()
}

/// Every unit of either run's decisions, `a`'s order first, then `b`'s units `a` lacks.
fn targets(a: &RunView, b: &RunView) -> Vec<TargetDiff> {
    let (a, b) = (decisions(a), decisions(b));
    let of = |side: &[Decided<'_>], unit: &str| {
        let found = side.iter().find(|decided| decided.unit == unit);
        found.map_or((None, String::new()), |d| (d.chosen, d.reason.to_string()))
    };
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for decided in a.iter().chain(&b) {
        if !seen.insert(decided.unit) {
            continue;
        }
        let (a_chosen, a_reason) = of(&a, decided.unit);
        let (b_chosen, b_reason) = of(&b, decided.unit);
        rows.push(TargetDiff {
            unit: decided.unit.to_string(),
            character_name: decided.character.to_string(),
            differs: a_chosen != b_chosen || a_reason != b_reason,
            a: a_chosen,
            b: b_chosen,
            a_reason,
            b_reason,
        });
    }
    rows
}

/// Every fixup that changed something in either run, `a`'s order first, then `b`'s new ones.
fn changes(a: &[FixupChanges], b: &[FixupChanges]) -> Vec<ChangeDiff> {
    let made = |side: &[FixupChanges], fixup: &str| {
        let found = side.iter().find(|changes| changes.fixup == fixup);
        found.map(Counts::of).unwrap_or_default()
    };
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    for changes in a.iter().chain(b) {
        if !seen.insert(changes.fixup.as_str()) {
            continue;
        }
        let (counts_a, counts_b) = (made(a, &changes.fixup), made(b, &changes.fixup));
        if counts_a == Counts::default() && counts_b == Counts::default() {
            continue;
        }
        rows.push(ChangeDiff {
            fixup: changes.fixup.clone(),
            a: counts_a,
            b: counts_b,
            differs: counts_a != counts_b,
        });
    }
    rows
}
