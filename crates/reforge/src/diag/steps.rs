//! A stage's steps: the cancel a stage reads between two of them, and the progress it reports
//! in them (#ADR-0097). Progress is counted in steps and holds no clock.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use super::{Sink, Stage};

/// A cancel a caller hands a run. Its clones share one flag: the caller keeps one and sets it,
/// from any thread, and the run reads it between the steps of each stage.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// A cancel that is not set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the flag: the stage under way ends after the step it is on.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether the flag is set.
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A stage ended at a cancel, between two of its steps. The stage gives no result, and no
/// stage after it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the run was cancelled in {0}")]
pub struct Cancelled(pub Stage);

/// One step of a stage, by its kind and its name. A name the mod spells (a WAD, a path, a
/// unit) is the mod's own text, which reaches a terminal through `clean`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Step {
    /// A WAD of the archive, read.
    Wad {
        /// Its name as the archive spells it.
        wad: String,
    },
    /// A part of a stage: of `load`, a result of `facts`, a step of `plan`, the patches
    /// applied in `check`.
    Part {
        /// Its name.
        part: String,
    },
    /// One fixup over the objects of one bin.
    Fixup {
        /// The fixup's id.
        fixup: String,
        /// The bin's path, else its hash.
        bin: String,
    },
    /// One policy's decisions.
    Policy {
        /// The policy, as `auto/own-rig`.
        policy: String,
    },
    /// One unit of a port's package.
    Unit {
        /// The unit, as `varus skin0`.
        unit: String,
    },
    /// One file of the package.
    File {
        /// Its path, else its hash.
        path: String,
    },
    /// One champion's HUD in a port's package.
    Hud {
        /// The champion's name.
        character: String,
    },
    /// One check.
    Check {
        /// The check's id.
        check: String,
    },
}

impl Step {
    /// A part of a stage, by its name.
    pub fn part(name: &str) -> Self {
        Step::Part {
            part: name.to_string(),
        }
    }
}

/// A stage begun on one of its steps: the step, its number from 1, and how many the stage
/// has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// The stage.
    pub stage: Stage,
    /// The step begun.
    pub step: Step,
    /// Its number, from 1.
    pub number: usize,
    /// How many steps the stage has.
    pub count: usize,
}

/// The steps of one stage, counted as they begin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Steps {
    count: usize,
    begun: usize,
}

impl Steps {
    /// A stage of `count` steps, none begun.
    pub fn new(count: usize) -> Self {
        Self { count, begun: 0 }
    }

    /// Begins the next step, or ends the stage where the run's cancel is set. The step is
    /// named only for a log with a listener.
    pub fn begin(
        &mut self,
        sink: &mut Sink<'_>,
        step: impl FnOnce() -> Step,
    ) -> Result<(), Cancelled> {
        sink.next_step()?;
        self.tell(sink, step);
        Ok(())
    }

    /// How many steps begun.
    pub fn begun(&self) -> usize {
        self.begun
    }

    /// Begins the next step of a stage that reads no cancel.
    pub fn tell(&mut self, sink: &mut Sink<'_>, step: impl FnOnce() -> Step) {
        self.begun += 1;
        debug_assert!(
            self.begun <= self.count,
            "step {} of a stage counted {} steps",
            self.begun,
            self.count
        );
        sink.progress(self.begun, self.count, step);
    }
}

/// The steps of a stage made of named parts, which begin in the order of their names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parts {
    names: &'static [&'static str],
    steps: Steps,
}

impl Parts {
    /// A stage of these parts, none begun.
    pub fn new(names: &'static [&'static str]) -> Self {
        Self {
            names,
            steps: Steps::new(names.len()),
        }
    }

    /// Begins the next part, or ends the stage where the run's cancel is set.
    ///
    /// # Panics
    ///
    /// When every part has begun.
    pub fn begin(&mut self, sink: &mut Sink<'_>) -> Result<(), Cancelled> {
        let begun = self.steps.begun();
        let name = self.names.get(begun).unwrap_or_else(|| {
            panic!(
                "part {} of a stage of the parts {:?}",
                begun + 1,
                self.names
            )
        });
        self.steps.begin(sink, || Step::part(name))
    }
}
