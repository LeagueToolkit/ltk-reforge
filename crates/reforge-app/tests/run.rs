//! A repair and a port as the app drives them: a new run from the archive for each request,
//! every stage's state, the report's parts as the views read them, what a watch hears, and the
//! write. The game is `reforge_testkit::skins`.

use camino::Utf8PathBuf;
use fs_err as fs;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use reforge::diag::Cancel;
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::BinHash;
use reforge_app::error::AppError;
use reforge_app::inputs::Inputs;
use reforge_app::run::{
    self, AfterView, KeptFileView, OutcomeView, PartView, RunKind, RunRequest, RunSettings,
    RunView, StageEvent, StageName, StageState, UnitTargetView, Unwatched, Watch,
};
use reforge_app::selection::{FixupChoice, RuleOp, RuleSpec};
use reforge_app::view::Finding;
use reforge_testkit::archive::zip;
use reforge_testkit::bins::{Skin, bin, blob, entry, flag, object};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind};
use reforge_testkit::skins::{self, BASE_TEX, WAD, skin_bin};
use reforge_testkit::{MemoryHistory, ModArchive, SchemaRows};
use std::io::Cursor;
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
            format: reforge_app::settings::PackageFormat::default(),
            keep: false,
        }
    }

    fn try_run(&self, request: &RunRequest) -> Result<RunView, AppError> {
        run::run(&self.inputs, request, None, &mut Unwatched, None)
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

/// #ADR-0105: the package is a `.fantome` unless the request asks for a `.modpkg`.
#[test]
fn a_repair_through_write_runs_every_stage_and_writes_the_package() {
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
    assert!(written.ends_with(".fantome"), "{written}");
    assert!(Utf8PathBuf::from(&written).is_file());
    assert!(view.package.is_some());

    let request = RunRequest {
        format: reforge_app::settings::PackageFormat::Modpkg,
        ..bench.request(&own_mod(), StageName::Write)
    };
    let view = bench.run(&request);
    assert_eq!(view.fault, None);
    let written = view.written.expect("a written package");
    assert!(written.ends_with(".modpkg"), "{written}");
    assert!(Utf8PathBuf::from(&written).is_file());
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
    // The field has no name in any table, so it is its hash; the class has one.
    assert_eq!(change.field, format!("{EMITTER}.086379e1"));
    assert_eq!(change.outcome, OutcomeView::Removed);
    let object = view
        .objects
        .iter()
        .find(|object| object.id == change.object);
    let object = object.unwrap_or_else(|| panic!("{} is not in {:?}", change.object, view.objects));
    assert_eq!(object.class, EMITTER);
    assert!(object.bin.ends_with(".bin"), "{}", object.bin);
}

#[test]
fn each_stage_that_gives_a_part_sends_it_as_it_ends_and_the_view_holds_the_same() {
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), StageName::Check);
    request.kind = RunKind::Port;
    let mut heard = Heard::default();

    let view = run::run(&bench.inputs, &request, None, &mut heard, None).unwrap();

    let [
        PartView::Names { names },
        PartView::Changes { changes, objects },
        PartView::Targets { targets },
        PartView::Package { package },
    ] = heard.parts.as_slice()
    else {
        panic!(
            "the parts of a port, in the order of its stages: {:?}",
            heard.parts
        );
    };
    // The names `load` sent are the tables'; the view adds the changed objects' after them.
    assert!(!names.is_empty(), "load sent no names");
    for (id, name) in names {
        assert_eq!(view.names.get(id), Some(name), "{id}");
    }
    assert!(names.keys().any(|id| id.starts_with("%u")), "{names:?}");
    assert_eq!(changes, &view.changes);
    assert_eq!(objects, &view.objects);
    assert_eq!(Some(targets), view.targets.as_ref());
    assert_eq!(Some(package), view.package.as_ref());
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
    parts: Vec<PartView>,
}

impl Watch for Heard {
    fn takes_parts(&self) -> bool {
        true
    }

    fn part(&mut self, part: PartView) {
        self.parts.push(part);
    }

    fn stage(&mut self, stage: StageName, event: StageEvent) {
        let event = match event {
            StageEvent::Started => "started",
            StageEvent::Ended { .. } => "ended",
            StageEvent::Resumed => "resumed",
        };
        self.stages.push((stage, event));
    }

    fn findings(&mut self) -> Option<Box<dyn FnMut(Finding) + Send>> {
        let findings = Arc::clone(&self.findings);
        Some(Box::new(move |finding| {
            findings.lock().unwrap().push(finding)
        }))
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
    // What a finding's fields name by id.
    let name = |id: &str| view.names.get(id).map(String::as_str);
    assert_eq!(name("%u0"), Some("test skin0"));
    assert_eq!(name("%c0"), Some("test"));
    assert!(name("%w0").is_some_and(|wad| wad.ends_with(".wad.client")));
}

#[test]
fn the_stages_of_a_kind_are_the_stages_its_run_shows() {
    let bench = Bench::new();
    for kind in [RunKind::Repair, RunKind::Port] {
        let mut request = bench.request(&own_mod(), StageName::Check);
        request.kind = kind;

        let view = bench.run(&request);

        let shown: Vec<StageName> = view.stages.iter().map(|stage| stage.stage).collect();
        assert_eq!(run::stages_of(&bench.inputs, kind), shown, "{kind:?}");
    }
}

#[test]
fn a_mods_image_reaches_the_view_as_the_packages_webp_thumbnail() {
    let bench = Bench::new();
    let mut png = Vec::new();
    let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(3, 2, Rgb([200, 40, 60])));
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .unwrap();
    let with_image = own_mod().extra("META/image.png", png);

    let view = bench.run(&bench.request(&with_image, StageName::Check));
    let without = bench.run(&bench.request(&own_mod(), StageName::Check));

    let thumbnail = view.package.and_then(|package| package.thumbnail);
    let thumbnail = thumbnail.expect("a thumbnail");
    assert!(
        thumbnail.starts_with("data:image/webp;base64,"),
        "{thumbnail:.40}"
    );
    assert_eq!(without.package.expect("a package").thumbnail, None);
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
        None,
        &mut heard,
        None,
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

#[test]
fn a_cancel_set_before_the_run_ends_it_in_its_first_stage_and_writes_nothing() {
    let bench = Bench::new();
    let cancel = Cancel::new();
    cancel.cancel();

    let request = bench.request(&own_mod(), StageName::Write);
    let view = run::run(&bench.inputs, &request, None, &mut Unwatched, Some(&cancel)).unwrap();

    let cancelled = view.cancelled.expect("the stage the cancel ended");
    assert!(
        matches!(cancelled, StageName::Read | StageName::Load),
        "{cancelled:?}"
    );
    assert_eq!(view.fault, None);
    assert_eq!(view.halted, None);
    assert_eq!(view.written, None);
    assert!(!bench.path("out").exists());
    assert!(
        view.stages
            .iter()
            .filter(|stage| stage.stage >= cancelled)
            .all(|stage| stage.state == StageState::NotRun),
        "{:?}",
        states(&view)
    );
}

/// A watch that sets a cancel as `check` ends.
struct CancelAfterCheck(Cancel);

impl Watch for CancelAfterCheck {
    fn stage(&mut self, stage: StageName, event: StageEvent) {
        if let (StageName::Check, StageEvent::Ended { .. }) = (stage, event) {
            self.0.cancel();
        }
    }
}

#[test]
fn a_cancel_set_after_check_stops_the_write() {
    let bench = Bench::new();
    let cancel = Cancel::new();
    let mut watch = CancelAfterCheck(cancel.clone());

    let request = bench.request(&own_mod(), StageName::Write);
    let view = run::run(&bench.inputs, &request, None, &mut watch, Some(&cancel)).unwrap();

    assert_eq!(view.cancelled, Some(StageName::Write));
    assert!(view.package.is_some());
    assert_eq!(view.written, None);
    assert!(!bench.path("out").exists());
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
fn a_unit_given_no_target_is_left_to_the_game_by_its_own_setting() {
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), StageName::Check);
    request.kind = RunKind::Port;
    request.settings.unit_targets = vec![UnitTargetView {
        character: "Test".to_string(),
        slot: 0,
        target: None,
    }];

    let view = bench.run(&request);

    let targets = view.targets.expect("the decisions");
    let decision = &targets.decisions[0];
    assert_eq!(
        (decision.chosen, decision.reason.as_str()),
        (None, "per-unit")
    );
}

#[test]
fn a_unit_target_or_a_kept_file_that_names_nothing_is_refused_before_the_run() {
    let bench = Bench::new();
    let mut request = bench.request(&own_mod(), StageName::Check);
    request.kind = RunKind::Port;
    request.settings.unit_targets = vec![UnitTargetView {
        character: "a/b".to_string(),
        slot: 0,
        target: Some(1),
    }];
    assert!(matches!(
        bench.try_run(&request),
        Err(AppError::BadSetting { .. })
    ));

    request.settings.unit_targets.clear();
    request.settings.kept_files = vec![KeptFileView {
        wad: WAD.to_string(),
        path: String::new(),
    }];
    assert!(matches!(
        bench.try_run(&request),
        Err(AppError::BadSetting { .. })
    ));
}

/// The base texture as the game shipped it one build ago.
const STALE_TEX: &str = "the base texture one build ago";

/// The mod's own rig at skin0, its skin naming the base's texture, which it ships as the game
/// had it one build ago: a stale copy (#MOD-ARTIFACT-0004).
fn stale_mod() -> ModArchive {
    let skin = Skin {
        texture: BASE_TEX.into(),
        ..own_skin("a", GRAPH0, RESOURCES0)
    };
    let mut files = own_files("a");
    files.push((BASE_TEX.into(), blob(STALE_TEX)));
    let objects = vec![entry(SKIN0, &skin), own_graph("a", GRAPH0)];
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files)
}

#[test]
fn a_repair_with_a_kept_file_carries_the_stale_copy() {
    let history =
        MemoryHistory::new(&["16.18.1", "16.19.1"]).with_content(BASE_TEX, &blob(STALE_TEX), 0..=0);
    let bench = Bench {
        inputs: Inputs::of_env(env(skins::game(), history)),
        ..Bench::new()
    };
    let standard = bench.request(&stale_mod(), StageName::Check);
    let mut keeping = standard.clone();
    keeping.settings.kept_files = vec![KeptFileView {
        wad: WAD.to_string(),
        path: BASE_TEX.to_string(),
    }];

    let dropped = bench.run(&standard);
    let kept = bench.run(&keeping);

    let stale = |view: &RunView| view.findings.iter().any(|f| f.code == "file.stale");
    assert!(stale(&dropped), "the standard run finds the stale copy");
    assert!(!stale(&kept), "the kept file is no stale copy");
    let chunks = |view: &RunView| view.package.as_ref().expect("a package").chunks;
    assert_eq!(chunks(&kept), chunks(&dropped) + 1);
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
