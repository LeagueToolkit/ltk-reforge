//! The schema database downloaded and kept up to date by the app (#ADR-0059), as LTK Manager
//! keeps it: the LTK Meta Wiki's copy, byte for byte, in the `meta` folder the League Toolkit
//! programs share, with the entity tag it was served under beside it. A check asks with the tag
//! and reads no body, and an update downloads the database only when the tag has moved.

use camino::Utf8Path;
use fs_err as fs;
use reforge::game::{SCHEMA_FILE, Schema};
use reqwest::StatusCode;
use reqwest::blocking::Response;
use reqwest::header::{ETAG, IF_NONE_MATCH};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, chain};
use crate::index::Web;

/// Where the LTK Meta Wiki publishes the database.
pub const PUBLISHED: &str = "https://meta-api.leaguetoolkit.dev/v1/db";

/// The tag of the copy in the folder, beside it.
const STAMP_FILE: &str = "meta-schema.stamp.json";

/// The database as the publisher served it.
pub struct Served {
    /// Its bytes.
    pub bytes: Vec<u8>,
    /// The entity tag it was served under, when the publisher sent one.
    pub etag: Option<String>,
}

/// Where the published database is read from: the web, or a test's.
pub trait Publisher {
    /// Whether the published database is still the one served under `etag`.
    fn unchanged(&self, etag: &str) -> Result<bool, AppError>;

    /// The published database, unless it is still the one served under `known`.
    fn fetch(&self, known: Option<&str>) -> Result<Option<Served>, AppError>;
}

/// The publisher at a URL, over the web.
pub struct Wiki<'a> {
    web: &'a Web,
    url: &'a str,
}

impl<'a> Wiki<'a> {
    /// The publisher at `url`, asked through `web`.
    pub fn new(web: &'a Web, url: &'a str) -> Self {
        Self { web, url }
    }
}

fn fetch_error(error: reqwest::Error) -> AppError {
    AppError::SchemaFetch {
        detail: chain(&error),
    }
}

impl Publisher for Wiki<'_> {
    fn unchanged(&self, etag: &str) -> Result<bool, AppError> {
        // A HEAD, since the answer is the status alone and the body is some 4 MB.
        let response = self
            .web
            .client()
            .head(self.url)
            .header(IF_NONE_MATCH, etag)
            .send()
            .and_then(Response::error_for_status)
            .map_err(fetch_error)?;
        Ok(response.status() == StatusCode::NOT_MODIFIED)
    }

    fn fetch(&self, known: Option<&str>) -> Result<Option<Served>, AppError> {
        let mut request = self.web.client().get(self.url);
        if let Some(etag) = known {
            request = request.header(IF_NONE_MATCH, etag);
        }
        let response = request
            .send()
            .and_then(Response::error_for_status)
            .map_err(fetch_error)?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(None);
        }
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|etag| etag.to_str().ok())
            .map(str::to_owned);
        let bytes = response.bytes().map_err(fetch_error)?.to_vec();
        Ok(Some(Served { bytes, etag }))
    }
}

/// What the folder holds beside the database, in LTK Manager's format, so either program reads
/// the other's.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stamp {
    etag: Option<String>,
    #[serde(default)]
    generation: Option<String>,
}

/// The tag of the database in the folder at `dir`. None when it holds no database, or one no
/// update stamped: what that copy is only a download says.
fn held_etag(dir: &Utf8Path) -> Option<String> {
    if !dir.join(SCHEMA_FILE).is_file() {
        return None;
    }
    let stamp: Stamp = serde_json::from_slice(&fs::read(dir.join(STAMP_FILE)).ok()?).ok()?;
    stamp.etag
}

/// The schema database in a folder, against the published one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub struct SchemaCheck {
    /// Whether the folder holds a database.
    pub held: bool,
    /// Whether an update downloads one: the folder holds none, or another than the one
    /// published.
    pub behind: bool,
}

/// How the database in the folder at `dir` stands against the published one.
pub fn check(dir: &Utf8Path, publisher: &dyn Publisher) -> Result<SchemaCheck, AppError> {
    let behind = match held_etag(dir) {
        Some(etag) => !publisher.unchanged(&etag)?,
        None => true,
    };
    Ok(SchemaCheck {
        held: dir.join(SCHEMA_FILE).is_file(),
        behind,
    })
}

/// Writes `bytes` to `path` through a file beside it, so a program reading the folder sees the
/// old file or the new one whole.
fn replace(path: &Utf8Path, bytes: &[u8]) -> std::io::Result<()> {
    let partial = format!("{path}.partial");
    fs::write(&partial, bytes)?;
    fs::rename(&partial, path)
}

/// Downloads the published database into the folder at `dir`, unless the copy there is the
/// one published. Answers whether one was installed.
pub fn update(dir: &Utf8Path, publisher: &dyn Publisher) -> Result<bool, AppError> {
    let Some(served) = publisher.fetch(held_etag(dir).as_deref())? else {
        return Ok(false);
    };
    // Read before it is installed: a database this app does not read never replaces one it does.
    let schema = Schema::parse(&served.bytes).map_err(|error| AppError::SchemaUnusable {
        detail: chain(&error),
    })?;
    let unwritable = |error: std::io::Error| AppError::SchemaUnwritable {
        detail: error.to_string(),
    };
    fs::create_dir_all(dir).map_err(unwritable)?;
    replace(&dir.join(SCHEMA_FILE), &served.bytes).map_err(unwritable)?;
    let stamp = Stamp {
        etag: served.etag,
        generation: schema.generation().map(str::to_owned),
    };
    let json = serde_json::to_vec_pretty(&stamp).map_err(|error| AppError::Internal {
        detail: error.to_string(),
    })?;
    // After the database: a stamp that names the copy before it makes the next check download
    // again, never trust a copy it does not name.
    replace(&dir.join(STAMP_FILE), &json).map_err(unwritable)?;
    Ok(true)
}
