//! A repair and a port as the app drives them: a new run from the archive for each request,
//! every stage's state, the report's parts as the views read them, what a watch hears, and the
//! write. The game is `reforge_testkit::skins`.

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::BinHash;
use reforge_app::error::AppError;
use reforge_app::inputs::Inputs;
use reforge_app::run::{
    self, AfterView, RunKind, RunRequest, RunSettings, RunView, StageEvent, StageName, StageState,
    Unwatched, Watch,
};
use reforge_app::runlog::LogLine;
use reforge_app::selection::{FixupChoice, RuleOp, RuleSpec};
use reforge_app::view::Finding;
use reforge_testkit::archive::zip;
use reforge_testkit::bins::{bin, blob, entry, flag, object};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind};
use reforge_testkit::skins::{self, skin_bin};
use reforge_testkit::{ModArchive, SchemaRows};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use zip::CompressionMethod;

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

/// A mod with its own rig at skin0.
fn own_mod() -> ModArchive {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
    ];
    let mut files = own_files("a");
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files)
}

/// A mod of one texture in a WAD of no champion: `load` ends at `archive.no-skin`.
fn no_skin_mod() -> ModArchive {
    ModArchive::new().file("Other.wad.client", "assets/other/x.tex", blob("x"))
}

/// A folder holding one archive, and the inputs over the testkit's game.
struct Bench {
    dir: TempDir,
    inputs: Inputs,
}

impl Bench {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
            inputs: Inputs::of_env(env(skins::game(), NoHistory)),
        }
    }

    fn path(&self, name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(self.dir.path().join(name)).unwrap()
    }

    fn request(&self, archive: &ModArchive, through: StageName) -> RunRequest {
        let path = self.path("mod.fantome");
        fs::write(&path, archive.build()).unwrap();
        RunRequest {
            kind: RunKind::Repair,
            archive: path.to_string(),
            through,
            settings: RunSettings::default(),
            out: Some(self.path("out").to_string()),
        }
    }

    fn try_run(&self, request: &RunRequest) -> Result<RunView, AppError> {
        run::run(&self.inputs, request, &mut Unwatched)
    }

    fn run(&self, request: &RunRequest) -> RunView {
        self.try_run(request).unwrap()
    }
}

fn states(view: &RunView) -> Vec<(StageName, StageState)> {
    view.stages
        .iter()
        .map(|stage| (stage.stage, stage.state))
        .collect()
}

#[test]
fn a_repair_through_write_runs_every_stage_and_writes_the_modpkg() {
    let bench = Bench::new();

    let view = bench.run(&bench.request(&own_mod(), StageName::Write));

    assert_eq!(view.fault, None);
    assert!(
        view.stages
            .iter()
            .all(|stage| stage.state == StageState::Ran),
        "{:?}",
        states(&view)
    );
    let written = view.written.expect("a written package");
    assert!(written.ends_with(".modpkg"), "{written}");
    assert!(Utf8PathBuf::from(&written).is_file());
    assert!(view.package.is_some());
}

#[test]
fn a_repair_stopped_after_fix_has_no_package_and_writes_nothing() {
    let bench = Bench::new();

    let view = bench.run(&bench.request(&own_mod(), StageName::Fix));

    use StageName::*;
    use StageState::*;
    assert_eq!(
        states(&view),
        [
            (Read, Ran),
            (Load, Ran),
            (Fix, Ran),
            (Build, NotRun),
            (Check, NotRun),
            (Write, NotRun)
        ]
    );
    assert_eq!(view.package, None);
    assert_eq!(view.written, None);
    assert!(!bench.path("out").exists());
}

#[test]
fn a_run_halted_in_load_marks_load_and_runs_nothing_after_it() {
    let bench = Bench::new();

    let view = bench.run(&bench.request(&no_skin_mod(), StageName::Write));

    use StageName::*;
    use StageState::*;
    assert_eq!(
        states(&view),
        [
            (Read, Ran),
            (Load, Halted),
            (Fix, NotRun),
            (Build, NotRun),
            (Check, NotRun),
            (Write, NotRun)
        ]
    );
    let halted = view.halted.expect("the halting finding");
    let finding = view.findings.iter().find(|f| f.id == halted).unwrap();
    assert_eq!(finding.code, "archive.no-skin");
    assert!(view.meta.is_some());
}

#[test]
fn the_inputs_findings_come_first_in_every_run() {
    let bench = Bench::new();
    let request = bench.request(&own_mod(), StageName::Load);

    let first = bench.run(&request);
    let second = bench.run(&request);

    let codes =
        |view: &RunView| -> Vec<String> { view.findings.iter().map(|f| f.code.clone()).collect() };
    assert_eq!(codes(&first), codes(&second));
    assert_eq!(
        codes(&first)[..3],
        ["names.mimir-absent", "history.absent", "schema.absent"]
    );
}

#[test]
fn a_finding_the_settings_block_refuses_the_write() {
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), StageName::Write);
    request.settings.block = vec!["history.absent".to_string()];

    let view = bench.run(&request);

    assert!(view.refused);
    assert_eq!(view.written, None);
}

#[test]
fn a_write_with_no_folder_is_refused_before_the_run() {
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), StageName::Write);
    request.out = None;

    assert!(matches!(bench.try_run(&request), Err(AppError::NoOutput)));
}

#[test]
fn a_fixup_the_library_does_not_have_is_refused() {
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), StageName::Fix);
    request.settings.fixups.off = vec!["no-such-fixup".to_string()];

    let Err(AppError::UnknownFixup { id }) = bench.try_run(&request) else {
        panic!("an unknown fixup was taken");
    };
    assert_eq!(id, "no-such-fixup");
}

#[test]
fn what_a_fixup_changed_reaches_the_view_value_by_value() {
    // #MOD-ARTIFACT-0020: a flag tools write on an emitter, which no build declared.
    const EMITTER: &str = "VfxEmitterDefinitionData";
    let schema = SchemaRows::new(300)
        .class(EMITTER, &[field("isLocalSpace", kind("Bool"))])
        .build();
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema)
        .build()
        .unwrap();
    let mut emitter = object("Emitter", EMITTER, vec![("isLocalSpace", flag(false))]);
    emitter.properties.insert(BinHash(0x0863_79e1), flag(true));
    let bench = Bench {
        inputs: Inputs::of_env(env),
        ..Bench::new()
    };

    let view = bench.run(&bench.request(&skin_archive(vec![emitter]), StageName::Fix));

    let fixup = view
        .changes
        .iter()
        .find(|fixup| fixup.fixup == "known-artifacts-vfx-emitter")
        .unwrap_or_else(|| panic!("no change of the emitter's fixup in {:?}", view.changes));
    assert_eq!(fixup.removed, 1);
    assert_eq!(fixup.changes.len(), 1);
    let change = &fixup.changes[0];
    assert_eq!(change.after, AfterView::Removed);
    assert_eq!(
        change.before.as_ref().map(|value| value.text.as_str()),
        Some("true")
    );
}

#[test]
fn a_mods_name_reaches_the_view_cleaned() {
    let bench = Bench::new();
    // A right-to-left override in the name, an escape sequence in the author.
    let info = serde_json::json!({
        "Name": format!("Evil{}Name", '\u{202e}'),
        "Author": format!("a{}[31m", '\u{1b}'),
    })
    .to_string();
    let bytes = zip(&[("META/info.json", info.as_bytes(), CompressionMethod::Stored)]);
    let mut request = bench.request(&ModArchive::new(), StageName::Read);
    fs::write(&request.archive, bytes).unwrap();
    request.through = StageName::Read;

    let view = bench.run(&request);

    let meta = view.meta.expect("the mod's meta");
    assert_eq!(meta.name.as_deref(), Some(r"Evil\u{202e}Name"));
    assert_eq!(meta.author.as_deref(), Some(r"a\u{1b}[31m"));
}

/// What a watch heard of a run.
#[derive(Default)]
struct Heard {
    stages: Vec<(StageName, &'static str)>,
    findings: Arc<Mutex<Vec<Finding>>>,
    lines: Arc<Mutex<Vec<LogLine>>>,
}

impl Watch for Heard {
    fn stage(&mut self, stage: StageName, event: StageEvent) {
        let event = match event {
            StageEvent::Started => "started",
            StageEvent::Ended { .. } => "ended",
        };
        self.stages.push((stage, event));
    }

    fn findings(&mut self) -> Option<Box<dyn FnMut(Finding) + Send>> {
        let findings = Arc::clone(&self.findings);
        Some(Box::new(move |finding| {
            findings.lock().unwrap().push(finding)
        }))
    }

    fn log(&mut self) -> Option<Box<dyn Fn(LogLine) + Send + Sync>> {
        let lines = Arc::clone(&self.lines);
        Some(Box::new(move |line| lines.lock().unwrap().push(line)))
    }
}

#[test]
fn a_port_runs_the_stages_of_a_port_and_names_each_units_target() {
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), StageName::Check);
    request.kind = RunKind::Port;

    let view = bench.run(&request);

    let stages: Vec<StageName> = view.stages.iter().map(|stage| stage.stage).collect();
    use StageName::*;
    assert_eq!(
        stages,
        [Read, Load, Fix, Facts, Decide, Plan, Build, Check, Write]
    );
    assert!(
        view.stages[..8]
            .iter()
            .all(|stage| stage.state == StageState::Ran),
        "{:?}",
        states(&view)
    );
    let targets = view.targets.expect("the decisions");
    assert_eq!(targets.policy.target, "auto");
    let decision = &targets.decisions[0];
    assert_eq!(decision.chosen, Some(0));
    assert_eq!(decision.character_name, "test");
    assert!(!decision.candidates.is_empty());
    // The other three levels of animation reuse, compared.
    assert_eq!(decision.alternatives.len(), 3);
}

#[test]
fn a_repair_has_no_decisions() {
    let bench = Bench::new();

    let view = bench.run(&bench.request(&own_mod(), StageName::Check));

    assert_eq!(view.targets, None);
}

#[test]
fn a_policy_the_library_does_not_have_is_refused_before_the_run() {
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), StageName::Check);
    request.kind = RunKind::Port;
    request.settings.target = "premium".to_string();

    assert!(matches!(
        bench.try_run(&request),
        Err(AppError::BadPolicy { .. })
    ));
}

#[test]
fn a_watch_hears_each_stage_start_and_end_in_order_and_each_finding() {
    let bench = Bench::new();
    let mut heard = Heard::default();

    let view = run::run(
        &bench.inputs,
        &bench.request(&own_mod(), StageName::Fix),
        &mut heard,
    )
    .unwrap();

    use StageName::*;
    assert_eq!(
        heard.stages,
        [
            (Read, "started"),
            (Read, "ended"),
            (Load, "started"),
            (Load, "ended"),
            (Fix, "started"),
            (Fix, "ended")
        ]
    );
    assert!(view.stages[..3].iter().all(|stage| stage.millis.is_some()));
    // The inputs' findings are in the log before the run starts; the watch hears the run's.
    let heard_codes: Vec<String> = heard
        .findings
        .lock()
        .unwrap()
        .iter()
        .map(|f| f.code.clone())
        .collect();
    let run_codes: Vec<String> = view
        .findings
        .iter()
        .filter(|f| f.channel != "run")
        .map(|f| f.code.clone())
        .collect();
    assert_eq!(heard_codes, run_codes);
}

fn remove_rule(field: &str) -> RuleSpec {
    RuleSpec {
        id: "remove-tool-field".to_string(),
        title: Some("A tool's field, removed".to_string()),
        records: Vec::new(),
        phase: reforge_app::catalog::FixupPhase::Undo,
        default: true,
        class: "VfxEmitterDefinitionData".to_string(),
        op: RuleOp::Remove {
            fields: vec![field.to_string()],
        },
    }
}

#[test]
fn a_rule_added_in_the_settings_runs_as_a_fixup() {
    const EMITTER: &str = "VfxEmitterDefinitionData";
    let schema = SchemaRows::new(300)
        .class(EMITTER, &[field("isLocalSpace", kind("Bool"))])
        .build();
    let env = Env::builder()
        .game(skins::game_at("16.19.300"))
        .schema(schema)
        .build()
        .unwrap();
    let emitter = object(
        "Emitter",
        EMITTER,
        vec![("isLocalSpace", flag(false)), ("toolField", flag(true))],
    );
    let bench = Bench {
        inputs: Inputs::of_env(env),
        ..Bench::new()
    };
    let mut request = bench.request(&skin_archive(vec![emitter]), StageName::Fix);
    request.settings.fixups.rules = vec![remove_rule("toolField")];

    let view = bench.run(&request);

    let fixup = view
        .changes
        .iter()
        .find(|fixup| fixup.fixup == "remove-tool-field")
        .unwrap_or_else(|| panic!("the rule changed nothing: {:?}", view.changes));
    assert_eq!(fixup.removed, 1);
    assert_eq!(fixup.changes[0].path, "toolField");
}

#[test]
fn a_choice_saved_as_a_selection_file_reads_back_the_same() {
    let bench = Bench::new();
    let path = bench.path("selection.json");
    let choice = FixupChoice {
        off: vec!["bank-id".to_string()],
        on: Vec::new(),
        rules: vec![remove_rule("toolField")],
    };

    choice.save(&path).unwrap();

    assert_eq!(FixupChoice::open(&path).unwrap(), choice);
    // The library reads the file, as the command does with `--fixups`.
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        reforge::fixups::FixupSelection::from_json(&text).is_ok(),
        "{text}"
    );
}
