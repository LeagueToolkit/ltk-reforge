//! A choice of fixups as the app edits it, and as the library saves it: `FixupSelection`, the
//! file the command reads with `--fixups`. A rule is the library's `Rule`; the types here are
//! its JSON, typed for the front end, and read through the library's own parser.

use camino::Utf8Path;
use reforge::fixups::{FixupSelection, Rule};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::catalog::FixupPhase;
use crate::error::{AppError, chain};

/// A field a rule renames or moves: its name before, and its name after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct FieldRow {
    /// The field's name in the old layout, or its hash as `0x1e6ba0c4`.
    pub from: String,
    /// Its name in the layout the target build reads.
    pub to: String,
}

/// The child struct a rule moves fields into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ChildSpec {
    /// The field of the rule's class that holds the child.
    pub field: String,
    /// The child's class.
    pub class: String,
}

/// What a rule does in the structs of its class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum RuleOp {
    /// Removes fields the target's class does not declare.
    Remove {
        /// The fields.
        fields: Vec<String>,
    },
    /// Renames fields, each from a name the target's class does not declare to one it does.
    Rename {
        /// The fields.
        fields: Vec<FieldRow>,
    },
    /// Moves fields into a child struct.
    Move {
        /// The child.
        into: ChildSpec,
        /// The fields.
        fields: Vec<FieldRow>,
    },
}

/// A record a rule rests on, as the library writes one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct RecordRef {
    /// The series: `game-change`, `game-behavior`, `mod-artifact`.
    pub series: String,
    /// Its number in the series.
    pub number: u16,
}

/// A rule: a fixup that is data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct RuleSpec {
    /// The name it is turned on and off by. A rule with a built-in fixup's id takes its place.
    pub id: String,
    /// One line for a person. None: the library writes one from the class and the operation.
    pub title: Option<String>,
    /// The records it rests on.
    pub records: Vec<RecordRef>,
    /// Where it runs.
    pub phase: FixupPhase,
    /// Whether a run takes it unless it is turned off.
    pub default: bool,
    /// The class whose structs it changes.
    pub class: String,
    /// What it does.
    pub op: RuleOp,
}

impl RuleSpec {
    /// The rule as the library's JSON writes it.
    fn to_json(&self) -> Value {
        let (name, body) = match &self.op {
            RuleOp::Remove { fields } => ("remove", json!({ "fields": fields })),
            RuleOp::Rename { fields } => ("rename", json!({ "fields": fields })),
            RuleOp::Move { into, fields } => ("move", json!({ "into": into, "fields": fields })),
        };
        let mut rule = json!({
            "id": self.id,
            "records": self.records,
            "phase": self.phase,
            "default": self.default,
            "class": self.class,
            name: body,
        });
        if let Some(title) = &self.title {
            rule["title"] = json!(title);
        }
        rule
    }

    /// The rule as the library reads it.
    pub fn rule(&self) -> Result<Rule, AppError> {
        serde_json::from_value(self.to_json()).map_err(|error| AppError::BadRule {
            id: self.id.clone(),
            detail: error.to_string(),
        })
    }

    /// A rule of the library's as the front end edits it.
    pub fn of(rule: &Rule) -> Result<Self, AppError> {
        let bad = |detail: String| AppError::BadRule {
            id: String::new(),
            detail,
        };
        let mut value = serde_json::to_value(rule).map_err(|error| bad(error.to_string()))?;
        let fields = value
            .as_object_mut()
            .ok_or_else(|| bad("a rule is no object".into()))?;
        let op = ["remove", "rename", "move"]
            .into_iter()
            .find_map(|name| fields.remove(name).map(|body| (name, body)))
            .ok_or_else(|| bad("a rule with no operation".into()))?;
        let mut body = op.1;
        body["op"] = json!(op.0);
        // A field that keeps its name is written once; the front end edits both names.
        if let Some(rows) = body.get_mut("fields").and_then(Value::as_array_mut)
            && op.0 != "remove"
        {
            for row in rows {
                if let Value::String(name) = row {
                    *row = json!({ "from": name, "to": name });
                }
            }
        }
        fields.insert("op".to_string(), body);
        fields.entry("title").or_insert(Value::Null);
        serde_json::from_value(value).map_err(|error| bad(error.to_string()))
    }
}

/// The fixups a run takes: those turned off and on from the standard set, and the rules
/// added.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct FixupChoice {
    /// Fixups turned off.
    pub off: Vec<String>,
    /// Fixups turned on that the standard set leaves out.
    pub on: Vec<String>,
    /// Rules added.
    pub rules: Vec<RuleSpec>,
}

impl FixupChoice {
    /// The choice as the library's selection.
    pub fn selection(&self) -> Result<FixupSelection, AppError> {
        let mut selection = FixupSelection::standard();
        for id in &self.off {
            selection = selection.without(id.as_str());
        }
        for id in &self.on {
            selection = selection.with(id.as_str());
        }
        for rule in &self.rules {
            selection = selection.rule(rule.rule()?);
        }
        Ok(selection)
    }

    /// The library's selection as the front end edits it.
    pub fn of(selection: &FixupSelection) -> Result<Self, AppError> {
        Ok(Self {
            off: selection.off.iter().map(ToString::to_string).collect(),
            on: selection.on.iter().map(ToString::to_string).collect(),
            rules: selection
                .rules
                .iter()
                .map(RuleSpec::of)
                .collect::<Result<_, _>>()?,
        })
    }

    /// Saves the choice as a selection file the command reads with `--fixups`.
    pub fn save(&self, path: &Utf8Path) -> Result<(), AppError> {
        self.selection()?
            .save(path)
            .map_err(|error| AppError::SelectionUnreadable {
                path: path.to_string(),
                detail: chain(&error),
            })
    }

    /// The choice a selection file holds.
    pub fn open(path: &Utf8Path) -> Result<Self, AppError> {
        let selection =
            FixupSelection::open(path).map_err(|error| AppError::SelectionUnreadable {
                path: path.to_string(),
                detail: chain(&error),
            })?;
        Self::of(&selection)
    }
}
