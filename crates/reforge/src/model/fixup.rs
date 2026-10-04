//! What a fixup says of itself: its id, its title, the records it implements, where it runs
//! and whether the standard set holds it. Data only, so a caller lists, saves and loads it.
//!
//! `docs/design/ARCHITECTURE.md`, "Fixups".

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::record::Record;

/// The name a caller turns a fixup on and off by: `vfx-filtering`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FixupId(Cow<'static, str>);

impl FixupId {
    /// The id of a built-in fixup.
    pub const fn built_in(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// An id from text.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Cow::Owned(id.into()))
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FixupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&self.0)
    }
}

impl From<&str> for FixupId {
    fn from(id: &str) -> Self {
        Self::new(id)
    }
}

impl PartialEq<str> for FixupId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for FixupId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// A build as the records name one: `12.17`, the season and the release within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// The season: `12`.
    pub major: u16,
    /// The release within the season: `17`.
    pub minor: u16,
}

impl Version {
    /// The build `major.minor`.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Text that is no build written `major.minor`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is no build written as 12.17")]
pub struct VersionError(pub String);

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(text: &str) -> Result<Self, VersionError> {
        let parts = text.split_once('.').and_then(|(major, minor)| {
            let plain = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
            (plain(major) && plain(minor)).then_some((major, minor))
        });
        parts
            .and_then(|(major, minor)| Some(Self::new(major.parse().ok()?, minor.parse().ok()?)))
            .ok_or_else(|| VersionError(text.to_string()))
    }
}

impl Serialize for Version {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = Cow::<str>::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// Where a fixup runs (#ADR-0007). The phases run in this order, and within `Layout` the
/// oldest build first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "kebab-case")]
pub enum Phase {
    /// On the file set, inside `load`, before characters and units are found.
    Files,
    /// A tool's damage taken out, before the layouts it would confuse.
    Undo,
    /// A change of Riot's followed.
    Layout {
        /// The first build that has the change.
        build: Version,
    },
    /// Values brought to the target schema.
    Conform,
    /// What the target build does not declare, removed.
    Strip,
}

impl Phase {
    /// The phase's name in a listing.
    pub fn name(&self) -> &'static str {
        match self {
            Phase::Files => "files",
            Phase::Undo => "undo",
            Phase::Layout { .. } => "layout",
            Phase::Conform => "conform",
            Phase::Strip => "strip",
        }
    }

    /// The build of a change of Riot's the phase follows.
    pub fn build(&self) -> Option<Version> {
        match self {
            Phase::Layout { build } => Some(*build),
            _ => None,
        }
    }
}

/// What a caller lists and toggles a fixup by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixupInfo {
    /// The name it is turned on and off by.
    pub id: FixupId,
    /// One line for a person: what it does.
    pub title: Cow<'static, str>,
    /// The records it implements: game changes and mod artifacts.
    #[serde(default)]
    pub records: Cow<'static, [Record]>,
    /// Where it runs.
    pub phase: Phase,
    /// Whether the standard set holds it.
    #[serde(default = "standard")]
    pub default: bool,
}

fn standard() -> bool {
    true
}

impl FixupInfo {
    /// The description of a built-in fixup of the standard set.
    pub const fn built_in(
        id: &'static str,
        title: &'static str,
        records: &'static [Record],
        phase: Phase,
    ) -> Self {
        Self {
            id: FixupId::built_in(id),
            title: Cow::Borrowed(title),
            records: Cow::Borrowed(records),
            phase,
            default: true,
        }
    }

    /// The one record the fixup implements. None when it implements several, or none.
    pub fn one_record(&self) -> Option<Record> {
        match &*self.records {
            [record] => Some(*record),
            _ => None,
        }
    }

    /// The key the fixups run in the order of: the phase, then the build, then the id.
    pub fn order(&self) -> (Phase, &FixupId) {
        (self.phase, &self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_reads_and_orders_by_its_numbers() {
        let parse = |text: &str| text.parse::<Version>();
        assert_eq!(parse("12.17"), Ok(Version::new(12, 17)));
        assert!(parse("12.6").unwrap() < parse("12.17").unwrap());
        assert!(parse("12.23").unwrap() < parse("13.1").unwrap());
        for wrong in ["12", "12.", ".17", "12.17.1", "a.b", "12.+7"] {
            assert_eq!(parse(wrong), Err(VersionError(wrong.to_string())));
        }
    }

    #[test]
    fn the_phases_order_as_they_run() {
        let layout = |major, minor| Phase::Layout {
            build: Version::new(major, minor),
        };
        let order = [
            Phase::Files,
            Phase::Undo,
            layout(12, 6),
            layout(12, 17),
            layout(13, 1),
            Phase::Conform,
            Phase::Strip,
        ];
        assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
