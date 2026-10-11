//! The history index asked from the History page (#ADR-0091): a command per question, each the
//! library's `History` answer with its hashes spelled through the hash tables. The page adds no
//! rule (#ADR-0046). `docs/design/ROADMAP-UX.md`, "The History page".

use reforge::env::Env;
use reforge::history::{History, Position};
use serde::Serialize;

use crate::error::AppError;

/// A build of the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct BuildView {
    /// Its position: its number in the order the builds arrived.
    pub position: u16,
    /// Its version, `16.19.8207193`.
    pub version: String,
    /// The commit of the census history that holds it.
    pub commit: String,
}

/// The builds of the history, in position order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct HistoryBuilds {
    /// The position of the build appended last, which is not always the newest version.
    pub head: u16,
    /// Every build, in position order.
    pub builds: Vec<BuildView>,
}

/// The questions the History page asks of a history.
pub struct Lookup<'a> {
    history: &'a dyn History,
}

impl<'a> Lookup<'a> {
    /// The questions over a history. Refused with no history.
    pub fn new(history: &'a dyn History) -> Result<Self, AppError> {
        if history.stamp().is_none() {
            return Err(AppError::NoIndex);
        }
        Ok(Self { history })
    }

    /// The questions over an environment's history.
    pub fn of(env: &'a Env) -> Result<Self, AppError> {
        Self::new(env.history())
    }

    /// The builds, and the head.
    pub fn builds(&self) -> HistoryBuilds {
        let builds = self.history.builds();
        HistoryBuilds {
            head: self.history.head().map_or(0, |head| head.0),
            builds: builds.iter().map(|row| self.build(row.at)).collect(),
        }
    }

    /// The build at a position, `#<n>` for a position with no build.
    fn build(&self, at: Position) -> BuildView {
        match self.history.build(at) {
            Some(row) => BuildView {
                position: at.0,
                version: row.build.to_string(),
                commit: row.commit.clone(),
            },
            None => BuildView {
                position: at.0,
                version: format!("#{at}"),
                commit: String::new(),
            },
        }
    }
}
