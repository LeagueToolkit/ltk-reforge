//! What the window asks of a run apart from its view (#ADR-0103): hashes spelled with the run's
//! names, and the places of the values the type check counts, read again from a kept run and
//! from the run page's last run. The game is `reforge_testkit::skins` at a build with a schema.

use std::sync::Arc;

use camino::Utf8PathBuf;
use fs_err as fs;
use ltk_meta::property::values::Map;
use ltk_meta::{PropertyKind, PropertyValueEnum};
use reforge::env::Env;
use reforge::game::NameBook;
use reforge::model::{name_hash, path_hash};
use reforge_app::error::AppError;
use reforge_app::inputs::Inputs;
use reforge_app::places::{self, Held, MAX_SPELLED, TypePlacesView};
use reforge_app::run::{self, HeldRun, RunKind, RunRequest, RunSettings, StageName, Unwatched};
use reforge_app::store::AppStore;
use reforge_testkit::bins::{float, object, pointer, text};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::schema::{field, kind, map_of};
use reforge_testkit::skins::{self, WAD, skin_bin};
use reforge_testkit::{ModArchive, SchemaRows};
use tempfile::TempDir;

/// A mod whose skin bin holds a material with a float where the target build declares a byte.
fn mistyped_mod() -> ModArchive {
    skin_archive(vec![object("A", "Material", vec![("count", float(2.0))])])
}

/// A store, the inputs over the testkit's game at a build whose schema declares the material,
/// and a folder for archives.
struct Bench {
    dir: TempDir,
    inputs: Inputs,
    store: AppStore,
}

impl Bench {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let schema = SchemaRows::new(300)
            .class(
                "Material",
                &[
                    field("count", kind("U8")),
                    field("parts", map_of("String", "Part")),
                ],
            )
            .class("Part", &[field("count", kind("U8"))])
            .build();
        let env = Env::builder()
            .game(skins::game_at("16.19.300"))
            .schema(schema)
            .build()
            .unwrap();
        Self {
            store: AppStore::open(&root.join("store")).unwrap(),
            inputs: Inputs::of_env(env),
            dir,
        }
    }

    /// A repair of [`mistyped_mod`] through `check`, held and kept on request.
    fn held(&self) -> (Option<u64>, HeldRun) {
        self.held_of(&mistyped_mod())
    }

    /// A repair of a mod with one mismatched row through `check`, held and kept on request.
    fn held_of(&self, archive: &ModArchive) -> (Option<u64>, HeldRun) {
        let path = Utf8PathBuf::from_path_buf(self.dir.path().join("mod.fantome")).unwrap();
        fs::write(&path, archive.build()).unwrap();
        let request = RunRequest {
            kind: RunKind::Repair,
            archive: path.to_string(),
            through: StageName::Check,
            settings: RunSettings::default(),
            out: None,
            keep: true,
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
        let found = view
            .findings
            .iter()
            .filter(|f| f.code == "check.type-mismatch");
        assert_eq!(found.count(), 1, "{:?}", view.findings);
        (view.saved, held)
    }
}

/// The one place of the one mismatched row.
fn only_place(places: &TypePlacesView) -> (&str, &str, &str, &str) {
    assert_eq!(places.mismatched.len(), 1, "{places:?}");
    assert!(places.undeclared.is_empty() && places.classes.is_empty());
    let row = &places.mismatched[0].places;
    assert_eq!(row.len(), 1, "{row:?}");
    let place = &row[0];
    (&place.wad, &place.path, &place.entry, &place.at)
}

#[test]
fn the_last_run_gives_the_place_of_a_value_of_another_type() {
    let bench = Bench::new();
    let (_, held) = bench.held();
    let snapshots = held.snapshots().cloned().expect("the run ended load");
    let request = held.request();

    let places = places::type_places(
        &bench.inputs,
        Some(&bench.store),
        Held {
            kind: request.kind,
            settings: &request.settings,
            snapshots,
            layer: Some(Arc::clone(held.layer())),
        },
    )
    .unwrap();

    let count = format!("{:08x}", name_hash("count").0);
    let entry = format!("{:08x}", name_hash("A").0);
    let skin = skin_bin("test", 0);
    assert_eq!(
        only_place(&places),
        (WAD, skin.as_str(), entry.as_str(), count.as_str())
    );
}

#[test]
fn a_string_map_key_reaches_the_window_cleaned_and_written_as_json() {
    let bench = Bench::new();
    // A key that holds each character the path's form uses, and a bidirectional control.
    let key = "a.b]c}\"d\u{202e}";
    let part = (text(key), pointer("Part", vec![("count", float(2.0))]));
    let parts = Map::new(PropertyKind::String, PropertyKind::Struct, vec![part]).unwrap();
    let material = object(
        "A",
        "Material",
        vec![("parts", PropertyValueEnum::Map(parts))],
    );
    let (_, held) = bench.held_of(&skin_archive(vec![material]));
    let request = held.request();

    let places = places::type_places(
        &bench.inputs,
        Some(&bench.store),
        Held {
            kind: request.kind,
            settings: &request.settings,
            snapshots: held.snapshots().cloned().expect("the run ended load"),
            layer: Some(Arc::clone(held.layer())),
        },
    )
    .unwrap();

    let parts = format!("{:08x}", name_hash("parts").0);
    let count = format!("{:08x}", name_hash("count").0);
    let at = format!("{parts}{{\"a.b]c}}\\\"d\\u{{202e}}\"}}.{count}");
    assert_eq!(only_place(&places).3, at);
}

#[test]
fn a_kept_run_gives_the_same_places_from_the_run_tier() {
    let bench = Bench::new();
    let (saved, held) = bench.held();
    let id = saved.expect("a run kept on request at its last stage");
    let kept = bench.store.kept_run(id).unwrap().expect("the run is kept");
    let again = bench.store.run_again(id).unwrap().expect("the run is kept");

    let from_store = places::type_places(
        &bench.inputs,
        Some(&bench.store),
        Held {
            kind: again.kind,
            settings: &again.settings,
            snapshots: kept.snapshots,
            layer: None,
        },
    )
    .unwrap();
    let from_held = places::type_places(
        &bench.inputs,
        Some(&bench.store),
        Held {
            kind: held.request().kind,
            settings: &held.request().settings,
            snapshots: held.snapshots().cloned().unwrap(),
            layer: Some(Arc::clone(held.layer())),
        },
    )
    .unwrap();

    assert_eq!(from_store, from_held);
}

#[test]
fn a_run_that_did_not_end_build_holds_no_package() {
    let bench = Bench::new();
    let (_, held) = bench.held();
    let mut snapshots = held.snapshots().cloned().unwrap();
    snapshots.build = None;

    let refused = places::type_places(
        &bench.inputs,
        Some(&bench.store),
        Held {
            kind: RunKind::Repair,
            settings: &RunSettings::default(),
            snapshots,
            layer: None,
        },
    );

    assert_eq!(refused, Err(AppError::NoPackageHeld));
}

#[test]
fn a_hash_is_spelled_by_its_length_and_one_no_name_spells_is_left_out() {
    let bench = Bench::new();
    let (_, held) = bench.held();
    let book = &held.snapshots().unwrap().load.names().clone();
    let skin = skin_bin("test", 0);
    let path = format!("{:016x}", path_hash(&skin).0);
    let unknown = "0123456789abcdef".to_string();
    let asked = [
        path.clone(),
        unknown,
        "not hex!".to_string(),
        "abc".to_string(),
    ];

    let spelled = places::spell(&asked, book, None).unwrap();

    assert_eq!(spelled.into_iter().collect::<Vec<_>>(), [(path, skin)]);
}

#[test]
fn a_list_of_hashes_past_the_bound_is_refused() {
    let asked = vec!["00000000".to_string(); MAX_SPELLED + 1];

    let refused = places::spell(&asked, &NameBook::default(), None);

    assert!(
        matches!(refused, Err(AppError::Internal { .. })),
        "{refused:?}"
    );
}
