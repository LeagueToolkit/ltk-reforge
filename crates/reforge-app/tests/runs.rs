//! A kept run as the Runs page reads it: the files at each stage with their marks, sizes and
//! kinds. The game is `reforge_testkit::skins`.

use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_meta::BinObject;
use reforge::history::NoHistory;
use reforge::model::{BinHash, name_hash};
use reforge_app::error::AppError;
use reforge_app::inputs::Inputs;
use reforge_app::run::{self, RunKind, RunRequest, RunSettings, RunView, StageName, Unwatched};
use reforge_app::store::{AppStore, FileRow, Mark, StageFiles};
use reforge_testkit::ModArchive;
use reforge_testkit::bins::{bin, entry, float};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, WAD, skin_bin};
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
        let root = Self::root_of(&self.dir);
        let path = root.join("mod.fantome");
        fs::write(&path, archive.build()).unwrap();
        let request = RunRequest {
            kind,
            archive: path.to_string(),
            through: StageName::Write,
            settings: RunSettings::default(),
            out: Some(root.join("out").to_string()),
        };
        let view = run::run(&self.inputs, &request, Some(&self.store), &mut Unwatched).unwrap();
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
