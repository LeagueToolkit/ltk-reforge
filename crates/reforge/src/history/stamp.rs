//! What lies beside the tables in an index's directory: `builds.tsv`, which numbers the builds,
//! and `stamp.json`, written last, which says what the index was derived from.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use serde::{Deserialize, Serialize};

use super::HistoryError;
use super::findings::{IndexStale, InstallNewer};
use super::tables::{Families, Position};
use crate::diag::Sink;
use crate::game::Build;

/// The format of the index. An index of another format is rebuilt, never migrated (#ADR-0036).
pub const FORMAT: u32 = 1;

/// The SQLite file of an index, in its directory.
pub const DB_FILE: &str = "history.sqlite";

/// The stamp of an index, in its directory.
pub const STAMP_FILE: &str = "stamp.json";

/// The builds of an index, in its directory.
pub const BUILDS_FILE: &str = "builds.tsv";

/// Orders two builds by their versions, part by part as numbers. A missing part counts as
/// zero.
pub fn version_order(a: &Build, b: &Build) -> Ordering {
    let (a, b): (Vec<u64>, Vec<u64>) = (a.parts().collect(), b.parts().collect());
    let part = |parts: &[u64], i: usize| parts.get(i).copied().unwrap_or(0);
    (0..a.len().max(b.len()))
        .map(|i| part(&a, i).cmp(&part(&b, i)))
        .find(|order| order.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// What `build.yaml` says of a build beside its version, manifest and date.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BuildDetails {
    /// Its season and patch, `16.19`.
    pub patch: String,
    /// Where the census read it: `rman`, or `rads` for the builds before 9.2.
    pub source: String,
    /// A `rads` build's solution, empty for another.
    pub solution: String,
    /// A `rads` build's release, empty for another.
    pub release: String,
    /// The version of a `rads` build's client executable, empty for another.
    pub exe: String,
    /// Whether its bins number property kinds as the builds before 10.8 do.
    pub legacy_bins: bool,
    /// The live realms that shipped it, sorted.
    pub realms: Vec<String>,
}

/// One build of the history: a line of `builds.tsv`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildRow {
    /// The build's position.
    pub at: Position,
    /// The commit of the census history that holds the build.
    pub commit: String,
    /// The build.
    pub build: Build,
    /// The id of the build's release manifest.
    pub manifest: String,
    /// The day the build went live.
    pub date: String,
    /// The rest of the build's `build.yaml`.
    pub details: BuildDetails,
}

impl BuildRow {
    /// The row as its line of `builds.tsv`: position, commit, version, manifest, date, patch,
    /// source, solution, release, exe, legacy bins (0 or 1), and the realms joined by spaces.
    pub fn line(&self) -> String {
        // The history is untrusted: a tab or a line break in a value would end it.
        let cell = |text: &str| text.replace(['\t', '\n', '\r'], " ");
        let d = &self.details;
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            self.at,
            cell(&self.commit),
            self.build,
            cell(&self.manifest),
            cell(&self.date),
            cell(&d.patch),
            cell(&d.source),
            cell(&d.solution),
            cell(&d.release),
            cell(&d.exe),
            u8::from(d.legacy_bins),
            cell(&d.realms.join(" "))
        )
    }

    /// The row a line of `builds.tsv` holds.
    pub fn parse(line: &str) -> Option<Self> {
        let mut parts = line.split('\t');
        let mut text = || parts.next().map(str::to_string);
        let (at, commit, build) = (text()?, text()?, text()?);
        let (manifest, date) = (text()?, text()?);
        let details = BuildDetails {
            patch: text()?,
            source: text()?,
            solution: text()?,
            release: text()?,
            exe: text()?,
            legacy_bins: match text()?.as_str() {
                "0" => false,
                "1" => true,
                _ => return None,
            },
            realms: text()?.split_whitespace().map(str::to_string).collect(),
        };
        let row = Self {
            at: Position(at.parse().ok()?),
            commit,
            build: Build::parse(&build)?,
            manifest,
            date,
            details,
        };
        text().is_none().then_some(row)
    }

    /// The builds of the index in `dir`, by position.
    pub fn read_all(dir: &Utf8Path) -> Result<Vec<Self>, HistoryError> {
        let path = dir.join(BUILDS_FILE);
        let mut rows = Vec::new();
        for (at, line) in fs::read_to_string(&path)?.lines().enumerate() {
            let row = Self::parse(line).filter(|row| usize::from(row.at.0) == at);
            rows.push(row.ok_or_else(|| HistoryError::Builds {
                path: path.clone(),
                line: at + 1,
            })?);
        }
        Ok(rows)
    }

    /// Writes `builds.tsv` into `dir`.
    pub fn write_all(dir: &Utf8Path, rows: &[Self]) -> Result<(), HistoryError> {
        let text: String = rows.iter().map(Self::line).collect();
        Ok(fs::write(dir.join(BUILDS_FILE), text)?)
    }
}

fn build_of_text<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Build, D::Error> {
    let text = String::deserialize(d)?;
    Build::parse(&text)
        .ok_or_else(|| serde::de::Error::custom(format_args!("{text:?} is no build")))
}

/// What an index was derived from, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    /// The index's format.
    pub format: u32,
    /// The history the index was derived from, as its builder was given it.
    pub repo: String,
    /// The commit of the head: the build appended last.
    pub head: String,
    /// The highest version among the builds. Builds land in the order they arrive, so a late
    /// hotfix of an older patch can be the head.
    #[serde(deserialize_with = "build_of_text")]
    pub newest_build: Build,
    /// How many builds the index holds.
    pub builds: usize,
    /// The families the index holds.
    pub families: Families,
    /// The version of the crate that wrote the index.
    pub writer: String,
    /// How many rows each table holds, by the table's name.
    pub rows: BTreeMap<String, u64>,
    /// Entry texts read that did not parse, so name nothing in `refs`, `tags` or `units`.
    pub unparsed_texts: usize,
}

/// The one field read before the rest, so that a stamp of another format is refused for its
/// format and not for its fields.
#[derive(Deserialize)]
struct FormatOnly {
    format: u32,
}

impl Stamp {
    /// The stamp of the index in `dir`.
    pub fn read(dir: &Utf8Path) -> Result<Self, HistoryError> {
        let path = dir.join(STAMP_FILE);
        let bytes = fs::read(&path)?;
        let unreadable = |source| HistoryError::Stamp {
            path: path.clone(),
            source,
        };
        let found: FormatOnly = serde_json::from_slice(&bytes).map_err(unreadable)?;
        if found.format != FORMAT {
            return Err(HistoryError::Format {
                dir: dir.to_owned(),
                found: found.format,
            });
        }
        serde_json::from_slice(&bytes).map_err(unreadable)
    }

    /// Writes `stamp.json` into `dir`.
    pub fn write(&self, dir: &Utf8Path) -> Result<(), HistoryError> {
        let path: Utf8PathBuf = dir.join(STAMP_FILE);
        let text = serde_json::to_string_pretty(self).map_err(|source| HistoryError::Stamp {
            path: path.clone(),
            source,
        })?;
        Ok(fs::write(path, text)?)
    }

    /// How many rows a table holds.
    pub fn rows_of(&self, table: &str) -> u64 {
        self.rows.get(table).copied().unwrap_or(0)
    }

    /// Raises what limits how far the index is trusted: `history.index-stale` when the
    /// history's head is another commit than the index's, and `history.install-newer` when
    /// the install is newer than the newest build.
    pub fn check(&self, repo_head: Option<&str>, install: Option<&Build>, sink: &mut Sink<'_>) {
        if let Some(head) = repo_head.filter(|head| *head != self.head) {
            sink.raise(IndexStale {
                index_head: self.head.clone(),
                repo_head: head.to_string(),
            });
        }
        if let Some(install) = install
            && version_order(install, &self.newest_build).is_gt()
        {
            sink.raise(InstallNewer {
                install: install.clone(),
                newest: self.newest_build.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(a: &str, b: &str) -> Ordering {
        version_order(&Build::parse(a).unwrap(), &Build::parse(b).unwrap())
    }

    #[test]
    fn versions_compare_numerically() {
        assert_eq!(order("16.19.8217343", "16.19.8207193"), Ordering::Greater);
        assert_eq!(order("16.9.1", "16.19.1"), Ordering::Less);
        assert_eq!(order("14.8.5756478__2", "14.8.5756478"), Ordering::Equal);
        assert_eq!(order("14.8.5756478__2", "14.8.5759209"), Ordering::Less);
        assert_eq!(
            order("16.19.8207193+branch.releases-16-19", "16.19.8207193"),
            Ordering::Equal
        );
        assert_eq!(order("16.19", "16.19.0"), Ordering::Equal);
    }

    #[test]
    fn a_build_row_is_its_line() {
        let row = BuildRow {
            at: Position(3),
            commit: "c3".into(),
            build: Build::parse("16.19.8207193").unwrap(),
            manifest: "ad6896ce5cf475a1".into(),
            date: "2026-09-21".into(),
            details: BuildDetails {
                patch: "16.19".into(),
                source: "rman".into(),
                realms: vec!["BR1".into(), "NA1".into()],
                ..BuildDetails::default()
            },
        };
        let line = row.line();
        assert_eq!(
            line,
            "3\tc3\t16.19.8207193\tad6896ce5cf475a1\t2026-09-21\t16.19\trman\t\t\t\t0\tBR1 NA1\n"
        );
        assert_eq!(BuildRow::parse(line.trim_end_matches('\n')), Some(row));
        let rads = "0\tc0\t8.20.2483196\te39c38647162cf28\t2018-10-03\t8.20\trads\tlol_game_client_sln\t\
                    0.0.1.250\t8.20.249.4223\t1\trads:live";
        let rads = BuildRow::parse(rads).unwrap();
        assert!(rads.details.legacy_bins);
        assert_eq!(rads.details.exe, "8.20.249.4223");
        assert_eq!(rads.details.realms, ["rads:live"]);
        assert_eq!(BuildRow::parse("3\tc3\t16.19.8207193"), None);
        assert_eq!(BuildRow::parse("3\tc3\t16.19.8207193\tm\td"), None);
        assert_eq!(BuildRow::parse("3\tc3\tsoon\tm\td\tp\ts\t\t\t\t0\t"), None);
        assert_eq!(BuildRow::parse("3\tc3\t1.1.1\tm\td\tp\ts\t\t\t\t2\t"), None);
    }
}
