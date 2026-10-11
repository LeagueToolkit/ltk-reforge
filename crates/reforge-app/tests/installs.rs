//! The installs listed: the Riot client's list read, each path listed once, an added folder
//! checked to be an install, and the order the list shows.

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use reforge_app::error::AppError;
use reforge_app::installs::{
    self, Found, InstallSource, InstallView, normalise, riot_client_installs,
};
use reforge_app::settings::{Settings, SettingsStore};
use tempfile::TempDir;

fn temp() -> (TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
        .expect("the temporary directory is UTF-8");
    (dir, root)
}

/// A folder laid out as the Riot client lays an install out, with no data in it.
fn client_folder(root: &Utf8Path) {
    let game = root.join("Game");
    fs::create_dir_all(&game).unwrap();
    fs::write(game.join("League of Legends.exe"), b"").unwrap();
}

/// An install that opens, at a build.
fn install(root: &Utf8Path, version: &str) {
    fs::create_dir_all(root.join("Game/DATA/FINAL/Champions")).unwrap();
    fs::write(
        root.join("Game/content-metadata.json"),
        serde_json::json!({ "version": version }).to_string(),
    )
    .unwrap();
}

/// The path as the Riot client writes it: forward slashes and a trailing one.
fn as_client_writes(path: &Utf8Path) -> String {
    format!("{}/", path.as_str().replace('\\', "/"))
}

fn view(path: &str, source: InstallSource) -> InstallView {
    InstallView {
        path: path.to_string(),
        source,
        build: None,
        readable: true,
        chosen: false,
    }
}

#[test]
fn the_riot_clients_list_gives_its_live_and_pbe_installs_with_a_game() {
    let (_dir, root) = temp();
    let live = root.join("Riot Games").join("League of Legends");
    let pbe = root.join("Riot Games").join("League of Legends (PBE)");
    let empty = root.join("Old").join("League of Legends");
    let other = root.join("Riot Games").join("VALORANT").join("live");
    client_folder(&live);
    client_folder(&pbe);
    fs::create_dir_all(&empty).unwrap();
    client_folder(&other);

    let mut clients = serde_json::Map::new();
    for path in [&live, &pbe, &empty, &other] {
        clients.insert(
            as_client_writes(path),
            serde_json::json!("RiotClientServices"),
        );
    }
    let file = root.join("RiotClientInstalls.json");
    fs::write(
        &file,
        serde_json::json!({ "associated_client": clients }).to_string(),
    )
    .unwrap();

    let mut found = riot_client_installs(&file);
    found.sort_by_key(|found| found.source);
    assert_eq!(
        found,
        [
            Found {
                path: normalise(live.as_str()),
                source: InstallSource::Live
            },
            Found {
                path: normalise(pbe.as_str()),
                source: InstallSource::Pbe
            },
        ]
    );
}

#[test]
fn a_riot_client_list_that_does_not_read_lists_none() {
    let (_dir, root) = temp();
    let file = root.join("RiotClientInstalls.json");
    assert!(riot_client_installs(&file).is_empty());
    fs::write(&file, "{ not json").unwrap();
    assert!(riot_client_installs(&file).is_empty());
}

#[test]
fn a_path_found_twice_is_listed_once_with_its_detected_source() {
    let (_dir, root) = temp();
    let live = normalise(root.join("League of Legends").as_str());
    let written = as_client_writes(&live);
    let written = if cfg!(windows) {
        written.to_uppercase()
    } else {
        written
    };
    let settings = Settings {
        game: Some(written.clone()),
        installs: vec![written],
        ..Settings::default()
    };

    let listed = installs::list(vec![view(live.as_str(), InstallSource::Live)], &settings);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].path, live.as_str());
    assert_eq!(listed[0].source, InstallSource::Live);
    assert!(listed[0].chosen);
}

#[test]
fn a_chosen_install_no_source_lists_is_listed_as_added() {
    let settings = Settings {
        game: Some("D:/Games/League".to_string()),
        ..Settings::default()
    };
    let listed = installs::list(Vec::new(), &settings);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].source, InstallSource::Added);
    assert!(listed[0].chosen);
    assert!(!listed[0].readable);
}

#[test]
fn the_list_shows_the_chosen_install_first_then_by_source_then_by_path() {
    let detected = vec![
        view("/b/running", InstallSource::Running),
        view("/b/pbe", InstallSource::Pbe),
        view("/b/live", InstallSource::Live),
        view("/a/live", InstallSource::Live),
    ];
    let settings = Settings {
        game: Some("/z/added".to_string()),
        installs: vec!["/y/added".to_string(), "/z/added".to_string()],
        ..Settings::default()
    };
    let order: Vec<_> = installs::list(detected, &settings)
        .into_iter()
        .map(|view| (view.source, view.chosen))
        .collect();
    use InstallSource::*;
    assert_eq!(
        order,
        [
            (Added, true),
            (Live, false),
            (Live, false),
            (Pbe, false),
            (Running, false),
            (Added, false),
        ]
    );
    let paths: Vec<_> = installs::list(Vec::new(), &settings)
        .into_iter()
        .map(|view| view.path)
        .collect();
    assert_eq!(
        paths,
        [
            normalise("/z/added").as_str(),
            normalise("/y/added").as_str()
        ]
    );
}

#[test]
fn an_added_folder_that_is_no_install_is_refused() {
    let (_dir, root) = temp();
    let store = SettingsStore::open(root.join("settings.json"));
    let folder = root.join("not an install");
    fs::create_dir_all(&folder).unwrap();

    let error = installs::add(&store, Vec::new(), folder.as_str()).unwrap_err();
    assert!(
        matches!(&error, AppError::GameUnreadable { path, .. } if *path == normalise(folder.as_str())),
        "{error}"
    );
    assert_eq!(store.get(), Settings::default());
}

#[test]
fn an_added_install_is_kept_and_chosen_when_none_is() {
    let (_dir, root) = temp();
    let store = SettingsStore::open(root.join("settings.json"));
    let first = root.join("first");
    let second = root.join("second");
    install(
        &first,
        "16.19.8207193+branch.releases-16-19.content.release",
    );
    install(&second, "16.20.1");

    let listed = installs::add(&store, Vec::new(), first.as_str()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].build.as_deref(), Some("16.19.8207193"));
    assert!(listed[0].readable && listed[0].chosen);

    let listed = installs::add(&store, Vec::new(), second.as_str()).unwrap();
    let chosen: Vec<_> = listed.iter().map(|view| view.chosen).collect();
    assert_eq!(chosen, [true, false]);
    assert_eq!(
        store.get().game.as_deref(),
        Some(normalise(first.as_str()).as_str())
    );
    assert_eq!(store.get().installs.len(), 2);
}

#[test]
fn removing_the_chosen_install_chooses_the_first_of_the_list() {
    let (_dir, root) = temp();
    let store = SettingsStore::open(root.join("settings.json"));
    let added = root.join("added");
    install(&added, "16.19.1");
    let live = normalise(root.join("League of Legends").as_str());
    let detected = || vec![view(live.as_str(), InstallSource::Live)];

    installs::add(&store, detected(), added.as_str()).unwrap();
    installs::choose(&store, detected(), added.as_str()).unwrap();
    assert_eq!(
        store.get().game.as_deref(),
        Some(normalise(added.as_str()).as_str())
    );

    // A detected install stays.
    let listed = installs::remove(&store, detected(), live.as_str()).unwrap();
    assert_eq!(listed.len(), 2);

    let listed = installs::remove(&store, detected(), added.as_str()).unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].chosen);
    assert_eq!(store.get().game.as_deref(), Some(live.as_str()));
    assert!(store.get().installs.is_empty());
}

#[test]
fn a_path_the_list_does_not_hold_is_not_chosen() {
    let (_dir, root) = temp();
    let store = SettingsStore::open(root.join("settings.json"));
    let error = installs::choose(&store, Vec::new(), "/elsewhere").unwrap_err();
    assert!(matches!(error, AppError::GameUnreadable { .. }), "{error}");
    assert_eq!(store.get().game, None);
}
