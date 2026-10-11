//! The numbered records under `docs/`, bundled into the app at build time (#ADR-0091) and read
//! here: listed, opened, and linked by the codes their text cites. `build.rs` writes the
//! bundle. The records are the repository's own text, read-only in the app.

use serde::{Deserialize, Serialize};

/// A record as `build.rs` bundles it.
struct Bundled {
    /// Its code without `#`: `GAME-CHANGE-0005`.
    code: &'static str,
    /// The heading's text after the code.
    title: &'static str,
    /// The whole file, as Markdown.
    body: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/records.rs"));

/// A series of numbered records under `docs/`. The library's `Series` names the series code
/// cites as data; this one holds the decisions too, since the app lists them all.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "kebab-case")]
pub enum Series {
    /// A decision.
    Adr,
    /// A change Riot made to the game.
    GameChange,
    /// How the game client behaves.
    GameBehavior,
    /// What tools, mod managers and authors' practices put in mods.
    ModArtifact,
}

impl Series {
    /// Every series, in the order the app lists them.
    const ALL: [Series; 4] = [
        Series::Adr,
        Series::GameChange,
        Series::GameBehavior,
        Series::ModArtifact,
    ];

    /// The series' part of a code: `GAME-CHANGE` of `#GAME-CHANGE-0005`.
    fn prefix(self) -> &'static str {
        match self {
            Series::Adr => "ADR",
            Series::GameChange => "GAME-CHANGE",
            Series::GameBehavior => "GAME-BEHAVIOR",
            Series::ModArtifact => "MOD-ARTIFACT",
        }
    }
}

/// A record as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct RecordHead {
    /// Its code as a citation: `#GAME-CHANGE-0005`.
    pub code: String,
    /// The series it is in.
    pub series: Series,
    /// Its number in the series.
    pub number: u32,
    /// The heading's text after the code.
    pub title: String,
    /// An ADR's status line as written: `Accepted (2026-10-04)`. None for the other series.
    pub status: Option<String>,
}

/// A record opened: its head, its text and the records it names and is named by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct RecordView {
    /// The record as the list shows it.
    pub head: RecordHead,
    /// The whole file, as Markdown.
    pub markdown: String,
    /// Every record its text names, each once, sorted, itself left out.
    pub cites: Vec<String>,
    /// Every bundled record whose text names it, sorted.
    pub cited_by: Vec<String>,
}

/// Every bundled record, in series order then number order.
pub fn list() -> Vec<RecordHead> {
    RECORDS.iter().map(head).collect()
}

/// The bundled record of a code, with or without its `#`. None for a code the bundle lacks.
pub fn record(code: &str) -> Option<RecordView> {
    let bare = code.strip_prefix('#').unwrap_or(code);
    let bundled = RECORDS.iter().find(|bundled| bundled.code == bare)?;
    let head = head(bundled);
    let cites = cited(bundled.body)
        .into_iter()
        .filter(|cited| *cited != head.code)
        .collect();
    let cited_by = RECORDS
        .iter()
        .filter(|other| other.code != bare && cited(other.body).contains(&head.code))
        .map(|other| format!("#{}", other.code))
        .collect();
    Some(RecordView {
        markdown: bundled.body.to_owned(),
        head,
        cites,
        cited_by,
    })
}

fn head(bundled: &Bundled) -> RecordHead {
    let (series, number) = Series::ALL
        .into_iter()
        .find_map(|series| {
            let number = bundled
                .code
                .strip_prefix(series.prefix())?
                .strip_prefix('-')?;
            Some((series, number.parse().ok()?))
        })
        .unwrap_or_else(|| panic!("build.rs bundled {:?}, no record's code", bundled.code));
    RecordHead {
        code: format!("#{}", bundled.code),
        series,
        number,
        title: bundled.title.to_owned(),
        status: (series == Series::Adr)
            .then(|| status(bundled.body))
            .flatten(),
    }
}

/// The text of an ADR's `**Status:**` line, with the lines it wraps onto.
fn status(body: &str) -> Option<String> {
    let mut lines = body.lines();
    let first = lines.find_map(|line| line.strip_prefix("- **Status:**"))?;
    // A wrapped line of the list item is indented under its text.
    let rest = lines.take_while(|line| line.starts_with("  ") && !line.trim().is_empty());
    let words: Vec<&str> = std::iter::once(first).chain(rest).map(str::trim).collect();
    Some(words.join(" "))
}

/// Every code a text names, `#SERIES-NNNN`, each once and sorted.
fn cited(text: &str) -> Vec<String> {
    let mut codes: Vec<String> = text
        .match_indices('#')
        .filter_map(|(at, _)| code_at(&text[at + 1..]))
        .map(|code| format!("#{code}"))
        .collect();
    codes.sort();
    codes.dedup();
    codes
}

/// The code at the start of a text, `GAME-CHANGE-0005`, when a series' part, a hyphen and four
/// digits start it and no further digit or letter follows.
fn code_at(text: &str) -> Option<&str> {
    Series::ALL.into_iter().find_map(|series| {
        let digits = text.strip_prefix(series.prefix())?.strip_prefix('-')?;
        let fits = digits.len() >= 4
            && digits.as_bytes()[..4].iter().all(u8::is_ascii_digit)
            && !digits
                .as_bytes()
                .get(4)
                .is_some_and(u8::is_ascii_alphanumeric);
        fits.then(|| &text[..series.prefix().len() + 5])
    })
}
