//! A kept run as the Runs page reads it: the files at each stage with their marks, sizes and
//! kinds, one file opened by its kind, and two runs compared. The game is
//! `reforge_testkit::skins`.

use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_meta::BinObject;
use reforge::env::Env;
use reforge::history::NoHistory;
use reforge::model::entity::FileId;
use reforge::model::{BinHash, ByteView, ChunkPath, ContentId, Spelled, WadName, name_hash};
use reforge::package::{
    Chunk, ChunkKey, Layer, Modpkg, Origin as ChunkOrigin, Package, PackageWriter, ProjectConfig,
};
use reforge::store::BlobStore;
use reforge_app::error::AppError;
use reforge_app::inputs::Inputs;
use reforge_app::run::{
    self, RunKind, RunRequest, RunSettings, RunView, StageName, StageState, UnitTargetView,
    Unwatched,
};
use reforge_app::store::{
    AppStore, Counts, EVIDENCE_ONLY_SCHEMA, FileRow, FileView, HEX_BYTES, Mark, Origin, StageFiles,
};
use reforge_app::view::Finding;
use reforge_testkit::bins::{bin, entry, float};
use reforge_testkit::facts::{archive, env, mod_skin, own_files, own_graph, own_skin};
use reforge_testkit::packs::{DdsPixels, dds};
use reforge_testkit::skins::{self, WAD, skin_bin};
use reforge_testkit::{ModArchive, SchemaRows};
use tempfile::TempDir;

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

/// An object with a field a tool wrote and no build declared (#MOD-ARTIFACT-0020), which
/// `fix` removes, so it writes the skin bin again.
fn tool_artifact() -> BinObject {
    let entry = name_hash("Characters/Test/Skins/Skin0/Tint");
    BinObject::builder(entry, name_hash("ValueColor"))
        .property(BinHash(0xecbe_c3b3), float(1.0))
        .build()
}

/// A mod with its own rig at skin0, its skin bin holding a tool's field.
fn artifact_mod() -> ModArchive {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
        tool_artifact(),
    ];
    let mut files = own_files("a");
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files)
}

/// [`artifact_mod`] with its texture a DDS, which a build puts in TEX's container.
fn textured_mod() -> ModArchive {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
        tool_artifact(),
    ];
    let mut files = own_files("a");
    for (path, bytes) in &mut files {
        if path.ends_with(".tex") {
            *bytes = dds(DdsPixels::Dxt1, (8, 4), 1);
        }
    }
    files.push((skin_bin("test", 0), bin(objects)));
    archive(&files)
}

/// A store, the inputs over the testkit's game, and a folder for archives and packages.
struct Bench {
    dir: TempDir,
    inputs: Inputs,
    store: AppStore,
}

impl Bench {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = AppStore::open(&Self::root_of(&dir).join("store")).unwrap();
        Self {
            dir,
            inputs: Inputs::of_env(env(skins::game(), NoHistory)),
            store,
        }
    }

    fn root_of(dir: &TempDir) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap()
    }

    /// A run of `archive` through write, kept in the store.
    fn kept(&self, archive: &ModArchive, kind: RunKind) -> (u64, RunView) {
        self.kept_with(archive, kind, RunSettings::default())
    }

    /// A run of `archive` through write with `settings`, kept in the store.
    fn kept_with(
        &self,
        archive: &ModArchive,
        kind: RunKind,
        settings: RunSettings,
    ) -> (u64, RunView) {
        let root = Self::root_of(&self.dir);
        let path = root.join("mod.fantome");
        fs::write(&path, archive.build()).unwrap();
        let request = RunRequest {
            kind,
            archive: path.to_string(),
            through: StageName::Write,
            settings,
            out: Some(root.join("out").to_string()),
            keep: false,
        };
        let view = run::run(
            &self.inputs,
            &request,
            Some(&self.store),
            &mut Unwatched,
            None,
        )
        .unwrap();
        assert_eq!(view.fault, None);
        (view.saved.expect("the run is kept"), view)
    }

    fn files(&self, id: u64, stage: StageName) -> StageFiles {
        let files = self.store.stage_files(id, stage).unwrap();
        files.unwrap_or_else(|| panic!("no run {id}"))
    }
}

/// The file at a path of a stage's WAD.
fn file<'a>(files: &'a StageFiles, wad: &str, path: &str) -> &'a FileRow {
    let wad = files.wads.iter().find(|w| w.name == wad);
    let wad = wad.unwrap_or_else(|| panic!("no WAD {wad:?} at {:?}", files.stage));
    let found = wad.files.iter().find(|f| f.path.as_deref() == Some(path));
    found.unwrap_or_else(|| panic!("no {path} at {:?}: {:?}", files.stage, wad.files))
}

#[test]
fn a_bin_the_fixups_rewrote_is_changed_at_fix_and_the_same_at_build() {
    let bench = Bench::new();
    let (id, _) = bench.kept(&artifact_mod(), RunKind::Repair);
    let skin = skin_bin("test", 0);

    let load = bench.files(id, StageName::Load);
    let fix = bench.files(id, StageName::Fix);
    let build = bench.files(id, StageName::Build);

    let loaded = file(&load, WAD, &skin);
    assert_eq!(loaded.mark, Mark::Same);
    assert_eq!(loaded.kind, "property-bin");
    assert!(loaded.size > 0);
    assert_eq!(loaded.content.len(), 64);
    assert_eq!(loaded.hash.len(), 16);
    let fixed = file(&fix, WAD, &skin);
    assert_eq!(fixed.mark, Mark::Changed);
    assert_ne!(fixed.content, loaded.content);
    assert_eq!(fixed.kind, "property-bin");
    let built = file(&build, WAD, &skin);
    assert_eq!(built.mark, Mark::Same);
    assert_eq!(built.content, fixed.content);
    let marks = |files: &StageFiles| -> Vec<Mark> {
        let rows = files.wads.iter().flat_map(|wad| &wad.files);
        rows.map(|row| row.mark).collect()
    };
    assert!(marks(&load).iter().all(|&mark| mark == Mark::Same));
    assert_eq!(
        marks(&fix).iter().filter(|&&m| m == Mark::Changed).count(),
        1,
        "only the skin bin changed: {fix:?}"
    );
}

#[test]
fn the_files_of_a_wad_are_in_path_order() {
    let bench = Bench::new();
    let (id, _) = bench.kept(&artifact_mod(), RunKind::Repair);

    let load = bench.files(id, StageName::Load);

    for wad in &load.wads {
        let paths: Vec<_> = wad.files.iter().map(|f| f.path.clone()).collect();
        let mut sorted = paths.clone();
        sorted.sort_by_key(|path| (path.is_none(), path.clone()));
        assert_eq!(paths, sorted);
    }
}

#[test]
fn a_stage_that_names_no_files_or_was_not_reached_is_refused() {
    let bench = Bench::new();
    let (id, _) = bench.kept(&artifact_mod(), RunKind::Repair);

    for stage in [StageName::Read, StageName::Facts, StageName::Plan] {
        let refused = bench.store.stage_files(id, stage);
        assert_eq!(refused, Err(AppError::NoFilesAtStage { stage }));
    }
    assert_eq!(bench.store.stage_files(id + 1, StageName::Load), Ok(None));
}

#[test]
fn a_ports_files_at_plan_are_marked_against_fix() {
    let bench = Bench::new();
    let (id, view) = bench.kept(&artifact_mod(), RunKind::Port);
    assert!(view.targets.is_some());

    let plan = bench.files(id, StageName::Plan);
    let build = bench.files(id, StageName::Build);

    let marks = |files: &StageFiles, mark: Mark| {
        let rows = files.wads.iter().flat_map(|wad| &wad.files);
        rows.filter(|row| row.mark == mark).count()
    };
    // The port places the mod's files at paths of its own.
    assert!(marks(&plan, Mark::Removed) > 0 && marks(&plan, Mark::Added) > 0);
    assert!(marks(&build, Mark::Same) > 0);
    assert!(
        plan.wads
            .iter()
            .flat_map(|wad| &wad.files)
            .all(|row| row.kind != "missing"),
        "{plan:?}"
    );
}

/// The fixup that removes [`tool_artifact`]'s field.
const ARTIFACT_FIXUP: &str = "known-artifacts-value-color";

/// [`artifact_mod`] ported twice, the second run with [`ARTIFACT_FIXUP`] off.
fn port_pair(bench: &Bench) -> (u64, u64) {
    let (a, _) = bench.kept(&artifact_mod(), RunKind::Port);
    let mut settings = RunSettings::default();
    settings.fixups.off.push(ARTIFACT_FIXUP.to_string());
    let (b, _) = bench.kept_with(&artifact_mod(), RunKind::Port, settings);
    (a, b)
}

#[test]
fn a_fixup_turned_off_shows_in_the_settings_the_findings_and_the_changes() {
    let bench = Bench::new();
    let (a, b) = port_pair(&bench);

    let compared = bench
        .store
        .compare(a, b)
        .unwrap()
        .expect("both runs are kept");

    assert!(compared.same_mod);
    assert_eq!((compared.a.id, compared.b.id), (a, b));
    let keys: Vec<&str> = compared.settings.iter().map(|d| d.key.as_str()).collect();
    assert_eq!(keys, ["fixups_off"]);
    assert_eq!(compared.settings[0].a, "");
    assert_eq!(compared.settings[0].b, ARTIFACT_FIXUP);
    assert_eq!(compared.inputs, []);
    let gone = &compared.findings.gone;
    assert!(
        !gone.is_empty() && gone.iter().all(|f| f.by.as_deref() == Some(ARTIFACT_FIXUP)),
        "{gone:?}"
    );
    assert!(compared.findings.same > 0);
    let change = compared.changes.iter().find(|c| c.fixup == ARTIFACT_FIXUP);
    let change = change.unwrap_or_else(|| panic!("{:?}", compared.changes));
    assert!(change.differs);
    assert!(change.a.removed > 0);
    assert_eq!(change.b, Counts::default());
    assert!(!compared.targets.is_empty());
    assert!(compared.targets.iter().all(|target| !target.differs));
}

/// A mod with a model of its own at skin0 and another at skin3, so a port that leaves one
/// unit to the game still writes the other.
fn two_skin_mod() -> ModArchive {
    let skin3 = "Characters/Test/Skins/Skin3";
    let graph3 = "Characters/Test/Animations/Skin3";
    let resources3 = "Characters/Test/Skins/Skin3/Resources";
    let mut files = own_files("a");
    files.extend(own_files("b"));
    files.push(mod_skin(
        "test",
        0,
        SKIN0,
        &own_skin("a", GRAPH0, RESOURCES0),
        vec![own_graph("a", GRAPH0)],
    ));
    files.push(mod_skin(
        "test",
        3,
        skin3,
        &own_skin("b", graph3, resources3),
        vec![own_graph("b", graph3)],
    ));
    archive(&files)
}

#[test]
fn a_unit_left_to_the_game_in_one_port_lists_its_unit_targets_against_none() {
    let bench = Bench::new();
    let (a, _) = bench.kept(&two_skin_mod(), RunKind::Port);
    let mut settings = RunSettings::default();
    settings.unit_targets.push(UnitTargetView {
        character: "Test".to_string(),
        slot: 3,
        target: None,
    });
    let (b, _) = bench.kept_with(&two_skin_mod(), RunKind::Port, settings);

    let compared = bench
        .store
        .compare(a, b)
        .unwrap()
        .expect("both runs are kept");

    let keys: Vec<&str> = compared.settings.iter().map(|d| d.key.as_str()).collect();
    assert_eq!(keys, ["unit_targets"]);
    assert_eq!(compared.settings[0].a, "");
    assert_eq!(compared.settings[0].b, "test:3=none");
}

#[test]
fn a_repair_against_a_port_differs_in_its_kind_and_has_no_targets() {
    let bench = Bench::new();
    let (a, _) = bench.kept(&artifact_mod(), RunKind::Repair);
    let (b, _) = bench.kept(&artifact_mod(), RunKind::Port);

    let compared = bench
        .store
        .compare(a, b)
        .unwrap()
        .expect("both runs are kept");

    let kind = compared.settings.iter().find(|d| d.key == "kind");
    let kind = kind.unwrap_or_else(|| panic!("{:?}", compared.settings));
    assert_eq!((kind.a.as_str(), kind.b.as_str()), ("repair", "port"));
    assert_eq!(compared.targets, []);
    assert!(compared.package.a.is_some() && compared.package.b.is_some());
}

#[test]
fn a_comparison_with_a_run_not_kept_is_none() {
    let bench = Bench::new();
    let (a, _) = bench.kept(&artifact_mod(), RunKind::Repair);

    assert_eq!(bench.store.compare(a, a + 1), Ok(None));
    assert_eq!(bench.store.compare(a + 1, a), Ok(None));
}

#[test]
fn two_runs_files_at_fix_are_marked_against_each_other() {
    let bench = Bench::new();
    let (a, b) = port_pair(&bench);
    let skin = skin_bin("test", 0);

    let fix = bench.store.compare_files(a, b, StageName::Fix).unwrap();
    let fix = fix.expect("both runs are kept");

    assert_eq!(fix.stage, StageName::Fix);
    assert_eq!(file(&fix, WAD, &skin).mark, Mark::Changed);
    let rows = fix.wads.iter().flat_map(|wad| &wad.files);
    let others = rows.filter(|row| row.path.as_deref() != Some(skin.as_str()));
    assert!(others.clone().count() > 0);
    assert!(others.clone().all(|row| row.mark == Mark::Same), "{fix:?}");
}

#[test]
fn files_compared_at_a_stage_a_run_lacks_are_refused() {
    let bench = Bench::new();
    let (a, _) = bench.kept(&artifact_mod(), RunKind::Repair);
    let (b, _) = bench.kept(&artifact_mod(), RunKind::Port);

    let refused = bench.store.compare_files(a, b, StageName::Plan);

    assert_eq!(
        refused,
        Err(AppError::NoFilesAtStage {
            stage: StageName::Plan
        })
    );
    assert_eq!(
        bench.store.compare_files(a, b + 1, StageName::Fix),
        Ok(None)
    );
}

/// The one file of a stage whose path ends so.
fn ending<'a>(files: &'a StageFiles, end: &str) -> &'a FileRow {
    let rows = files.wads.iter().flat_map(|wad| &wad.files);
    let mut found = rows.filter(|row| row.path.as_deref().is_some_and(|p| p.ends_with(end)));
    let first = found
        .next()
        .unwrap_or_else(|| panic!("no file ends {end}: {files:?}"));
    assert!(found.next().is_none(), "two files end {end}");
    first
}

fn content(row: &FileRow) -> ContentId {
    ContentId::parse(&row.content).unwrap()
}

#[test]
fn a_bin_opens_as_its_objects_with_their_names() {
    let bench = Bench::new();
    let (id, _) = bench.kept(&artifact_mod(), RunKind::Repair);
    let fix = bench.files(id, StageName::Fix);
    let skin = file(&fix, WAD, &skin_bin("test", 0));
    let names = bench.inputs.env().names();

    let view = bench
        .store
        .file_view(id, content(skin), Some(names))
        .unwrap();

    let FileView::Bin { text, more } = view else {
        panic!("not a bin: {view:?}");
    };
    assert_eq!(more, 0);
    // The entry is spelled as the run learnt it, the class as the tables do.
    let head = format!("{} = SkinCharacterDataProperties {{", SKIN0.to_lowercase());
    assert!(text.to_lowercase().contains(&head.to_lowercase()), "{text}");
}

#[test]
fn a_texture_opens_as_a_picture_the_app_decoded() {
    let bench = Bench::new();
    let (id, _) = bench.kept(&textured_mod(), RunKind::Repair);
    let load = bench.files(id, StageName::Load);
    let build = bench.files(id, StageName::Build);

    for (row, format) in [
        (ending(&load, ".tex"), "DXT1"),
        (ending(&build, ".tex"), "Bc1"),
    ] {
        let view = bench.store.file_view(id, content(row), None).unwrap();

        let FileView::Image {
            png,
            width,
            height,
            format: shown,
        } = view
        else {
            panic!("not a picture: {view:?}");
        };
        assert!(png.starts_with("data:image/png;base64,"), "{png}");
        assert_eq!((width, height), (8, 4));
        assert_eq!(shown, format);
    }
}

#[test]
fn bytes_of_no_kind_open_as_their_first_bytes_in_hex() {
    let bench = Bench::new();
    let bytes: Vec<u8> = (0..300u32).map(|i| (i * 7 % 251) as u8 | 0x80).collect();
    let id = bench.store.tier().put(ByteView::from(bytes.clone()));

    let view = bench.store.file_view(1, id, None).unwrap();

    let FileView::Hex { head, size } = view else {
        panic!("not hex: {view:?}");
    };
    assert_eq!(size, 300);
    let lines: Vec<&str> = head.lines().collect();
    assert_eq!(lines.len(), HEX_BYTES / 16);
    assert_eq!(lines[0].split(' ').count(), 16);
    assert!(lines[0].starts_with(&format!("{:02x} {:02x}", bytes[0], bytes[1])));
}

#[test]
fn bytes_no_tier_holds_are_refused() {
    let bench = Bench::new();
    let id = ContentId::of(b"never kept");

    let refused = bench.store.file_view(1, id, None);

    assert_eq!(refused, Err(AppError::BlobMissing { id: id.hex() }));
}

/// A port of [`artifact_mod`] kept, and a fresh store beside the bench's.
fn ported_and_fresh(bench: &Bench) -> (RunView, AppStore) {
    let (_, view) = bench.kept(&artifact_mod(), RunKind::Port);
    let fresh = AppStore::open(&Bench::root_of(&bench.dir).join("fresh")).unwrap();
    (view, fresh)
}

/// Each decision of a view by its unit's name, with the slot chosen.
fn decided(view: &RunView) -> Vec<(String, Option<u16>)> {
    let targets = view.targets.as_ref().expect("the decisions");
    let decisions = targets.decisions.iter();
    decisions
        .map(|d| {
            let unit = view.names.get(&d.unit).unwrap_or(&d.unit);
            (unit.clone(), d.chosen)
        })
        .collect()
}

/// The findings of a run raised before its write: what the report beside its package holds,
/// since the writer writes that file before the write raises what it wrote.
fn before_write(view: &RunView) -> Vec<Finding> {
    let findings = view.findings.iter().filter(|f| f.stage != "write");
    findings.cloned().collect()
}

#[test]
fn a_package_opened_reads_as_the_run_that_wrote_it() {
    let bench = Bench::new();
    let (view, fresh) = ported_and_fresh(&bench);
    let package = Utf8PathBuf::from(view.written.clone().expect("the package"));

    let id = fresh.open_package(&package).unwrap();

    let listed = fresh.list().unwrap();
    assert_eq!(listed.len(), 1);
    let row = &listed[0];
    assert_eq!(row.id, id);
    assert_eq!(row.origin, Origin::Package);
    assert_eq!(row.opened_from.as_deref(), Some(package.as_str()));
    assert_eq!(row.package.as_deref(), Some(package.as_str()));
    assert!(row.kept && !row.archive_ingested);
    assert_eq!(row.reached, []);
    assert_eq!(row.kind, RunKind::Port);
    let built = view.package.as_ref().expect("the package view");
    assert!(row.name.is_some());
    assert_eq!(row.version.as_deref(), Some(built.version.as_str()));
    let opened = fresh.run_view(id).unwrap().expect("the view");
    assert_eq!(decided(&opened), decided(&view));
    assert_eq!(opened.findings, before_write(&view));
    assert!(opened.stages.iter().all(|s| s.state == StageState::NotRun));
    assert_eq!(
        fresh.stage_files(id, StageName::Load),
        Err(AppError::NoFilesAtStage {
            stage: StageName::Load
        })
    );
    let export = fresh.export_project(id, &package.with_extension("reforge-project"));
    assert_eq!(export, Err(AppError::NoProjectOf { id }));
}

#[test]
fn the_stages_of_a_kind_without_inputs_are_the_librarys() {
    let bench = Bench::new();

    for kind in [RunKind::Repair, RunKind::Port] {
        assert_eq!(
            run::stages_of_kind(kind),
            run::stages_of(&bench.inputs, kind)
        );
    }
}

#[test]
fn a_package_opened_beside_its_run_compares_over_the_views() {
    let bench = Bench::new();
    let (run, view) = bench.kept(&artifact_mod(), RunKind::Port);
    let package = Utf8PathBuf::from(view.written.clone().expect("the package"));
    let opened = bench.store.open_package(&package).unwrap();

    let compared = bench
        .store
        .compare(run, opened)
        .unwrap()
        .expect("both are kept");

    assert!(compared.same_mod);
    let gone: Vec<&str> = compared
        .findings
        .gone
        .iter()
        .map(|f| f.stage.as_str())
        .collect();
    assert_eq!(gone, ["write"]);
    assert_eq!(compared.findings.added, []);
    assert!(compared.targets.iter().all(|target| !target.differs));
    let files = bench.store.compare_files(run, opened, StageName::Load);
    assert_eq!(
        files,
        Err(AppError::NoFilesAtStage {
            stage: StageName::Load
        })
    );
}

#[test]
fn a_package_opened_has_no_schema_build_where_its_run_has_the_schemas() {
    let game = skins::game();
    let sizes = game.sizes();
    let env = Env::builder()
        .game(game)
        .sizes(sizes)
        .schema(SchemaRows::new(300).build())
        .build()
        .unwrap();
    let bench = Bench {
        inputs: Inputs::of_env(env),
        ..Bench::new()
    };
    let (run, view) = bench.kept(&artifact_mod(), RunKind::Port);
    let package = Utf8PathBuf::from(view.written.clone().expect("the package"));
    let opened = bench.store.open_package(&package).unwrap();

    let compared = bench
        .store
        .compare(run, opened)
        .unwrap()
        .expect("both are kept");

    let schema = compared.inputs.iter().find(|d| d.key == "schema_build");
    let schema = schema.unwrap_or_else(|| panic!("{:?}", compared.inputs));
    assert_eq!((schema.a.as_str(), schema.b.as_str()), ("300", ""));
}

#[test]
fn a_package_opened_carries_the_fixups_its_run_turned_off() {
    let bench = Bench::new();
    let mut settings = RunSettings::default();
    settings.fixups.off.push(ARTIFACT_FIXUP.to_string());
    let (run, view) = bench.kept_with(&artifact_mod(), RunKind::Port, settings);
    let package = Utf8PathBuf::from(view.written.clone().expect("the package"));

    let opened = bench.store.open_package(&package).unwrap();

    let again = bench.store.run_again(opened).unwrap().expect("the row");
    assert_eq!(again.settings.fixups.off, [ARTIFACT_FIXUP]);
    assert_eq!(again.settings.fixups.on, Vec::<String>::new());
    let compared = bench
        .store
        .compare(run, opened)
        .unwrap()
        .expect("both are kept");
    let keys: Vec<&str> = compared.settings.iter().map(|d| d.key.as_str()).collect();
    assert!(!keys.contains(&"fixups_off"), "{:?}", compared.settings);
}

#[test]
fn a_package_opened_with_no_report_beside_it_reads_from_its_evidence() {
    let bench = Bench::new();
    let (view, fresh) = ported_and_fresh(&bench);
    let package = Utf8PathBuf::from(view.written.clone().expect("the package"));
    let stem = package.file_stem().unwrap();
    fs::remove_file(package.with_file_name(format!("{stem}.reforge-report.json"))).unwrap();

    let id = fresh.open_package(&package).unwrap();

    let opened = fresh.run_view(id).unwrap().expect("the view");
    assert_eq!(opened.findings, []);
    let targets = opened.targets.as_ref().expect("the evidence's units");
    let shipped: Vec<(String, u16, Option<u16>)> = targets
        .decisions
        .iter()
        .map(|d| (d.character_name.clone(), d.source, d.chosen))
        .collect();
    let ran = view
        .targets
        .as_ref()
        .expect("the decisions")
        .decisions
        .iter();
    let ran: Vec<(String, u16, Option<u16>)> = ran
        .filter(|d| d.chosen.is_some())
        .map(|d| (d.character_name.to_lowercase(), d.source, d.chosen))
        .collect();
    assert!(!ran.is_empty());
    assert_eq!(shipped, ran);
    let report: serde_json::Value =
        serde_json::from_str(&fresh.report(id).unwrap().expect("the report")).unwrap();
    assert_eq!(report["schema"], EVIDENCE_ONLY_SCHEMA);
    assert_eq!(report["evidence"]["run"], "port");
    let meta = opened.meta.as_ref().expect("the mod");
    assert_eq!(meta.file_name, package.file_name().unwrap());
}

#[test]
fn a_package_with_no_readme_is_refused() {
    let bench = Bench::new();
    let out = Bench::root_of(&bench.dir).join("bare");
    let config = ProjectConfig {
        name: "Bare".into(),
        display_name: Spelled::new("Bare"),
        version: "1.0.0".into(),
        description: Spelled::new("A package no run built."),
        authors: vec![Spelled::new("reforge tests")],
        champions: vec!["Test".into()],
    };
    let mut package = Package::new(config);
    let key = ChunkKey::new(
        WadName::parse(WAD).unwrap(),
        ChunkPath::parse("data/characters/test/skins/skin0.bin").unwrap(),
    );
    let bytes = ByteView::from(bin(vec![tool_artifact()]));
    package.insert(
        Layer::Base,
        key,
        Chunk::new(bytes, ChunkOrigin::Kept(FileId::new(0))),
    );
    let writer = Modpkg::new(&out);
    writer.write(&package, "{}").unwrap();
    let path = writer.path_of(&package);

    let refused = bench.store.open_package(&path);

    assert_eq!(
        refused,
        Err(AppError::NoEvidence {
            path: path.to_string()
        })
    );
    assert_eq!(bench.store.list().unwrap(), []);
}
