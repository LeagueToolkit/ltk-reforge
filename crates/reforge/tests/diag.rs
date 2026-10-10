//! The diagnostics log: raising, the order, filters, levels and the serialised form.

use std::sync::{Arc, Mutex};

use reforge::diag::{
    Action, Channel, Code, Diag, DiagnosticPolicy, Diagnostics, Filter, FilterError, Halted, Input,
    Message, Record, Render, Severity, Stage, Subject,
};
use reforge::model::{Spelled, WadHash};
use serde::Serialize;

#[derive(Serialize)]
struct EntryCorrupt {
    entry: Spelled,
}

impl Diag for EntryCorrupt {
    const CODE: Code = Code::new("archive.entry-corrupt", Channel::Input, Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Entry(self.entry.clone())
    }

    fn message(&self, f: &mut Message) {
        f.text("the entry fails its checksum; skipped");
    }
}

#[derive(Serialize)]
struct Changed {
    fields: Vec<u64>,
}

impl Diag for Changed {
    const CODE: Code = Code::new("fixup.changed", Channel::Output, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message) {
        f.count(self.fields.len(), "value")
            .text(" brought to what the build reads: ");
        f.list(&self.fields, |f, hash| {
            f.path(WadHash(*hash));
        });
    }
}

#[derive(Serialize)]
struct NotZip;

impl Diag for NotZip {
    const CODE: Code = Code::new("archive.not-zip", Channel::Input, Severity::Fatal);

    fn subject(&self) -> Subject {
        Subject::Run
    }

    fn message(&self, f: &mut Message) {
        f.text("the archive is not a zip");
    }
}

#[derive(Serialize)]
struct IndexStale;

impl Diag for IndexStale {
    const CODE: Code = Code::new("history.index-stale", Channel::Run, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Input(Input::History)
    }

    fn message(&self, f: &mut Message) {
        f.text("the index is of another commit than the history's head");
    }
}

fn sample() -> Diagnostics {
    let mut log = Diagnostics::new();
    log.at(Stage::Read).raise(IndexStale);
    log.at(Stage::Read).raise(EntryCorrupt {
        entry: Spelled::new("WAD/Lux.wad.client/a\u{1b}.dds"),
    });
    let mut fix = log.at(Stage::Fix);
    fix.by("vfx-filtering", Some(Record::game_change(1)))
        .raise(Changed {
            fields: (1..=10).collect(),
        });
    log
}

#[test]
fn diagnostics_keep_the_order_they_were_raised_in() {
    let log = sample();
    let codes: Vec<&str> = log.iter().map(|d| d.code.name).collect();
    assert_eq!(
        codes,
        [
            "history.index-stale",
            "archive.entry-corrupt",
            "fixup.changed"
        ]
    );
    let ids: Vec<u32> = log.iter().map(|d| d.id.0).collect();
    assert_eq!(ids, [0, 1, 2]);
    assert_eq!(log.worst(), Some(Severity::Error));
    assert_eq!(log.halted(), None);
}

#[test]
fn a_finding_is_found_again_by_its_type() {
    let log = sample();
    let changed: Vec<&Changed> = log.of::<Changed>().collect();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].fields.len(), 10);
    assert_eq!(log.of::<NotZip>().count(), 0);
}

#[test]
fn a_fatal_finding_ends_the_run_with_its_id() {
    let mut log = Diagnostics::new();
    let halted = log.at(Stage::Read).halt(NotZip);
    assert_eq!(halted, Halted(log.iter().next().unwrap().id));
    assert_eq!(log.halted(), Some(halted));
}

#[test]
fn a_listener_hears_each_diagnostic_as_it_is_raised() {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let mut log = Diagnostics::new();
    let sink = Arc::clone(&heard);
    log.listen(move |d| sink.lock().unwrap().push(d.code.name));
    log.at(Stage::Read).raise(IndexStale);
    log.at(Stage::Read).raise(NotZip);
    assert_eq!(
        *heard.lock().unwrap(),
        ["history.index-stale", "archive.not-zip"]
    );
}

#[test]
fn a_message_counts_lists_and_cleans() {
    let log = sample();
    let cx = Render::plain();
    let changed = log.iter().nth(2).unwrap();
    assert_eq!(
        changed.message(&cx),
        "10 values brought to what the build reads: 0000000000000001, 0000000000000002, \
         0000000000000003, 0000000000000004, 0000000000000005, 0000000000000006, \
         0000000000000007, 0000000000000008 and 2 more"
    );
    let corrupt = log.iter().nth(1).unwrap();
    assert_eq!(
        cx.subject_name(&corrupt.subject),
        "WAD/Lux.wad.client/a\\u{1b}.dds"
    );
}

#[test]
fn a_diagnostic_serialises_with_its_subject_its_source_and_its_data() {
    let log = sample();
    let json = log.iter().nth(2).unwrap().to_json(&Render::plain());
    assert_eq!(json["id"], 2);
    assert_eq!(json["code"], "fixup.changed");
    assert_eq!(json["severity"], "note");
    assert_eq!(json["channel"], "output");
    assert_eq!(json["stage"], "fix");
    assert_eq!(json["subject"]["kind"], "run");
    assert_eq!(json["by"], "vfx-filtering");
    assert_eq!(json["record"], "#GAME-CHANGE-0001");
    assert_eq!(json["data"]["fields"].as_array().unwrap().len(), 10);
    let stale = log.iter().next().unwrap().to_json(&Render::plain());
    assert!(stale.get("by").is_none() && stale.get("record").is_none());
    assert_eq!(stale["subject"]["id"], "history");
}

#[test]
fn a_filter_selects_by_code_area_severity_stage_and_source() {
    let log = sample();
    let names = |f: &Filter| log.select(f).map(|d| d.code.name).collect::<Vec<_>>();
    assert_eq!(names(&Filter::all()).len(), 3);
    assert_eq!(
        names(&Filter::all().code("archive.*")),
        ["archive.entry-corrupt"]
    );
    assert_eq!(
        names(&Filter::all().code("archive")),
        ["archive.entry-corrupt"]
    );
    assert_eq!(
        names(&Filter::all().code("fixup.changed")),
        ["fixup.changed"]
    );
    assert_eq!(names(&Filter::all().code("fixup.lost")), Vec::<&str>::new());
    assert_eq!(
        names(&Filter::all().at_least(Severity::Warning)),
        ["history.index-stale", "archive.entry-corrupt"]
    );
    assert_eq!(names(&Filter::all().stage(Stage::Fix)), ["fixup.changed"]);
    assert_eq!(
        names(&Filter::all().raised_by("vfx-filtering")),
        ["fixup.changed"]
    );
    assert_eq!(
        names(&Filter::all().raised_by("retype")),
        Vec::<&str>::new()
    );
    assert_eq!(
        names(&Filter::all().channel(Channel::Run)),
        ["history.index-stale"]
    );
    assert_eq!(names(&Filter::all().code("archive").code("fixup")).len(), 2);
}

#[test]
fn a_filter_reads_from_text_and_from_json() {
    let filter = Filter::parse(
        "check.*, severity>=warning, stage=fix, by=retype, record=#MOD-ARTIFACT-0003",
    )
    .unwrap();
    assert_eq!(filter.codes, ["check.*"]);
    assert_eq!(filter.min_severity, Some(Severity::Warning));
    assert_eq!(filter.stages, [Stage::Fix]);
    assert_eq!(filter.by, ["retype"]);
    assert_eq!(filter.records, [Record::mod_artifact(3)]);
    assert_eq!(
        Filter::parse("colour=red"),
        Err(FilterError::Key("colour".into()))
    );
    assert_eq!(
        Filter::parse("severity=loud"),
        Err(FilterError::Value {
            key: "severity",
            value: "loud".into()
        })
    );
    let saved = serde_json::to_string(&filter).unwrap();
    assert_eq!(serde_json::from_str::<Filter>(&saved).unwrap(), filter);
}

#[test]
fn no_diagnostic_blocks_the_write_until_a_policy_says_so() {
    let log = sample();
    assert!(DiagnosticPolicy::default().blocking(&log).is_empty());
    assert!(DiagnosticPolicy::strict().blocking(&log).is_empty());
    let policy = DiagnosticPolicy::default()
        .with(
            Filter::all().code("archive").at_least(Severity::Error),
            Action::Block,
        )
        .with(Filter::all().code("fixup.changed"), Action::Hide);
    let blocking: Vec<u32> = policy.blocking(&log).iter().map(|id| id.0).collect();
    assert_eq!(blocking, [1]);
    let hidden: Vec<&str> = log
        .iter()
        .filter(|d| policy.hides(d))
        .map(|d| d.code.name)
        .collect();
    assert_eq!(hidden, ["fixup.changed"]);
    let saved = serde_json::to_string(&policy).unwrap();
    assert_eq!(
        serde_json::from_str::<DiagnosticPolicy>(&saved).unwrap(),
        policy
    );
}
