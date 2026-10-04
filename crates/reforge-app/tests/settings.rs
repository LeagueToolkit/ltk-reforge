use fs_err as fs;
use reforge_app::settings::{Settings, SettingsStore, Theme};

#[test]
fn an_absent_file_gives_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(dir.path().join("settings.json"));
    assert_eq!(store.get(), Settings::default());
}

#[test]
fn a_saved_setting_reads_back_in_a_new_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("settings.json");
    let settings = Settings {
        theme: Theme::Light,
    };

    SettingsStore::open(&path).save(settings.clone()).unwrap();

    assert_eq!(SettingsStore::open(&path).get(), settings);
}

#[test]
fn a_setting_is_saved_under_its_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let settings = Settings { theme: Theme::Dark };

    SettingsStore::open(&path).save(settings).unwrap();

    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved, serde_json::json!({ "appearance.theme": "dark" }));
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
    fs::write(&path, "{}").unwrap();

    assert_eq!(SettingsStore::open(&path).get(), Settings::default());
}
