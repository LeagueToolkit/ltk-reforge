//! A mod explored as the Mod page reads it: a port's run through `load` over the store's tiers,
//! its load report rendered, its files at `load` with their provenance, one file opened, and
//! the last mod explored held until the next. The game is `reforge_testkit::skins`.

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::history::NoHistory;
use reforge::model::ContentId;
use reforge_app::explore::{self, ExploredMod};
use reforge_app::inputs::Inputs;
use reforge_app::run::RunKind;
use reforge_app::state::Explored;
use reforge_app::store::{AppStore, FileView, StageFiles};
use reforge_testkit::ModArchive;
use reforge_testkit::bins::{bin, blob, entry};
use reforge_testkit::facts::{env, own_files, own_graph, own_skin};
use reforge_testkit::skins::{self, BASE_TEX, WAD, pet_skin, skin_bin};
use tempfile::TempDir;

const SKIN0: &str = "Characters/Test/Skins/Skin0";
const GRAPH0: &str = "Characters/Test/Animations/Skin0";
const RESOURCES0: &str = "Characters/Test/Skins/Skin0/Resources";

/// A mod of two characters in the champion's WAD: the champion's own rig at skin0, the
/// sub-character's skin0, and the game's base texture at its own path.
fn two_characters() -> ModArchive {
    let objects = vec![
        entry(SKIN0, &own_skin("a", GRAPH0, RESOURCES0)),
        own_graph("a", GRAPH0),
    ];
    let pet = vec![entry("Characters/TestPet/Skins/Skin0", &pet_skin())];
    let archive = own_files("a")
        .into_iter()
        .fold(ModArchive::new(), |archive, (path, bytes)| {
            archive.file(WAD, &path, bytes)
        });
    archive
        .file(WAD, &skin_bin("test", 0), bin(objects))
        .file(WAD, &skin_bin("testpet", 0), bin(pet))
        .file(WAD, BASE_TEX, blob(BASE_TEX))
}

/// A store, the inputs over the testkit's game, and a folder for archives.
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

    fn written(&self, name: &str, bytes: &[u8]) -> Utf8PathBuf {
        let path = Self::root_of(&self.dir).join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn explore(&self, name: &str, archive: &ModArchive) -> ExploredMod {
        let path = self.written(name, &archive.build());
        explore::explore(&self.inputs, &path, Some(&self.store))
    }
}

/// The file at a path of the files' WAD.
fn file_at<'a>(files: &'a StageFiles, path: &str) -> &'a reforge_app::store::FileRow {
    let wad = files
        .wads
        .iter()
        .find(|wad| wad.name == WAD)
        .expect("the champion's WAD");
    let found = wad.files.iter().find(|f| f.path.as_deref() == Some(path));
    found.unwrap_or_else(|| panic!("no {path}: {:?}", wad.files))
}

#[test]
fn a_mod_of_two_characters_lists_its_wads_characters_units_and_kinds() {
    let bench = Bench::new();
    let explored = bench.explore("two.fantome", &two_characters());
    let view = explored.view();
    assert_eq!(view.halted, None);
    assert_eq!(view.fault, None);
    assert_eq!(
        view.meta.as_ref().map(|meta| meta.file_name.as_str()),
        Some("two.fantome")
    );

    assert_eq!(view.wads.len(), 1);
    let wad = &view.wads[0];
    assert_eq!((wad.name.as_str(), wad.role.as_str()), (WAD, "champion"));
    assert_eq!((wad.files, wad.named), (8, 8));

    let characters: Vec<_> = view
        .characters
        .iter()
        .map(|c| (c.name.as_str(), c.kind.as_str(), c.wad.as_str(), c.units))
        .collect();
    assert_eq!(
        characters,
        [("test", "champion", WAD, 1), ("testpet", "sub", WAD, 1)]
    );

    let units: Vec<_> = view
        .units
        .iter()
        .map(|u| (u.character_name.as_str(), u.slot, u.file.clone(), u.loads))
        .collect();
    assert_eq!(
        units,
        [
            ("test", 0, skin_bin("test", 0), 1),
            ("testpet", 0, skin_bin("testpet", 0), 1),
        ]
    );

    let kinds: Vec<_> = view
        .kinds
        .iter()
        .map(|k| (k.kind.as_str(), k.files))
        .collect();
    assert_eq!(
        kinds,
        [("property-bin", 2), ("text", 6)],
        "in the kind's order"
    );
    let dating = view.dating.as_ref().expect("load ended");
    assert_eq!(dating.not_before, None, "no history dates it");
}

#[test]
fn the_files_at_load_carry_the_provenance_the_port_gave() {
    let bench = Bench::new();
    let explored = bench.explore("two.fantome", &two_characters());
    let sources: Vec<_> = (explored.view().sources.iter())
        .map(|s| (s.fact.as_str(), s.files))
        .collect();
    assert_eq!(sources, [("install", 1), ("own", 7)], "in the facts' order");

    let files = explored.files(Some(&bench.store)).expect("load ended");
    let texture = file_at(&files, BASE_TEX);
    assert_eq!(texture.source.as_deref(), Some("install"));
    let skin = file_at(&files, &skin_bin("test", 0));
    assert_eq!(skin.source.as_deref(), Some("own"));
    assert_eq!(skin.kind, "property-bin");
    assert!(skin.size > 0, "the ingest tier holds its bytes");
}

#[test]
fn the_skin_bin_of_a_mod_explored_opens_as_a_bin() {
    let bench = Bench::new();
    let explored = bench.explore("two.fantome", &two_characters());
    let files = explored.files(Some(&bench.store)).unwrap();
    let skin = file_at(&files, &skin_bin("test", 0));
    let content = ContentId::parse(&skin.content).unwrap();
    let shown = explore::explored_file(Some(&explored), Some(&bench.store), content, None);
    match shown.unwrap() {
        FileView::Bin { text, more } => {
            assert!(text.contains("SkinCharacterDataProperties"), "{text}");
            assert_eq!(more, 0);
        }
        other => panic!("shown as {other:?}"),
    }
}

#[test]
fn an_explore_keeps_nothing_in_the_stores_rows_and_ingests_the_archive() {
    let bench = Bench::new();
    let explored = bench.explore("two.fantome", &two_characters());
    assert!(bench.store.list().unwrap().is_empty());
    assert_eq!(bench.store.ingest().entries().len(), 1);
    let again = explored.run_again();
    assert_eq!(again.kind, RunKind::Port);
    assert!(again.archive.ends_with("two.fantome"));
    assert!(again.archive_present);

    // A second explore of the same archive starts from the raw snapshot the first ingested.
    let second = bench.explore("two.fantome", &two_characters());
    assert_eq!(second.view().wads, explored.view().wads);
    assert_eq!(bench.store.ingest().entries().len(), 1);
}

#[test]
fn a_mod_that_does_not_read_answers_a_view_halted_with_no_wads() {
    let bench = Bench::new();
    let path = bench.written("not.fantome", b"not a zip");
    let explored = explore::explore(&bench.inputs, &path, Some(&bench.store));
    let view = explored.view();
    let halted = view.halted.expect("read halted");
    assert!(
        view.findings
            .iter()
            .any(|f| f.id == halted && f.severity == reforge_app::view::Severity::Fatal)
    );
    assert!(view.wads.is_empty());
    assert!(view.dating.is_none());
    assert!(explored.raw().is_none());
    assert!(explored.files(Some(&bench.store)).is_none());
}

#[test]
fn exploring_a_second_mod_replaces_the_first() {
    let bench = Bench::new();
    let held = Explored::default();
    held.set(bench.explore("two.fantome", &two_characters()));
    let one = ModArchive::new().file(WAD, &skin_bin("test", 0), bin(vec![]));
    held.set(bench.explore("one.fantome", &one));
    let name = held.with(|explored| explored.map(|e| e.view().archive.clone()));
    assert!(name.is_some_and(|name| name.ends_with("one.fantome")));
}
