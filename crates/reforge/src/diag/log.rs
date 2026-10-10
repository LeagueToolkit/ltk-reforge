//! The log a run carries from its first stage to its report, and the sinks stages raise into.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::message::{Message, Part, Render};
use super::{Channel, Code, Diag, Filter, Record, Severity, Stage, Subject};

/// A diagnostic's place in the run's log, from 0. A result that a diagnostic explains holds
/// this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct DiagId(pub u32);

impl fmt::Display for DiagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A run ended because a diagnostic left it with nothing to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the run ended at diagnostic {0}")]
pub struct Halted(pub DiagId);

/// A finding with its type erased, so one log holds every kind.
trait Finding: Send + Sync {
    fn message(&self, f: &mut Message);
    fn data(&self) -> serde_json::Value;
    fn as_any(&self) -> &dyn Any;
}

impl<T: Diag> Finding for T {
    fn message(&self, f: &mut Message) {
        Diag::message(self, f);
    }

    fn data(&self) -> serde_json::Value {
        // A finding is plain data. One that does not serialise is a fault of its declaration,
        // and the report then says so in its place.
        serde_json::to_value(self)
            .unwrap_or_else(|e| serde_json::json!({ "unserialisable": e.to_string() }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A finding read back from a snapshot: its message as parts and its fields as data. It is no
/// finding type, so [`Diagnostic::downcast_ref`] finds nothing in it.
struct Recorded {
    message: Vec<Part>,
    data: serde_json::Value,
}

impl Finding for Recorded {
    fn message(&self, f: &mut Message) {
        f.parts(&self.message);
    }

    fn data(&self) -> serde_json::Value {
        self.data.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A diagnostic as a snapshot keeps it (#ADR-0094): its code by name, its finding as the
/// report's data, and its message as parts, spelled when it is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Logged {
    /// Its code's name, `area.finding`.
    pub code: String,
    /// The stage that raised it.
    pub stage: Stage,
    /// What it is about.
    pub subject: Subject,
    /// The fixup or check that raised it, when one did.
    pub by: Option<String>,
    /// The record it rests on, when it has one.
    pub record: Option<Record>,
    /// Its one line for a person, as parts.
    pub message: Vec<Part>,
    /// The finding's fields.
    pub data: serde_json::Value,
}

/// A logged diagnostic whose code the program does not declare.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no code {0} is declared")]
pub struct UnknownCode(pub String);

/// One diagnostic of a run.
pub struct Diagnostic {
    /// Its place in the log.
    pub id: DiagId,
    /// Its code, with the channel and severity the code fixes.
    pub code: Code,
    /// The stage that raised it.
    pub stage: Stage,
    /// What it is about.
    pub subject: Subject,
    /// The fixup or check that raised it, when one did.
    pub by: Option<Arc<str>>,
    /// The record it rests on, when it has one.
    pub record: Option<Record>,
    finding: Box<dyn Finding>,
}

impl Diagnostic {
    /// What the output loses.
    pub fn severity(&self) -> Severity {
        self.code.severity
    }

    /// What it speaks to.
    pub fn channel(&self) -> Channel {
        self.code.channel
    }

    /// The finding as its type, when it is a `T`.
    pub fn downcast_ref<T: Diag>(&self) -> Option<&T> {
        self.finding.as_any().downcast_ref()
    }

    /// The finding's fields, as the report holds them.
    pub fn data(&self) -> serde_json::Value {
        self.finding.data()
    }

    /// One line for a person.
    pub fn message(&self, cx: &Render<'_>) -> String {
        let mut f = Message::new();
        self.finding.message(&mut f);
        Message::write(&f.into_parts(), cx)
    }

    /// The diagnostic as a snapshot keeps it.
    pub fn logged(&self) -> Logged {
        let mut f = Message::new();
        self.finding.message(&mut f);
        Logged {
            code: self.code.name.to_string(),
            stage: self.stage,
            subject: self.subject.clone(),
            by: self.by.as_deref().map(str::to_string),
            record: self.record,
            message: f.into_parts(),
            data: self.data(),
        }
    }

    /// The diagnostic as a report holds it, and as a listener's stream writes it, one per line.
    pub fn to_json(&self, cx: &Render<'_>) -> serde_json::Value {
        let mut out = serde_json::json!({
            "id": self.id.0,
            "code": self.code.name,
            "severity": self.severity().as_str(),
            "channel": self.channel().as_str(),
            "stage": self.stage.as_str(),
            "subject": {
                "kind": self.subject.kind(),
                "id": self.subject.id(),
                "name": cx.subject_name(&self.subject),
            },
            "message": self.message(cx),
            "data": self.data(),
        });
        if let Some(by) = &self.by {
            out["by"] = serde_json::Value::from(&**by);
        }
        if let Some(record) = self.record {
            out["record"] = serde_json::Value::from(record.to_string());
        }
        out
    }
}

impl fmt::Debug for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}] {}: {}",
            self.severity(),
            self.code,
            // An entry's name is the mod's own text, and a test failure prints this.
            crate::model::clean(&self.subject.id()),
            self.message(&Render::plain())
        )
    }
}

/// Hears each diagnostic as it is raised.
type Listener = Box<dyn FnMut(&Diagnostic) + Send>;

/// The ordered, append-only log of a run. The order is the order raised, which is the same for
/// the same inputs.
#[derive(Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
    listener: Option<Listener>,
}

impl Diagnostics {
    /// An empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets what hears each diagnostic as it is raised, so a caller shows a run as it goes.
    pub fn listen(&mut self, listener: impl FnMut(&Diagnostic) + Send + 'static) {
        self.listener = Some(Box::new(listener));
    }

    /// The sink of a stage.
    pub fn at(&mut self, stage: Stage) -> Sink<'_> {
        Sink {
            log: self,
            stage,
            by: None,
            record: None,
        }
    }

    /// Every diagnostic, in the order raised.
    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    /// The diagnostic at an id.
    pub fn get(&self, id: DiagId) -> Option<&Diagnostic> {
        self.items.get(id.0 as usize)
    }

    /// How many diagnostics were raised.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether none was raised.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Every finding of one type, in the order raised.
    pub fn of<T: Diag>(&self) -> impl Iterator<Item = &T> {
        self.items.iter().filter_map(Diagnostic::downcast_ref)
    }

    /// The diagnostics a filter selects, in the order raised.
    pub fn select<'a>(&'a self, filter: &'a Filter) -> impl Iterator<Item = &'a Diagnostic> {
        self.items.iter().filter(move |d| filter.matches(d))
    }

    /// The worst severity raised.
    pub fn worst(&self) -> Option<Severity> {
        self.items.iter().map(Diagnostic::severity).max()
    }

    /// The diagnostics from the one at `from` on, as a snapshot keeps them.
    pub fn logged_from(&self, from: usize) -> Vec<Logged> {
        let items = self.items.get(from..).unwrap_or_default();
        items.iter().map(Diagnostic::logged).collect()
    }

    /// Raises logged diagnostics again, in their order, each with the code of that name in
    /// `codes`. Nothing is raised when one names a code `codes` lacks.
    pub fn restore(&mut self, logged: &[Logged], codes: &[Code]) -> Result<(), UnknownCode> {
        let code = |name: &str| codes.iter().find(|code| code.name == name).copied();
        let found: Vec<Code> = logged
            .iter()
            .map(|item| code(&item.code).ok_or_else(|| UnknownCode(item.code.clone())))
            .collect::<Result<_, _>>()?;
        for (item, code) in logged.iter().zip(found) {
            let finding = Recorded {
                message: item.message.clone(),
                data: item.data.clone(),
            };
            self.push(
                item.stage,
                item.by.as_deref().map(Arc::from),
                item.record,
                code,
                item.subject.clone(),
                Box::new(finding),
            );
        }
        Ok(())
    }

    /// The first diagnostic that left the run with nothing to write.
    pub fn halted(&self) -> Option<Halted> {
        self.items
            .iter()
            .find(|d| d.severity() == Severity::Fatal)
            .map(|d| Halted(d.id))
    }

    fn push(
        &mut self,
        stage: Stage,
        by: Option<Arc<str>>,
        record: Option<Record>,
        code: Code,
        subject: Subject,
        finding: Box<dyn Finding>,
    ) -> DiagId {
        let id = DiagId(u32::try_from(self.items.len()).unwrap_or_else(|_| {
            panic!(
                "a run raised {} diagnostics, more than 32 bits count",
                self.items.len()
            )
        }));
        let diagnostic = Diagnostic {
            id,
            code,
            stage,
            subject,
            by,
            record,
            finding,
        };
        if let Some(listener) = &mut self.listener {
            listener(&diagnostic);
        }
        self.items.push(diagnostic);
        id
    }
}

/// Where a stage, a fixup or a check raises its diagnostics. It fills in who raised them.
pub struct Sink<'a> {
    log: &'a mut Diagnostics,
    stage: Stage,
    by: Option<Arc<str>>,
    record: Option<Record>,
}

impl Sink<'_> {
    /// A sink for one fixup or check of this stage, which fills in its id and the record it
    /// implements.
    pub fn by(&mut self, id: &str, record: Option<Record>) -> Sink<'_> {
        Sink {
            log: self.log,
            stage: self.stage,
            by: Some(Arc::from(id)),
            record,
        }
    }

    /// The stage this sink raises for.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// How many diagnostics the log holds: the place of the next one raised.
    pub fn mark(&self) -> usize {
        self.log.len()
    }

    /// The diagnostics raised since `mark`, as a snapshot keeps them.
    pub fn logged_since(&self, mark: usize) -> Vec<Logged> {
        self.log.logged_from(mark)
    }

    /// Raises a finding and returns its place in the log.
    pub fn raise<D: Diag>(&mut self, finding: D) -> DiagId {
        let subject = finding.subject();
        self.log.push(
            self.stage,
            self.by.clone(),
            self.record.or(D::RECORD),
            D::CODE,
            subject,
            Box::new(finding),
        )
    }

    /// Raises a finding that leaves the run with nothing to write, and returns the error a
    /// stage ends with.
    pub fn halt<D: Diag>(&mut self, finding: D) -> Halted {
        debug_assert!(
            D::CODE.severity == Severity::Fatal,
            "{} ends a run and is declared {}",
            D::CODE,
            D::CODE.severity
        );
        Halted(self.raise(finding))
    }
}
