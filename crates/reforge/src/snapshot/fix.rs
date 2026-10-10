//! The snapshot after `fix` (#ADR-0094): each WAD's files as `fix` left them, the bins it
//! wrote again named by the id of their new bytes, and what each fixup changed. A run starts
//! from it and its load snapshot, and reads the objects of the rewritten bins back from the
//! store.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{LoadSnapshot, Program, ResumeError, SNAPSHOT_FORMAT, Setting, StageSnapshot};
use crate::diag::{Input, Logged, Stage};
use crate::env::Env;
use crate::fixups::{After, Change, Changes, FixupSet, Rule, Snapshot as Value};
use crate::game::NameBook;
use crate::model::entity::{FileId, ObjectId, WadId};
use crate::model::value::kind_named;
use crate::model::walk::Step;
use crate::model::{ContentId, FieldPath, FixupId};
use crate::report::SchemaInput;

/// The inputs `fix` reads besides `load`'s, each by what identifies it (#ADR-0094): the
/// schema, which gives the target build's types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixInputs {
    /// The schema. None when the run had no database, and the install's bins gave the types.
    pub schema: Option<SchemaInput>,
}

impl FixInputs {
    /// The inputs an environment has open.
    pub fn of(env: &Env) -> Self {
        Self {
            schema: env.schema().map(|schema| SchemaInput {
                latest: schema.latest(),
            }),
        }
    }
}

/// The settings `fix` reads (#ADR-0094).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixSettings {
    /// The fixups on bin values, by id, in the order they run.
    pub fixups: Vec<FixupId>,
    /// The table rules among them, which are data an id does not fix.
    pub rules: Vec<Rule>,
}

impl FixSettings {
    /// The settings of a run under a fixup set.
    pub fn of(set: &FixupSet) -> Self {
        Self {
            fixups: set.bin_ids(),
            rules: set.rules().into_iter().cloned().collect(),
        }
    }

    fn differs(&self, other: &Self) -> Option<Setting> {
        if self.fixups != other.fixups {
            Some(Setting::BinFixups)
        } else if self.rules != other.rules {
            Some(Setting::Rules)
        } else {
            None
        }
    }
}

/// A value of a change as the snapshot keeps it.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValueRecord {
    kind: String,
    text: String,
}

impl ValueRecord {
    fn of(value: &Value) -> Self {
        Self {
            kind: value.kind.to_string(),
            text: value.text.clone(),
        }
    }

    fn value(&self) -> Result<Value, ResumeError> {
        Ok(Value {
            kind: kind(&self.kind)?,
            text: self.text.clone(),
        })
    }
}

fn kind(name: &str) -> Result<&'static str, ResumeError> {
    kind_named(name).ok_or_else(|| ResumeError::Malformed(format!("{name:?} is no kind")))
}

/// What a change left, as the snapshot keeps it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "as", rename_all = "kebab-case")]
enum AfterRecord {
    Value(ValueRecord),
    Moved { to: Vec<Step> },
    Removed,
    Lost,
    Unconvertible { wants: String },
}

/// A change as the snapshot keeps it: its paths with each field's class, which the report's
/// hash form leaves out.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChangeRecord {
    fixup: FixupId,
    object: ObjectId,
    path: Vec<Step>,
    before: Option<ValueRecord>,
    after: AfterRecord,
}

impl ChangeRecord {
    fn of(change: &Change) -> Self {
        let after = match &change.after {
            After::Value(value) => AfterRecord::Value(ValueRecord::of(value)),
            After::Moved { to } => AfterRecord::Moved { to: to.steps() },
            After::Removed => AfterRecord::Removed,
            After::Lost => AfterRecord::Lost,
            After::Unconvertible { wants } => AfterRecord::Unconvertible {
                wants: (*wants).to_string(),
            },
        };
        Self {
            fixup: change.fixup.clone(),
            object: change.object,
            path: change.path.steps(),
            before: change.before.as_ref().map(ValueRecord::of),
            after,
        }
    }

    fn change(&self) -> Result<Change, ResumeError> {
        let after = match &self.after {
            AfterRecord::Value(value) => After::Value(value.value()?),
            AfterRecord::Moved { to } => After::Moved {
                to: FieldPath::from_steps(to),
            },
            AfterRecord::Removed => After::Removed,
            AfterRecord::Lost => After::Lost,
            AfterRecord::Unconvertible { wants } => After::Unconvertible {
                wants: kind(wants)?,
            },
        };
        Ok(Change {
            fixup: self.fixup.clone(),
            object: self.object,
            path: FieldPath::from_steps(&self.path),
            before: self.before.as_ref().map(ValueRecord::value).transpose()?,
            after,
        })
    }
}

/// The snapshot after `fix` (#ADR-0094). It names bytes by content id and holds none.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSnapshot {
    format: u32,
    program: Program,
    inputs: FixInputs,
    settings: FixSettings,
    wads: BTreeMap<WadId, BTreeMap<String, ContentId>>,
    changes: Vec<ChangeRecord>,
    names: NameBook,
    findings: Vec<Logged>,
}

impl StageSnapshot for FixSnapshot {
    const STAGE: Stage = Stage::Fix;
}

impl FixSnapshot {
    /// The snapshot of a fixed mod loaded as `load` names it, with the bins `fix` wrote
    /// again by the file each was read from and the id of its new bytes, what the fixups
    /// changed, the names the run learnt and the findings raised from the start of `read` to
    /// the end of `fix`. The new bytes are in the store already.
    pub fn of(
        load: &LoadSnapshot,
        rewritten: &BTreeMap<FileId, ContentId>,
        changes: &Changes,
        names: &NameBook,
        (inputs, settings): (FixInputs, FixSettings),
        findings: Vec<Logged>,
    ) -> Self {
        let mut wads: BTreeMap<WadId, BTreeMap<String, ContentId>> = BTreeMap::new();
        for wad in &load.part().wads {
            wads.entry(wad.id).or_default();
        }
        for file in &load.part().files {
            let content = rewritten.get(&file.id).copied().unwrap_or(file.content);
            let files = wads.entry(file.wad).or_default();
            files.insert(file.hash.clone(), content);
        }
        Self {
            format: SNAPSHOT_FORMAT,
            program: Program::this(),
            inputs,
            settings,
            wads,
            changes: changes.iter().map(ChangeRecord::of).collect(),
            names: names.clone(),
            findings,
        }
    }

    /// The program that wrote it.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The inputs `fix` read.
    pub fn inputs(&self) -> &FixInputs {
        &self.inputs
    }

    /// The settings `fix` read.
    pub fn settings(&self) -> &FixSettings {
        &self.settings
    }

    /// Each WAD's files as `fix` left them, by path hash in hex, with the id of their bytes.
    pub fn wads(&self) -> &BTreeMap<WadId, BTreeMap<String, ContentId>> {
        &self.wads
    }

    /// The names the run learnt up to the end of `fix`.
    pub fn names(&self) -> &NameBook {
        &self.names
    }

    /// The findings from the start of `read` to the end of `fix`, in order.
    pub fn findings(&self) -> &[Logged] {
        &self.findings
    }

    /// What each fixup changed, in the order made.
    pub fn changes(&self) -> Result<Changes, ResumeError> {
        self.changes.iter().map(ChangeRecord::change).collect()
    }

    /// The bins `fix` wrote again, by the file each was read from, with the id of their new
    /// bytes: every file whose bytes differ from what `load` left.
    pub fn rewritten(
        &self,
        load: &LoadSnapshot,
    ) -> Result<BTreeMap<FileId, ContentId>, ResumeError> {
        let mut rewritten = BTreeMap::new();
        for file in &load.part().files {
            let content = self.wads.get(&file.wad).and_then(|f| f.get(&file.hash));
            let content = content.ok_or_else(|| {
                ResumeError::Malformed(format!("{} is in the load snapshot alone", file.id))
            })?;
            if *content != file.content {
                rewritten.insert(file.id, *content);
            }
        }
        Ok(rewritten)
    }

    /// Refuses a run whose program, inputs or `fix` settings are not the snapshot's.
    pub fn holds(&self, inputs: &FixInputs, settings: &FixSettings) -> Result<(), ResumeError> {
        if self.format != SNAPSHOT_FORMAT {
            return Err(ResumeError::Format { found: self.format });
        }
        self.program.resumes()?;
        if self.inputs != *inputs {
            return Err(ResumeError::Input(Input::Schema));
        }
        if let Some(setting) = self.settings.differs(settings) {
            return Err(ResumeError::Setting(setting));
        }
        Ok(())
    }
}
