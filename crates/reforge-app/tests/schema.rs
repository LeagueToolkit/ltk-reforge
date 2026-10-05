//! The schema database downloaded by the app: the published copy installed byte for byte, with
//! its entity tag beside it as LTK Manager writes it, and asked for again only when the tag has
//! moved.

use std::cell::Cell;

use camino::Utf8PathBuf;
use fs_err as fs;
use reforge::game::{SCHEMA_FILE, Schema};
use reforge_app::error::AppError;
use reforge_app::schema::{self, Publisher, SchemaCheck, Served};
use tempfile::TempDir;

/// A database of no class that describes builds up to `latest`.
fn database(latest: u64) -> Vec<u8> {
    serde_json::json!({
        "formatVersion": 1,
        "latest": latest,
        "hashSource": { "fetchedAt": "2026-09-28T09:29:09Z" },
        "classes": {},
    })
    .to_string()
    .into_bytes()
}

/// A publisher of one database under one tag, counting the requests it answers.
struct Wiki {
    bytes: Vec<u8>,
    etag: String,
    asked: Cell<usize>,
}

impl Wiki {
    fn new(bytes: Vec<u8>, etag: &str) -> Self {
        Self {
            bytes,
            etag: etag.to_string(),
            asked: Cell::new(0),
        }
    }
}

impl Publisher for Wiki {
    fn unchanged(&self, etag: &str) -> Result<bool, AppError> {
        self.asked.set(self.asked.get() + 1);
        Ok(etag == self.etag)
    }

    fn fetch(&self, known: Option<&str>) -> Result<Option<Served>, AppError> {
        self.asked.set(self.asked.get() + 1);
        if known == Some(self.etag.as_str()) {
            return Ok(None);
        }
        Ok(Some(Served {
            bytes: self.bytes.clone(),
            etag: Some(self.etag.clone()),
        }))
    }
}

fn scratch() -> (TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(dir.path().join("meta")).unwrap();
    (dir, path)
}

const HELD: SchemaCheck = SchemaCheck {
    held: true,
    behind: false,
};

#[test]
fn a_folder_with_no_database_is_behind_without_asking() {
    let (_dir, meta) = scratch();
    let wiki = Wiki::new(database(8217343), "\"v1\"");
    let check = schema::check(&meta, &wiki).unwrap();
    assert_eq!(
        check,
        SchemaCheck {
            held: false,
            behind: true
        }
    );
    assert_eq!(wiki.asked.get(), 0);
}

#[test]
fn an_update_installs_the_published_database_as_it_was_served() {
    let (_dir, meta) = scratch();
    let wiki = Wiki::new(database(8217343), "\"v1\"");
    assert!(schema::update(&meta, &wiki).unwrap());

    assert_eq!(fs::read(meta.join(SCHEMA_FILE)).unwrap(), wiki.bytes);
    let read = Schema::open(&meta.join(SCHEMA_FILE)).unwrap();
    assert_eq!(read.latest(), 8217343);
    assert_eq!(schema::check(&meta, &wiki).unwrap(), HELD);
    assert!(
        !schema::update(&meta, &wiki).unwrap(),
        "the copy is the one published"
    );
}

#[test]
fn the_stamp_is_the_one_ltk_manager_writes() {
    let (_dir, meta) = scratch();
    schema::update(&meta, &Wiki::new(database(8217343), "\"v1\"")).unwrap();
    let stamp: serde_json::Value =
        serde_json::from_slice(&fs::read(meta.join("meta-schema.stamp.json")).unwrap()).unwrap();
    assert_eq!(
        stamp,
        serde_json::json!({ "etag": "\"v1\"", "generation": "2026-09-28T09:29:09Z" })
    );
}

#[test]
fn a_copy_ltk_manager_installed_is_read_by_its_stamp() {
    let (_dir, meta) = scratch();
    fs::create_dir_all(&meta).unwrap();
    fs::write(meta.join(SCHEMA_FILE), database(8217343)).unwrap();
    let stamp = r#"{ "etag": "\"v1\"", "generation": "2026-09-28T09:29:09Z" }"#;
    fs::write(meta.join("meta-schema.stamp.json"), stamp).unwrap();
    let wiki = Wiki::new(database(8217343), "\"v1\"");
    assert_eq!(schema::check(&meta, &wiki).unwrap(), HELD);
}

#[test]
fn a_copy_no_update_stamped_is_behind() {
    let (_dir, meta) = scratch();
    fs::create_dir_all(&meta).unwrap();
    fs::write(meta.join(SCHEMA_FILE), database(8217343)).unwrap();
    let check = schema::check(&meta, &Wiki::new(database(8217343), "\"v1\"")).unwrap();
    assert_eq!(
        check,
        SchemaCheck {
            held: true,
            behind: true
        }
    );
}

#[test]
fn a_database_published_since_replaces_the_copy() {
    let (_dir, meta) = scratch();
    schema::update(&meta, &Wiki::new(database(8217343), "\"v1\"")).unwrap();
    let newer = Wiki::new(database(8230722), "\"v2\"");
    assert!(schema::check(&meta, &newer).unwrap().behind);
    assert!(schema::update(&meta, &newer).unwrap());
    assert_eq!(fs::read(meta.join(SCHEMA_FILE)).unwrap(), newer.bytes);
    assert_eq!(schema::check(&meta, &newer).unwrap(), HELD);
}

#[test]
fn a_database_the_app_does_not_read_replaces_nothing() {
    let (_dir, meta) = scratch();
    let first = Wiki::new(database(8217343), "\"v1\"");
    schema::update(&meta, &first).unwrap();
    let unread = Wiki::new(b"{\"formatVersion\": 2}".to_vec(), "\"v2\"");
    let error = schema::update(&meta, &unread).unwrap_err();
    assert!(matches!(error, AppError::SchemaUnusable { .. }), "{error}");
    assert_eq!(fs::read(meta.join(SCHEMA_FILE)).unwrap(), first.bytes);
    assert_eq!(schema::check(&meta, &first).unwrap(), HELD);
}

/// The database the LTK Meta Wiki publishes, installed in a scratch folder and read.
#[test]
#[ignore = "downloads the published schema database, some 4 MB"]
fn the_published_database_installs_in_a_folder() {
    let (_dir, meta) = scratch();
    let web = reforge_app::index::Web::new().unwrap();
    let wiki = schema::Wiki::new(&web, schema::PUBLISHED);
    assert!(schema::update(&meta, &wiki).unwrap());
    let read = Schema::open(&meta.join(SCHEMA_FILE)).unwrap();
    eprintln!(
        "{} bytes, builds up to {}",
        fs::metadata(meta.join(SCHEMA_FILE)).unwrap().len(),
        read.latest()
    );
    assert_eq!(schema::check(&meta, &wiki).unwrap(), HELD);
}
