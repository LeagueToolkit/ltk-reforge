//! LTK Manager's library read for the Batch page (#ADR-0106), over a data folder built as the
//! manager lays one out.

use std::io::Write as _;

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::model::clean;
use reforge_app::error::AppError;
use reforge_app::manager::{self, ManagerFormat, ManagerMod};
use reforge_testkit::fixes::skin_archive;
use reforge_testkit::skins::{self, skin_bin};
use tempfile::TempDir;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// The manager's data folder, removed with the test.
struct Folder {
    _dir: TempDir,
    root: Utf8PathBuf,
}

impl Folder {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        fs::create_dir_all(root.join("mods")).unwrap();
        Self { _dir: dir, root }
    }

    /// One mod's entry in `library.json`, and its folder with a config when `display` says.
    fn entry(&self, slug: &str, format: &str, storage: &str, display: Option<&str>) -> String {
        if let Some(display) = display {
            let dir = self.root.join("mods").join(slug);
            fs::create_dir_all(&dir).unwrap();
            let config = serde_json::json!({
                "name": slug,
                "display_name": display,
                "version": "2.1.0",
                "authors": ["someone"],
            });
            fs::write(dir.join("mod.config.json"), config.to_string()).unwrap();
        }
        serde_json::json!({
            "id": format!("id-{slug}"),
            "installedAt": "2026-10-24T19:08:46Z",
            "format": format,
            "storage": storage,
            "slug": slug,
            "sourceSha256": "ab".repeat(32),
        })
        .to_string()
    }

    fn index(&self, entries: &[String]) {
        let text = format!("{{\"version\": 2, \"mods\": [{}]}}", entries.join(","));
        fs::write(self.root.join("library.json"), text).unwrap();
    }

    /// The manager's report of one mod: the champions and tags it derived.
    fn report(&self, slug: &str, champions: &[&str], tags: &[&str]) {
        let report = serde_json::json!({
            "version": 2,
            "reports": {
                format!("id-{slug}"): {
                    "modId": format!("id-{slug}"),
                    "derived": { "champions": champions, "maps": [], "tags": tags },
                },
            },
        });
        fs::write(self.root.join("wad-reports.json"), report.to_string()).unwrap();
    }
}

/// A mod as the reader lists one with no report of it.
fn unreported(
    slug: &str,
    name: &str,
    version: Option<&str>,
    archive: Option<String>,
) -> ManagerMod {
    ManagerMod {
        slug: slug.into(),
        name: name.into(),
        version: version.map(String::from),
        format: ManagerFormat::Fantome,
        archive,
        extended: false,
        installed_at: "2026-10-24T19:08:46Z".into(),
        champions: Vec::new(),
        maps: Vec::new(),
        tags: Vec::new(),
    }
}

/// A plain Fantome, as the testkit builds one.
fn plain() -> Vec<u8> {
    skin_archive(vec![]).build()
}

/// A Fantome with a layer directory, as league-mod packs one (#MOD-ARTIFACT-0040).
fn extended() -> Vec<u8> {
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    zip.start_file("META/info.json", options).unwrap();
    zip.write_all(br#"{"Name":"x","Author":"y","Version":"1.0.0","Description":""}"#)
        .unwrap();
    zip.start_file(
        format!("WAD/{}/{}", skins::WAD, skin_bin("test", 0)),
        options,
    )
    .unwrap();
    zip.write_all(b"").unwrap();
    zip.start_file(
        format!("WAD_pink/{}/{}", skins::WAD, skin_bin("test", 0)),
        options,
    )
    .unwrap();
    zip.write_all(b"").unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn the_library_lists_each_mod_with_its_archive_and_whether_it_is_extended() {
    let folder = Folder::new();
    fs::write(folder.root.join("mods").join("plain-one.fantome"), plain()).unwrap();
    fs::write(folder.root.join("mods").join("layered.fantome"), extended()).unwrap();
    fs::create_dir_all(folder.root.join("archives")).unwrap();
    fs::write(
        folder.root.join("archives").join("id-older.fantome"),
        plain(),
    )
    .unwrap();
    folder.index(&[
        folder.entry(
            "plain-one",
            "fantome",
            "archive",
            Some("Plain \u{1b}[31mOne"),
        ),
        folder.entry("layered", "fantome", "archive", Some("Layered")),
        folder.entry("unpacked", "fantome", "project", Some("Unpacked")),
        folder.entry("older", "fantome", "archive", None),
        folder.entry("packed", "modpkg", "archive", Some("Packed")),
    ]);
    folder.report("plain-one", &["Vex", "Jan\u{1b}na"], &["champion-skin"]);

    let library = manager::read_library(&folder.root).unwrap();

    assert_eq!(library.folder, folder.root.to_string());
    let at = |name: &str| folder.root.join("mods").join(name).to_string();
    let older = folder.root.join("archives").join("id-older.fantome");
    let expected = vec![
        ManagerMod {
            // The manager's text, cleaned: the escape is spelled out, not dropped.
            champions: vec!["Vex".into(), clean("Jan\u{1b}na").into_owned()],
            tags: vec!["champion-skin".into()],
            ..unreported(
                "plain-one",
                &clean("Plain \u{1b}[31mOne"),
                Some("2.1.0"),
                Some(at("plain-one.fantome")),
            )
        },
        ManagerMod {
            extended: true,
            ..unreported(
                "layered",
                "Layered",
                Some("2.1.0"),
                Some(at("layered.fantome")),
            )
        },
        unreported("unpacked", "Unpacked", Some("2.1.0"), None),
        unreported("older", "older", None, Some(older.to_string())),
        ManagerMod {
            format: ManagerFormat::Modpkg,
            // The archive is not where the layout puts it.
            ..unreported("packed", "Packed", Some("2.1.0"), None)
        },
    ];
    assert_eq!(library.mods, expected);
}

#[test]
fn a_folder_with_no_index_is_no_library() {
    let folder = Folder::new();
    let Err(AppError::ManagerAbsent { path }) = manager::read_library(&folder.root) else {
        panic!("a folder with no index read");
    };
    assert_eq!(path, folder.root.to_string());
}

#[test]
fn an_index_that_does_not_read_is_refused_and_one_of_another_shape_lists_what_it_can() {
    let folder = Folder::new();
    fs::write(folder.root.join("library.json"), "{").unwrap();
    assert!(matches!(
        manager::read_library(&folder.root),
        Err(AppError::FolderUnreadable { .. })
    ));

    fs::write(
        folder.root.join("library.json"),
        r#"{"version": 9, "mods": [{"slug": "bare", "format": "later"}], "later": true}"#,
    )
    .unwrap();
    let library = manager::read_library(&folder.root).unwrap();
    assert_eq!(library.mods.len(), 1);
    assert_eq!(library.mods[0].name, "bare");
    assert_eq!(library.mods[0].format, ManagerFormat::Other);
    assert_eq!(library.mods[0].archive, None);
}
