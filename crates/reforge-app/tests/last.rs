//! The run page's last run, held until the next: its files at each stage and one file opened,
//! from its own layer when the store did not keep it and from the run tier when it did, a
//! bin's object asked for first. The game is `reforge_testkit::skins`.

use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_meta::BinObject;
use reforge::game::Speller;
use reforge::history::NoHistory;
use reforge::model::{BinHash, ContentId, name_hash};
use reforge::store::BlobStore;
use reforge_app::bin_rows::{BinDocument, BinReadError, BinViews, READ_PAGE, RowNode};
use reforge_app::error::AppError;
use reforge_app::inputs::Inputs;
use reforge_app::run::{
    self, HeldRun, RunKind, RunRequest, RunSettings, RunView, StageName, Unwatched,
};
use reforge_app::store::{AppStore, FileRow, FileView, StageFiles};
use reforge_testkit::ModArchive;
use reforge_testkit::bins::{bin, entry, float};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, WAD, skin_bin};
use tempfile::TempDir;

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

/// An object with a field a tool wrote and no build declared (#MOD-ARTIFACT-0020), which
/// `fix` removes, so the run writes the skin bin again and its bytes are the run's own.
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

    /// A port of [`artifact_mod`] through `through`, held.
    fn held(&self, through: StageName) -> (RunView, HeldRun) {
        let root = Self::root_of(&self.dir);
        let path = root.join("mod.fantome");
        fs::write(&path, artifact_mod().build()).unwrap();
        let request = RunRequest {
            kind: RunKind::Port,
            archive: path.to_string(),
            through,
            settings: RunSettings::default(),
            out: Some(root.join("out").to_string()),
            format: reforge_app::settings::PackageFormat::default(),
            keep: false,
        };
        let held = run::run_held(
            &self.inputs,
            &request,
            Some(&self.store),
            &mut Unwatched,
            None,
        );
        let (view, held) = held.unwrap();
        assert_eq!(view.fault, None);
        (view, held)
    }
}

/// The skin bin at a stage.
fn skin_at(files: &StageFiles) -> &FileRow {
    let skin = skin_bin("test", 0);
    let wad = files.wads.iter().find(|w| w.name == WAD);
    let wad = wad.unwrap_or_else(|| panic!("no WAD {WAD} at {:?}", files.stage));
    let found = wad
        .files
        .iter()
        .find(|f| f.path.as_deref() == Some(skin.as_str()));
    found.unwrap_or_else(|| panic!("no {skin} at {:?}: {:?}", files.stage, wad.files))
}

#[test]
fn a_run_the_store_did_not_keep_is_browsed_from_its_own_layer() {
    let bench = Bench::new();
    let (view, held) = bench.held(StageName::Check);
    assert_eq!(view.saved, None, "a run through check is not kept");

    let fix = held.files(Some(&bench.store), StageName::Fix).unwrap();
    let fix = fix.expect("the run ended load");
    let fixed = skin_at(&fix);
    assert_eq!(fixed.kind, "property-bin");
    let content = ContentId::parse(&fixed.content).unwrap();

    let shown = held.file(Some(&bench.store), content);
    assert_eq!(shown, Ok(FileView::Bin));
    assert_eq!(
        run::held_file(None, Some(&bench.store), content),
        Err(AppError::BlobMissing { id: content.hex() }),
        "the bytes `fix` wrote are the held run's alone"
    );
}

#[test]
fn a_kept_run_held_reads_its_bytes_from_the_run_tier() {
    let bench = Bench::new();
    let (view, held) = bench.held(StageName::Write);
    let id = view.saved.expect("a written run is kept");
    assert!(held.layer().is_empty(), "the store took the layer");

    for stage in [StageName::Load, StageName::Fix, StageName::Build] {
        let from_held = held.files(Some(&bench.store), stage).unwrap();
        let from_store = bench.store.stage_files(id, stage).unwrap();
        assert_eq!(from_held, from_store, "{stage:?}");
    }
    // A port patches the skin bin and ships the graph's bin as a file of its own.
    let build = held.files(Some(&bench.store), StageName::Build).unwrap();
    let rows = build
        .iter()
        .flat_map(|files| &files.wads)
        .flat_map(|w| &w.files);
    let built = rows
        .filter(|row| row.kind == "property-bin")
        .collect::<Vec<_>>();
    assert!(!built.is_empty(), "{build:?}");
    for row in built {
        assert!(
            bench
                .store
                .tier()
                .contains(ContentId::parse(&row.content).unwrap())
        );
    }
}

#[test]
fn a_held_run_refuses_a_stage_that_names_no_files() {
    let bench = Bench::new();
    let (_, held) = bench.held(StageName::Check);

    for stage in [StageName::Read, StageName::Facts, StageName::Check] {
        let refused = held.files(Some(&bench.store), stage);
        assert_eq!(refused, Err(AppError::NoFilesAtStage { stage }));
    }
}

/// The skin bin at `fix` read from a held run: what `ask` reads of it.
fn fixed_skin<T>(
    bench: &Bench,
    held: &HeldRun,
    ask: impl FnOnce(&BinDocument, &Speller<'_>) -> Result<T, AppError>,
) -> T {
    let fix = held.files(Some(&bench.store), StageName::Fix).unwrap();
    let fix = fix.expect("the run ended load");
    let content = ContentId::parse(&skin_at(&fix).content).unwrap();
    let views = BinViews::default();
    let source = held.bin_source(Some(&bench.store));
    source.read(&views, content, None, ask).unwrap()
}

#[test]
fn a_bin_the_held_run_fixed_opens_to_a_row_per_object() {
    let bench = Bench::new();
    let (_, held) = bench.held(StageName::Check);
    let tint = tool_artifact().path_hash;

    let handle = fixed_skin(
        &bench,
        &held,
        |document, speller| Ok(document.open(speller)),
    );

    let entries: Vec<&str> = handle.rows.iter().map(|row| row.entry.as_str()).collect();
    assert!(
        entries.contains(&&*format!("0x{:08x}", tint.0)),
        "{entries:?}"
    );
    assert!(handle.rows.iter().all(|row| row.node == RowNode::Object));
}

#[test]
fn an_object_the_bin_lacks_is_refused_by_its_address() {
    let bench = Bench::new();
    let (_, held) = bench.held(StageName::Check);
    let missing = BinHash(0x1234_5678);

    let refused = fixed_skin(&bench, &held, |document, speller| {
        Ok(document.children(missing, "", 0, READ_PAGE, speller))
    });

    assert_eq!(
        refused,
        Err(BinReadError::NodeNotFound("0x12345678:".into()))
    );
}
