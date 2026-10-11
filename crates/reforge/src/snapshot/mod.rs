//! The snapshots after `load` and the stages after it (#ADR-0094): the mod as each stage left
//! it, its bytes named by content id in the blob store, with what the stage was judged under.
//! A run starts from one in place of the stages up to it, and reparses the bins from the
//! store's bytes.
//!
//! The load snapshot is the report's load part with what the report's tables do not say:
//! where each bin's links lead, the groups, the files a table named, the names learnt and the
//! findings as data. The parsed objects are not in it; they are a cache of the bins' bytes.
//! A snapshot of the run group (`fix` and after) holds what its stage added, and a run
//! resumes from it with the load snapshot and the run group's snapshots before it
//! ([`Snapshots`]).
//!
//! The raw snapshot is `archive`'s, and becomes JSON at the same edge as these: its reading
//! version and its caps are in it, and a resume from one read back refuses them as it refuses
//! the snapshot `read` gave. Kept by its archive, it is a cache ([`RawCache`]).
//!
//! `docs/design/ARCHITECTURE.md`, "Shape" and "Report and dump".

mod build;
mod check;
mod decide;
mod facts;
mod fix;
mod plan;
mod raw;

use std::collections::{BTreeMap, BTreeSet};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub use self::build::{BuildSettings, BuildSnapshot};
pub use self::check::{CheckSettings, CheckSnapshot};
pub use self::decide::{DecideSettings, DecideSnapshot};
pub use self::facts::{FactsSettings, FactsSnapshot};
pub use self::fix::{FixInputs, FixSettings, FixSnapshot};
pub use self::plan::{PlanSettings, PlanSnapshot};
pub use self::raw::{Cleared, RawCache, RawEntry};
use crate::archive::{Caps, RawExtra, RawImage, RawSnapshot};
use crate::diag::{Input, Logged, Stage, UnknownCode};
use crate::env::Env;
use crate::facts::FactsError;
use crate::game::{Build, NameBook, NamesStamp};
use crate::load::{Dating, FileProvenance, Inventory, KeptFile, LoadOptions, ReworkTable};
use crate::model::entity::{
    BinNode, Character, FileId, FileNode, Group, Link, ModMeta, ModWad, SkinUnit, WadId,
};
use crate::model::hash::wad_hash_of_hex;
use crate::model::{Blobs, ChunkPath, ContentId, Mod, WadHash, bin, clean_json};
use crate::report::{HistoryInput, LoadPart};

/// The format of every snapshot: one number for their shape, raised when it changes. A
/// snapshot of another format is not read.
pub const SNAPSHOT_FORMAT: u32 = 2;

/// The snapshot of a stage, as the JSON edge writes and reads it. A snapshot type holds its
/// fields and nothing of the format: [`to_json`] and [`from_json`] are the one place a
/// snapshot becomes text and text a snapshot.
pub trait StageSnapshot: Serialize + DeserializeOwned {
    /// The stage it is kept after.
    const STAGE: Stage;
}

impl StageSnapshot for RawSnapshot {
    const STAGE: Stage = Stage::Read;
}

/// A snapshot as JSON, every character a terminal acts on written as a JSON escape, as the
/// report is.
///
/// # Panics
///
/// When serde fails to write a snapshot into memory, which no value of one makes it do.
pub fn to_json<S: StageSnapshot>(snapshot: &S) -> String {
    let json = serde_json::to_string(snapshot)
        .unwrap_or_else(|error| panic!("a {} snapshot does not write: {error}", S::STAGE));
    clean_json(&json).into_owned()
}

/// A snapshot from its JSON.
pub fn from_json<S: StageSnapshot>(json: &str) -> Result<S, ResumeError> {
    serde_json::from_str(json).map_err(ResumeError::Json)
}

/// A run's snapshots from `load` on (#ADR-0094): the load group's, and the run group's as far
/// as the run went. A run resumes after any stage whose snapshot they hold.
#[derive(Debug, Clone)]
pub struct Snapshots {
    /// The snapshot after `load`.
    pub load: LoadSnapshot,
    /// The snapshot after `fix`.
    pub fix: Option<FixSnapshot>,
    /// The snapshot after `facts`.
    pub facts: Option<FactsSnapshot>,
    /// The snapshot after `decide`.
    pub decide: Option<DecideSnapshot>,
    /// The snapshot after `plan`.
    pub plan: Option<PlanSnapshot>,
    /// The snapshot after `build`.
    pub build: Option<BuildSnapshot>,
    /// The snapshot after `check`.
    pub check: Option<CheckSnapshot>,
}

impl Snapshots {
    /// The snapshots from the load snapshot alone.
    pub fn new(load: LoadSnapshot) -> Self {
        Self {
            load,
            fix: None,
            facts: None,
            decide: None,
            plan: None,
            build: None,
            check: None,
        }
    }

    /// Every id the snapshots name, each once, in order: what a store must hold for a run to
    /// resume after any of their stages.
    pub fn ids(&self) -> Vec<ContentId> {
        let mut ids: BTreeSet<ContentId> = self.load.ids().collect();
        ids.extend(self.fix.iter().flat_map(FixSnapshot::ids));
        ids.extend(self.facts.iter().flat_map(FactsSnapshot::ids));
        ids.extend(self.decide.iter().flat_map(DecideSnapshot::ids));
        ids.extend(self.plan.iter().flat_map(PlanSnapshot::ids));
        ids.extend(self.build.iter().flat_map(BuildSnapshot::ids));
        ids.extend(self.check.iter().flat_map(CheckSnapshot::ids));
        ids.into_iter().collect()
    }
}

/// The snapshot after a stage, or the refusal of a resume after it.
pub(crate) fn after<T>(snapshot: Option<T>, stage: Stage) -> Result<T, ResumeError> {
    snapshot.ok_or(ResumeError::Missing(stage))
}

/// The snapshot after a stage, for a resume that reads the stages `through` says: none for a
/// stage it does not read, and the refusal where it reads the stage and the snapshot is
/// missing.
pub(crate) fn through<T>(
    snapshot: Option<T>,
    stage: Stage,
    through: impl Fn(Stage) -> bool,
) -> Result<Option<T>, ResumeError> {
    through(stage).then(|| after(snapshot, stage)).transpose()
}

/// The program that wrote a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    /// The workspace version.
    pub version: String,
    /// The commit it was built from. None for a build outside a git checkout.
    pub commit: Option<String>,
}

impl Program {
    /// This program.
    pub fn this() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            commit: option_env!("REFORGE_COMMIT").map(str::to_string),
        }
    }

    /// Refuses a snapshot this program did not write. A program with no commit is no one
    /// program: two builds outside a checkout share a version and differ in their code.
    fn resumes(&self) -> Result<(), ResumeError> {
        if self.commit.is_none() {
            Err(ResumeError::Uncommitted)
        } else if *self != Self::this() {
            Err(ResumeError::Program {
                wrote: self.clone(),
            })
        } else {
            Ok(())
        }
    }
}

/// The inputs `load` reads, each by what identifies it (#ADR-0094).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadInputs {
    /// The install's build, when it says.
    pub build: Option<Build>,
    /// The history. None when the run was judged against the install alone.
    pub history: Option<HistoryInput>,
    /// The name tables that loaded.
    pub names: NamesStamp,
}

impl LoadInputs {
    /// The inputs an environment has open.
    pub fn of(env: &Env) -> Self {
        Self {
            build: env.game().build().cloned(),
            history: env.history().stamp().map(|stamp| HistoryInput {
                head: stamp.head.clone(),
                newest_build: stamp.newest_build.clone(),
                builds: stamp.builds,
            }),
            names: env.names().stamp().clone(),
        }
    }

    /// The first input that differs from another's.
    fn differs(&self, other: &Self) -> Option<Input> {
        if self.build != other.build {
            Some(Input::Install)
        } else if self.history != other.history {
            Some(Input::History)
        } else if self.names != other.names {
            Some(Input::Names)
        } else {
            None
        }
    }
}

/// What a run builds its package from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A port's plan.
    Port,
    /// The fixed mod, in place (#ADR-0011).
    Repair,
}

/// A setting a stage reads, which a snapshot after it was made under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// The caps an archive is read under (#ADR-0095).
    Caps,
    /// A port or a repair.
    Mode,
    /// The file fixups and their order.
    FileFixups,
    /// Whether stale copies are carried.
    KeepStale,
    /// The files carried where they are stale copies, each named alone (#ADR-0098).
    KeptFiles,
    /// The table of reworks.
    Reworks,
    /// The fixups on bin values and their order.
    BinFixups,
    /// The table rules among them.
    Rules,
    /// The policy targets are chosen under.
    Policy,
    /// The policies compared with it.
    Compared,
    /// The targets given to units alone (#ADR-0098).
    UnitTargets,
    /// Whether a bank's events are cut from the game's events bank only.
    GameEventsOnly,
    /// Whether the units at slots other than their champion's target ship.
    KeepOtherSlots,
    /// Whether each DDS ships as it is.
    KeepDds,
    /// The checks and their order.
    Checks,
}

/// The settings a stage reads, as its snapshot declares them (#ADR-0094).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageSettings {
    /// `read`: the caps an archive is read under (#ADR-0095).
    Read(Caps),
    /// `load`.
    Load(LoadSettings),
    /// `fix`.
    Fix(FixSettings),
    /// `facts`.
    Facts(FactsSettings),
    /// `decide`.
    Decide(DecideSettings),
    /// `plan`.
    Plan(PlanSettings),
    /// `build`.
    Build(BuildSettings),
    /// `check`.
    Check(CheckSettings),
    /// `write`, which reads none: its writers and its policy are the caller's at the call.
    Write,
}

/// The settings `load` reads (#ADR-0094).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadSettings {
    /// A port or a repair: a port's load also takes the inventory and the file provenance.
    pub mode: Mode,
    /// The file fixups, by id, in the order they run.
    pub file_fixups: Vec<String>,
    /// Whether stale copies are carried as the mod's own files.
    pub keep_stale: bool,
    /// The files carried where they are stale copies, each named alone (#ADR-0098).
    pub kept_files: Vec<KeptFile>,
    /// The champions whose model was replaced, and when.
    pub reworks: ReworkTable,
}

impl LoadSettings {
    /// The settings of a run of `mode` under `options`.
    pub fn of(mode: Mode, options: &LoadOptions) -> Self {
        Self {
            mode,
            file_fixups: options.fixups.iter().map(|f| f.id().to_string()).collect(),
            keep_stale: options.keep_stale,
            kept_files: options.kept_files.clone(),
            reworks: options.reworks.clone(),
        }
    }

    /// The first setting that differs from another's.
    fn differs(&self, other: &Self) -> Option<Setting> {
        if self.mode != other.mode {
            Some(Setting::Mode)
        } else if self.file_fixups != other.file_fixups {
            Some(Setting::FileFixups)
        } else if self.keep_stale != other.keep_stale {
            Some(Setting::KeepStale)
        } else if self.kept_files != other.kept_files {
            Some(Setting::KeptFiles)
        } else if self.reworks != other.reworks {
            Some(Setting::Reworks)
        } else {
            None
        }
    }
}

/// Why a run does not start from a snapshot. The caller starts from an earlier one: from the
/// raw snapshot, else from the archive.
#[derive(Debug, thiserror::Error)]
pub enum ResumeError {
    /// The snapshot is of another format than this program reads.
    #[error("the snapshot is of format {found}, and this program reads {SNAPSHOT_FORMAT}")]
    Format {
        /// The snapshot's format.
        found: u32,
    },
    /// The raw snapshot is of another reading than this program's (#ADR-0095).
    #[error("the raw snapshot is of reading {found}, and this program reads {reads}")]
    Reading {
        /// The raw snapshot's reading.
        found: u32,
        /// This program's.
        reads: u32,
    },
    /// Another program wrote the snapshot.
    #[error("the snapshot was written by {} at {:?}", wrote.version, wrote.commit)]
    Program {
        /// The program that wrote it.
        wrote: Program,
    },
    /// A program built outside a git checkout wrote the snapshot: no commit says which.
    #[error("the snapshot was written by a build with no commit")]
    Uncommitted,
    /// An input the environment has open is not the one the snapshot was judged under.
    #[error("the run's {0:?} is not the snapshot's")]
    Input(Input),
    /// A setting a stage up to the snapshot reads is not the snapshot's.
    #[error("the run's {0:?} is not the snapshot's")]
    Setting(Setting),
    /// A run resumes after a stage whose snapshot the run's snapshots lack.
    #[error("the snapshots hold none after {0}")]
    Missing(Stage),
    /// A repair resumes after a stage it has not (#ADR-0011).
    #[error("a repair has no stage {0}")]
    NotInARepair(Stage),
    /// The game did not answer while the facts were reopened.
    #[error("the game did not answer while the facts were reopened")]
    Facts(#[source] FactsError),
    /// The run has read an archive already.
    #[error("the run has read an archive already")]
    Started,
    /// The store holds nothing under an id the snapshot names.
    #[error("the store holds no bytes under {0:?}")]
    Store(ContentId),
    /// A finding of the snapshot names a code this program does not declare.
    #[error(transparent)]
    Code(#[from] UnknownCode),
    /// The snapshot does not hold together: an id out of its place, a bin that does not read
    /// where the snapshot says it read.
    #[error("the snapshot does not hold together: {0}")]
    Malformed(String),
    /// The snapshot is not JSON of its shape.
    #[error("the snapshot does not read")]
    Json(#[source] serde_json::Error),
}

/// What of a loaded bin its bytes do not say: the file it was read from, where its links lead,
/// and why it did not read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBin {
    /// The file it was read from.
    pub file: FileId,
    /// Its links, resolved as the client resolves them.
    pub links: Vec<Link>,
    /// Why it did not read, when it did not.
    pub error: Option<String>,
}

/// One file of a WAD of the snapshot: its path where one is known, and the id of its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WadFile<'a> {
    /// Its path, where the archive or the names spell it.
    pub path: Option<&'a ChunkPath>,
    /// The id its bytes are in the store under.
    pub content: ContentId,
}

/// What a load snapshot is written from: the mod as `load` left it and what is known of it.
pub struct LoadState<'a> {
    /// The mod.
    pub module: &'a Mod,
    /// Its dating.
    pub dating: &'a Dating,
    /// What the archive carries, in a port.
    pub inventory: Option<&'a Inventory>,
    /// Whose bytes its files are, in a port.
    pub provenance: Option<&'a FileProvenance>,
    /// The names the run learnt.
    pub names: &'a NameBook,
    /// The archive's extras, as the raw snapshot has them.
    pub extras: &'a [RawExtra],
    /// The archive's image, as the raw snapshot has it.
    pub image: Option<RawImage>,
}

/// What a load snapshot reopens to: the state a run holds once `load` ends that its part
/// does not hold as it is.
pub struct Reopened {
    /// The mod, its bins parsed again from the store's bytes.
    pub module: Mod,
    /// Its dating.
    pub dating: Dating,
    /// Whose bytes its files are, in a port.
    pub provenance: Option<FileProvenance>,
    /// The names the run learnt.
    pub names: NameBook,
}

/// The snapshot after `load` (#ADR-0094). It names bytes by content id and holds none, and its
/// size is the number of paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadSnapshot {
    format: u32,
    program: Program,
    inputs: LoadInputs,
    settings: LoadSettings,
    #[serde(rename = "mod")]
    meta: ModMeta,
    extras: Vec<RawExtra>,
    image: Option<RawImage>,
    part: LoadPart,
    bins: Vec<LoadBin>,
    groups: Vec<Group>,
    named_by_table: BTreeSet<FileId>,
    same_size_capped: Vec<FileId>,
    names: NameBook,
    findings: Vec<Logged>,
}

impl StageSnapshot for LoadSnapshot {
    const STAGE: Stage = Stage::Load;
}

impl LoadSnapshot {
    /// The snapshot of a loaded mod, judged against `env` under `settings`, with the findings
    /// raised from the start of `read` to the end of `load`. Every file's bytes are in the
    /// run's store: the archive's from `read`, and the ones a file fixup made or rewrote and
    /// the game's bins taken as the mod's from `load`.
    pub fn of(
        state: &LoadState<'_>,
        env: &Env,
        settings: LoadSettings,
        findings: Vec<Logged>,
    ) -> Self {
        let m = state.module;
        let bins = m.bins.values().map(|bin| LoadBin {
            file: bin.file,
            links: bin.links.clone(),
            error: bin.error.clone(),
        });
        let capped = state.provenance.map(|p| p.capped().to_vec());
        Self {
            format: SNAPSHOT_FORMAT,
            program: Program::this(),
            inputs: LoadInputs::of(env),
            settings,
            meta: m.meta.clone(),
            extras: state.extras.to_vec(),
            image: state.image,
            part: LoadPart::of(m, state.dating, state.inventory, state.provenance),
            bins: bins.collect(),
            groups: m.groups.clone(),
            named_by_table: m.named_by_table.clone(),
            same_size_capped: capped.unwrap_or_default(),
            names: state.names.clone(),
            findings,
        }
    }

    /// The snapshot's format.
    pub fn format(&self) -> u32 {
        self.format
    }

    /// The program that wrote it.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The inputs it was judged under.
    pub fn inputs(&self) -> &LoadInputs {
        &self.inputs
    }

    /// The settings it was loaded under.
    pub fn settings(&self) -> &LoadSettings {
        &self.settings
    }

    /// What the archive says of itself.
    pub fn meta(&self) -> &ModMeta {
        &self.meta
    }

    /// The archive's extras, as the raw snapshot has them.
    pub fn extras(&self) -> &[RawExtra] {
        &self.extras
    }

    /// The archive's image, as the raw snapshot has it.
    pub fn image(&self) -> Option<RawImage> {
        self.image
    }

    /// The report's load part: the WADs, files, characters and units, the dating, and in a
    /// port the inventory and the file provenance.
    pub fn part(&self) -> &LoadPart {
        &self.part
    }

    /// The findings of `read` and `load`, in order.
    pub fn findings(&self) -> &[Logged] {
        &self.findings
    }

    /// The names the run learnt.
    pub fn names(&self) -> &NameBook {
        &self.names
    }

    /// Every id it names: its files', its extras' first bytes' and its image's.
    pub fn ids(&self) -> impl Iterator<Item = ContentId> + '_ {
        let files = self.part.files.iter().map(|file| file.content);
        let heads = self.extras.iter().map(|extra| extra.head);
        let image = self.image.and_then(|image| match image {
            RawImage::Held(id) => Some(id),
            RawImage::TooLarge(_) | RawImage::Unreadable => None,
        });
        files.chain(heads).chain(image)
    }

    /// Each WAD's files as `load` left them, by path hash: the file fixups' rewrites and the
    /// files they made among them.
    pub fn wads(&self) -> BTreeMap<WadId, BTreeMap<WadHash, WadFile<'_>>> {
        let mut wads: BTreeMap<WadId, BTreeMap<WadHash, WadFile<'_>>> = BTreeMap::new();
        for wad in &self.part.wads {
            wads.entry(wad.id).or_default();
        }
        for file in &self.part.files {
            let Some(hash) = wad_hash_of_hex(&file.hash) else {
                continue;
            };
            let at = WadFile {
                path: file.path.as_ref(),
                content: file.content,
            };
            wads.entry(file.wad).or_default().insert(hash, at);
        }
        wads
    }

    /// Refuses a run whose program, inputs or load settings are not the snapshot's.
    pub fn holds(&self, inputs: &LoadInputs, settings: &LoadSettings) -> Result<(), ResumeError> {
        if self.format != SNAPSHOT_FORMAT {
            return Err(ResumeError::Format { found: self.format });
        }
        self.program.resumes()?;
        if let Some(input) = self.inputs.differs(inputs) {
            return Err(ResumeError::Input(input));
        }
        if let Some(setting) = self.settings.differs(settings) {
            return Err(ResumeError::Setting(setting));
        }
        Ok(())
    }

    /// The state a run holds once `load` ends, its bytes in the run's store `blobs` and its
    /// bins parsed again.
    pub fn reopen(&self, blobs: &Blobs) -> Result<Reopened, ResumeError> {
        let mut m = Mod::new(self.meta.clone(), blobs.clone());
        let extras = self.extras.iter().map(|extra| extra.open(blobs));
        m.extras = extras
            .collect::<Result<_, _>>()
            .map_err(ResumeError::Store)?;
        let image = self.image.map(|image| image.open(blobs)).transpose();
        m.image = image.map_err(ResumeError::Store)?;
        self.reopen_files(&mut m, blobs)?;
        self.reopen_bins(&mut m)?;
        self.reopen_characters(&mut m)?;
        m.groups = self.groups.clone();
        m.named_by_table = self.named_by_table.clone();
        let row = &self.part.dating;
        let dating = Dating {
            not_before: row.not_before.clone(),
            evidence: row.evidence.as_ref().map(|evidence| evidence.piece),
            dated: row.dated,
            at_bound: row.at_bound,
        };
        let provenance = self.part.provenance.as_ref().map(|rows| {
            let facts = rows.iter().map(|row| (row.file, row.fact.clone()));
            FileProvenance::reopen(&m, facts, self.same_size_capped.clone())
        });
        Ok(Reopened {
            module: m,
            dating,
            provenance,
            names: self.names.clone(),
        })
    }

    fn reopen_files(&self, m: &mut Mod, blobs: &Blobs) -> Result<(), ResumeError> {
        for row in &self.part.wads {
            let id = m.wads.push(ModWad {
                name: row.name.clone(),
                role: row.role.clone(),
                layout: row.layout,
                files: Vec::new(),
                packed_locale: row.packed_locale.clone(),
            });
            in_place(id, row.id, "a WAD")?;
            if row.restoration {
                m.restorations.insert(id);
            }
        }
        for row in &self.part.files {
            let hash = wad_hash_of_hex(&row.hash);
            let hash = hash.ok_or_else(|| malformed(format!("{:?} is no chunk hash", row.hash)))?;
            if m.wads.len() <= row.wad.index() {
                return Err(malformed(format!(
                    "{} is in no WAD of the snapshot",
                    row.id
                )));
            }
            if !blobs.holds(row.content) {
                return Err(ResumeError::Store(row.content));
            }
            let pushed = m.push_file(FileNode {
                wad: row.wad,
                hash,
                path: row.path.clone(),
                size: row.size,
                content: row.content,
                kind: row.kind,
                used: row.used,
                source: row.source,
            });
            let id = pushed.map_err(|there| malformed(format!("{} is at {}", row.id, there.0)))?;
            in_place(id, row.id, "a file")?;
        }
        Ok(())
    }

    fn reopen_bins(&self, m: &mut Mod) -> Result<(), ResumeError> {
        for row in &self.bins {
            if m.files.len() <= row.file.index() {
                return Err(malformed(format!("a bin reads {}, no file", row.file)));
            }
            let bin = m.push_bin(BinNode {
                file: row.file,
                links: row.links.clone(),
                objects: Vec::new(),
                error: row.error.clone(),
            });
            if row.error.is_none() {
                let parsed = bin::parse(&m.bytes(row.file));
                let parsed =
                    parsed.map_err(|why| malformed(format!("{bin} reads no more: {why}")))?;
                m.push_objects(bin, parsed.objects);
            }
        }
        Ok(())
    }

    fn reopen_characters(&self, m: &mut Mod) -> Result<(), ResumeError> {
        for row in &self.part.characters {
            let id = m.characters.push(Character {
                name: row.name.clone(),
                spelled: row.spelled.clone(),
                kind: row.kind.clone(),
                wad: row.wad,
                units: row.units.clone(),
                root_bin: row.root_bin,
            });
            in_place(id, row.id, "a character")?;
            if row.equal_to_game {
                m.equal_to_game.insert(id);
            }
        }
        for row in &self.part.units {
            let loads = row.loads.iter().map(|&file| {
                m.bin_of(file)
                    .ok_or_else(|| malformed(format!("{} loads {file}, no bin", row.id)))
            });
            let loads = loads.collect::<Result<_, _>>()?;
            let id = m.units.push(SkinUnit {
                character: row.character,
                source_slot: row.slot,
                file: row.file,
                bin: row.bin,
                loads,
                entry: row.entry,
                entry_name_slot: row.entry_name_slot,
            });
            in_place(id, row.id, "a unit")?;
        }
        Ok(())
    }
}

fn malformed(what: String) -> ResumeError {
    ResumeError::Malformed(what)
}

/// Refuses an entity reopened at another id than the one the snapshot gives it.
fn in_place<T: crate::model::Sigil>(
    id: crate::model::Id<T>,
    row: crate::model::Id<T>,
    what: &str,
) -> Result<(), ResumeError> {
    (id == row)
        .then_some(())
        .ok_or_else(|| malformed(format!("{what} at {row} reopens at {id}")))
}
