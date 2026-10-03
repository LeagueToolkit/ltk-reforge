//! Selecting diagnostics, and what a caller does with the ones selected. Both are data, saved
//! beside a caller's choice of fixups.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::log::{DiagId, Diagnostic, Diagnostics};
use super::{Channel, Record, Series, Severity, Stage};

/// Why a filter's text does not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FilterError {
    /// A `key=value` term with a key no filter has.
    #[error("no filter by {0:?}: severity, channel, stage, by, subject or record")]
    Key(String),
    /// A value its key does not take.
    #[error("{value:?} is no {key}")]
    Value {
        /// The term's key.
        key: &'static str,
        /// The value given.
        value: String,
    },
}

/// Selects diagnostics. A diagnostic matches when it matches every field that is set, and a
/// field with several values matches any of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Filter {
    /// Codes: `check.name-unresolved`, or an area as `check.*` or `check`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub codes: Vec<String>,
    /// The least severity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_severity: Option<Severity>,
    /// Channels.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<Channel>,
    /// Stages.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stages: Vec<Stage>,
    /// Ids of the fixups or checks that raised them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub by: Vec<String>,
    /// Subjects, by their printed id: `%u0`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subjects: Vec<String>,
    /// Records they rest on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub records: Vec<Record>,
}

impl Filter {
    /// Selects everything.
    pub fn all() -> Self {
        Self::default()
    }

    /// Also requires one of these codes or areas.
    pub fn code(mut self, pattern: &str) -> Self {
        self.codes.push(pattern.to_string());
        self
    }

    /// Also requires this severity or worse.
    pub fn at_least(mut self, severity: Severity) -> Self {
        self.min_severity = Some(severity);
        self
    }

    /// Also requires one of these channels.
    pub fn channel(mut self, channel: Channel) -> Self {
        self.channels.push(channel);
        self
    }

    /// Also requires one of these stages.
    pub fn stage(mut self, stage: Stage) -> Self {
        self.stages.push(stage);
        self
    }

    /// Also requires one of these fixups or checks.
    pub fn raised_by(mut self, id: &str) -> Self {
        self.by.push(id.to_string());
        self
    }

    /// Also requires one of these subjects, by printed id.
    pub fn subject(mut self, id: &str) -> Self {
        self.subjects.push(id.to_string());
        self
    }

    /// Whether the filter selects a diagnostic.
    pub fn matches(&self, d: &Diagnostic) -> bool {
        let any = |set: bool, hit: bool| !set || hit;
        any(
            !self.codes.is_empty(),
            self.codes.iter().any(|p| code_matches(p, d.code.name)),
        ) && self.min_severity.is_none_or(|s| d.severity() >= s)
            && any(
                !self.channels.is_empty(),
                self.channels.contains(&d.channel()),
            )
            && any(!self.stages.is_empty(), self.stages.contains(&d.stage))
            && any(
                !self.by.is_empty(),
                d.by.as_deref()
                    .is_some_and(|by| self.by.iter().any(|b| b == by)),
            )
            && any(
                !self.subjects.is_empty(),
                self.subjects.contains(&d.subject.id()),
            )
            && any(
                !self.records.is_empty(),
                d.record.is_some_and(|r| self.records.contains(&r)),
            )
    }

    /// A filter from text: terms separated by commas, each a code or area, or `key=value` with
    /// the key `severity` (that severity or worse), `channel`, `stage`, `by`, `subject` or
    /// `record`.
    pub fn parse(text: &str) -> Result<Self, FilterError> {
        let mut out = Self::default();
        for term in text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let Some((key, value)) = term.split_once('=') else {
                out.codes.push(term.to_string());
                continue;
            };
            let (key, value) = (key.trim().trim_end_matches('>'), value.trim());
            let bad = |key| FilterError::Value {
                key,
                value: value.to_string(),
            };
            match key {
                "severity" => out.min_severity = Some(word(value).map_err(|()| bad("severity"))?),
                "channel" => out.channels.push(word(value).map_err(|()| bad("channel"))?),
                "stage" => out.stages.push(word(value).map_err(|()| bad("stage"))?),
                "by" => out.by.push(value.to_string()),
                "subject" => out.subjects.push(value.to_string()),
                "record" => out.records.push(value.parse().map_err(|()| bad("record"))?),
                other => return Err(FilterError::Key(other.to_string())),
            }
        }
        Ok(out)
    }
}

/// Whether a code is the pattern, or is in the area the pattern names.
fn code_matches(pattern: &str, code: &str) -> bool {
    let area = pattern.strip_suffix(".*").unwrap_or(pattern);
    pattern == code
        || (!pattern.contains('.') || pattern.ends_with(".*"))
            && code.split_once('.').is_some_and(|(a, _)| a == area)
}

/// A lowercase word as one of the enums a report spells that way.
fn word<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, ()> {
    serde_json::from_value(serde_json::Value::from(text)).map_err(|_| ())
}

impl FromStr for Record {
    type Err = ();

    /// A record from its citation, `#GAME-CHANGE-0001`, with or without the `#`.
    fn from_str(text: &str) -> Result<Self, ()> {
        let text = text.strip_prefix('#').unwrap_or(text);
        let (series, number) = text.rsplit_once('-').ok_or(())?;
        let series = match series {
            "GAME-CHANGE" => Series::GameChange,
            "GAME-BEHAVIOR" => Series::GameBehavior,
            "MOD-ARTIFACT" => Series::ModArtifact,
            _ => return Err(()),
        };
        if number.len() != 4 {
            return Err(());
        }
        Ok(Record {
            series,
            number: number.parse().map_err(|_| ())?,
        })
    }
}

/// What a caller does with the diagnostics a filter selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// Not printed. Kept in the report.
    Hide,
    /// `write` refuses while one is in the log.
    Block,
}

/// One rule of a policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    /// The diagnostics the rule is about.
    pub filter: Filter,
    /// What is done with them.
    pub action: Action,
}

/// A caller's levels: which diagnostics are hidden, and which refuse the write. No code blocks
/// by default (#ADR-0003).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticPolicy {
    /// The rules, each applied on its own.
    pub rules: Vec<PolicyRule>,
}

impl DiagnosticPolicy {
    /// A policy for a strict run: every error a check finds refuses the write.
    pub fn strict() -> Self {
        Self::default().with(
            Filter::all().code("check.*").at_least(Severity::Error),
            Action::Block,
        )
    }

    /// Adds a rule.
    pub fn with(mut self, filter: Filter, action: Action) -> Self {
        self.rules.push(PolicyRule { filter, action });
        self
    }

    /// Whether a diagnostic is not printed.
    pub fn hides(&self, d: &Diagnostic) -> bool {
        self.rules
            .iter()
            .any(|r| r.action == Action::Hide && r.filter.matches(d))
    }

    /// The diagnostics that refuse the write, in the order raised.
    pub fn blocking(&self, log: &Diagnostics) -> Vec<DiagId> {
        log.iter()
            .filter(|d| {
                self.rules
                    .iter()
                    .any(|r| r.action == Action::Block && r.filter.matches(d))
            })
            .map(|d| d.id)
            .collect()
    }
}
