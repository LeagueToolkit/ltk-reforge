//! What a run finds about its history: there is none, its index does not open, the index is
//! behind the history, the install is past it.

use serde::Serialize;

use crate::diag::{Channel, Code, Diag, Input, Message, Severity, Subject};
use crate::game::Build;

/// How many digits of a commit id a message shows.
const SHORT: usize = 12;

fn short(commit: &str) -> &str {
    commit.get(..SHORT).unwrap_or(commit)
}

/// No history was given. Files, objects and clips are judged against the install only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryAbsent;

impl Diag for HistoryAbsent {
    const CODE: Code = Code::new("history.absent", Channel::Run, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Input(Input::History)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("no history: files, objects and clips are judged against the install only");
    }
}

/// A history was given and its index does not open. The run is judged against the install
/// only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexMissing {
    /// The index's directory.
    pub index: String,
    /// Why it does not open.
    pub reason: String,
}

impl Diag for IndexMissing {
    const CODE: Code = Code::new("history.index-missing", Channel::Run, Severity::Error);

    fn subject(&self) -> Subject {
        Subject::Input(Input::History)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the index at ")
            .spelled(&self.index)
            .text(" does not open: ")
            .spelled(&self.reason)
            .text("; judged against the install only");
    }
}

/// The index is of another commit than the history's head. It answers for the builds it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexStale {
    /// The commit the index was derived from.
    pub index_head: String,
    /// The history's head.
    pub repo_head: String,
}

impl Diag for IndexStale {
    const CODE: Code = Code::new("history.index-stale", Channel::Run, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Input(Input::History)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the index is of ")
            .spelled(short(&self.index_head))
            .text(", the history is at ")
            .spelled(short(&self.repo_head))
            .text("; append to the index or rebuild it");
    }
}

/// The install is newer than the newest build the history holds, so bytes the game shipped
/// since pass as the mod's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallNewer {
    /// The install's build.
    pub install: Build,
    /// The newest build of the history.
    pub newest: Build,
}

impl Diag for InstallNewer {
    const CODE: Code = Code::new("history.install-newer", Channel::Run, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Input(Input::History)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.text("the install is ")
            .text(self.install.as_str())
            .text(", the history ends at ")
            .text(self.newest.as_str())
            .text(": bytes shipped since pass as the mod's own");
    }
}
