//! The snapshot after `plan` (#ADR-0094): the port as its report holds it, the values the
//! report leaves out by the content id of their bytes (`PortValues`), and each WAD's files as
//! the port places them. A run starts from it and the snapshots before it, and reads the
//! values back from the store.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Program, ResumeError, SNAPSHOT_FORMAT, Setting, StageSnapshot};
use crate::diag::{Logged, Stage};
use crate::game::NameBook;
use crate::model::{ContentId, Mod};
use crate::port::{Port, PortOptions, PortValues, ValuesError};
use crate::store::BlobStore;

/// The settings `plan` reads (#ADR-0094). It reads no input `load` does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSettings {
    /// Whether stale copies are carried.
    pub keep_stale: bool,
    /// Whether a bank's events are cut from the game's events bank only (#ADR-0030).
    pub game_events_only: bool,
    /// Whether the units at slots other than their champion's target ship (#ADR-0082).
    pub keep_other_slots: bool,
}

impl PlanSettings {
    /// The settings of a run whose plan has these options.
    pub fn of(options: &PortOptions) -> Self {
        Self {
            keep_stale: options.keep_stale,
            game_events_only: options.game_events_only,
            keep_other_slots: options.keep_other_slots,
        }
    }

    fn differs(&self, other: &Self) -> Option<Setting> {
        if self.keep_stale != other.keep_stale {
            Some(Setting::KeepStale)
        } else if self.game_events_only != other.game_events_only {
            Some(Setting::GameEventsOnly)
        } else if self.keep_other_slots != other.keep_other_slots {
            Some(Setting::KeepOtherSlots)
        } else {
            None
        }
    }
}

/// The snapshot after `plan` (#ADR-0094).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSnapshot {
    format: u32,
    program: Program,
    settings: PlanSettings,
    wads: BTreeMap<String, BTreeMap<String, ContentId>>,
    port: Port,
    values: PortValues,
    names: NameBook,
    findings: Vec<Logged>,
}

impl StageSnapshot for PlanSnapshot {
    const STAGE: Stage = Stage::Plan;
}

impl PlanSnapshot {
    /// The snapshot of a port of the fixed mod `m`, planned under `settings`, its values put
    /// into `store`, with the names the run learnt and the findings raised from the start of
    /// `read` to the end of `plan`.
    pub fn of(
        port: &Port,
        m: &Mod,
        names: &NameBook,
        (settings, findings): (PlanSettings, Vec<Logged>),
        store: &dyn BlobStore,
    ) -> Self {
        let values = PortValues::of(port, store);
        let mut wads: BTreeMap<String, BTreeMap<String, ContentId>> = BTreeMap::new();
        let hud = port.hud.champions.iter().flat_map(|hud| &hud.files);
        for (&file, place) in port
            .files
            .iter()
            .chain(hud.map(|(file, place)| (file, place)))
        {
            let wad = m.wads[m.files[file].wad].name.to_string();
            let content = m.files[file].content;
            wads.entry(wad)
                .or_default()
                .insert(place.to_string(), content);
        }
        for (made, &content) in port.audio.banks.iter().zip(values.banks()) {
            let files = wads.entry(made.wad.to_string()).or_default();
            files.insert(made.path.to_string(), content);
        }
        Self {
            format: SNAPSHOT_FORMAT,
            program: Program::this(),
            settings,
            wads,
            port: port.part(),
            values,
            names: names.clone(),
            findings,
        }
    }

    /// The program that wrote it.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The settings `plan` read.
    pub fn settings(&self) -> &PlanSettings {
        &self.settings
    }

    /// Each WAD's files as the port places them, by path, with the id of their bytes: the
    /// mod's files the port ships, by the WAD of the mod they are in, and the banks it made,
    /// by the WAD they ship in.
    pub fn wads(&self) -> &BTreeMap<String, BTreeMap<String, ContentId>> {
        &self.wads
    }

    /// The port as its report holds it.
    pub fn part(&self) -> &Port {
        &self.port
    }

    /// The names the run learnt up to the end of `plan`.
    pub fn names(&self) -> &NameBook {
        &self.names
    }

    /// The findings from the start of `read` to the end of `plan`, in order.
    pub fn findings(&self) -> &[Logged] {
        &self.findings
    }

    /// The port `plan` gave, its values read back from `store`.
    pub fn port(&self, store: &dyn BlobStore) -> Result<Port, ResumeError> {
        let port = self.values.reopen(self.port.clone(), store);
        port.map_err(|error| match error {
            ValuesError::Missing(id) => ResumeError::Store(id),
            ValuesError::Unreadable(what) => ResumeError::Malformed(what),
        })
    }

    /// Refuses a run whose program or `plan` settings are not the snapshot's.
    pub fn holds(&self, settings: &PlanSettings) -> Result<(), ResumeError> {
        if self.format != SNAPSHOT_FORMAT {
            return Err(ResumeError::Format { found: self.format });
        }
        self.program.resumes()?;
        if let Some(setting) = self.settings.differs(settings) {
            return Err(ResumeError::Setting(setting));
        }
        Ok(())
    }
}
