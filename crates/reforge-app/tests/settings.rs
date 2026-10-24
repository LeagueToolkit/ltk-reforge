use fs_err as fs;
use reforge_app::settings::{PackageFormat, Settings, SettingsStore, Theme};

#[test]
fn an_absent_file_gives_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(dir.path().join("settings.json"));
    assert_eq!(store.get(), Settings::default());
}

/// #ADR-0105: a package is a `.fantome` until a person chooses otherwise.
#[test]
fn the_package_format_is_the_fantome_by_default_and_is_saved_under_its_id() {
    assert_eq!(Settings::default().format, PackageFormat::Fantome);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let settings = Settings {
        format: PackageFormat::Modpkg,
        ..Settings::default()
    };
    SettingsStore::open(&path).save(settings.clone()).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["output.format"], "modpkg");
    assert_eq!(SettingsStore::open(&path).get(), settings);
}

#[test]
fn a_saved_setting_reads_back_in_a_new_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("settings.json");
    let settings = Settings {
        theme: Theme::Light,
        game: Some("C:/Riot Games/League of Legends".to_string()),
        ..Settings::default()
    };

    SettingsStore::open(&path).save(settings.clone()).unwrap();

    assert_eq!(SettingsStore::open(&path).get(), settings);
}

#[test]
fn a_setting_is_saved_under_its_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let settings = Settings {
        theme: Theme::Dark,
        history: Some("D:/index".to_string()),
        ..Settings::default()
    };

    SettingsStore::open(&path).save(settings).unwrap();

    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["appearance.theme"], "dark");
    assert_eq!(saved["inputs.history"], "D:/index");
    assert_eq!(saved["inputs.game"], serde_json::Value::Null);
}

#[test]
fn a_file_that_does_not_read_gives_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, "{ not json").unwrap();

    assert_eq!(SettingsStore::open(&path).get(), Settings::default());
}

#[test]
fn a_setting_the_file_lacks_takes_its_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, r#"{ "appearance.theme": "light" }"#).unwrap();

    let settings = SettingsStore::open(&path).get();
    assert_eq!(settings.theme, Theme::Light);
    assert_eq!(settings.game, None);
}
