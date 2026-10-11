//! A kept run as the Runs page reads it: the files at each stage with their marks, sizes and
//! kinds, one file opened by its kind, and two runs compared. The game is
//! `reforge_testkit::skins`.

use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_meta::BinObject;
use reforge::history::NoHistory;
use reforge::model::{BinHash, ByteView, ContentId, name_hash};
use reforge::store::BlobStore;
use reforge_app::error::AppError;
use reforge_app::inputs::Inputs;
use reforge_app::run::{self, RunKind, RunRequest, RunSettings, RunView, StageName, Unwatched};
use reforge_app::store::{AppStore, Counts, FileRow, FileView, HEX_BYTES, Mark, StageFiles};
use reforge_testkit::ModArchive;
use reforge_testkit::bins::{bin, entry, float};
use reforge_testkit::facts::{archive, env, own_files, own_graph, own_skin};
use reforge_testkit::packs::{DdsPixels, dds};
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
