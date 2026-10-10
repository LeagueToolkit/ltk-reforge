//! The load snapshot (#ADR-0094): the mod as `load` left it, its bytes named by content id in
//! the blob store, with what the stage was judged under. A run starts from one in place of
//! `read` and `load`, and reparses the bins from the store's bytes.
//!
//! The snapshot is the report's load part with what the report's tables do not say: where
//! each bin's links lead, the groups, the files a table named, the names learnt and the
//! findings as data. The parsed objects are not in it; they are a cache of the bins' bytes.
//!
//! `docs/design/ARCHITECTURE.md`, "Shape" and "Report and dump".

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::archive::{RawExtra, RawImage};
use crate::diag::{Input, Logged, UnknownCode};
use crate::env::Env;
use crate::game::{Build, NameBook, NamesStamp};
use crate::load::{Dating, FileProvenance, Inventory, LoadOptions, ReworkTable};
use crate::model::entity::{
    BinNode, Character, FileId, FileNode, FileSource, Group, Link, ModMeta, ModWad, SkinUnit, WadId,
};
use crate::model::hash::wad_hash_of_hex;
use crate::model::{ChunkPath, ContentId, Mod, WadHash, bin, clean_json};
use crate::report::{HistoryInput, LoadPart};
use crate::store::BlobStore;

/// The format of every snapshot: one number for their shape, raised when it changes. A
/// snapshot of another format is not read.
pub const SNAPSHOT_FORMAT: u32 = 1;

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

/// A setting `load` reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadSetting {
    /// A port or a repair.
    Mode,
    /// The file fixups and their order.
    FileFixups,
    /// Whether stale copies are carried.
    KeepStale,
    /// The table of reworks.
    Reworks,
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
            reworks: options.reworks.clone(),
        }
    }

    /// The first setting that differs from another's.
    fn differs(&self, other: &Self) -> Option<LoadSetting> {
        if self.mode != other.mode {
            Some(LoadSetting::Mode)
        } else if self.file_fixups != other.file_fixups {
            Some(LoadSetting::FileFixups)
        } else if self.keep_stale != other.keep_stale {
            Some(LoadSetting::KeepStale)
        } else if self.reworks != other.reworks {
            Some(LoadSetting::Reworks)
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
    /// An input the environment has open is not the one the snapshot was judged under.
    #[error("the run's {0:?} is not the snapshot's")]
    Input(Input),
    /// A setting `load` reads is not the snapshot's.
    #[error("the run's {0:?} is not the snapshot's")]
    Setting(LoadSetting),
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

/// What a load snapshot reopens to: the state a run holds once `load` ends.
pub struct Reopened {
    /// The mod, its bins parsed again from the store's bytes.
    pub module: Mod,
    /// Its dating.
    pub dating: Dating,
    /// What the archive carries, in a port.
    pub inventory: Option<Inventory>,
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

impl LoadSnapshot {
    /// The snapshot of a loaded mod, judged against `env` under `settings`, with the findings
    /// raised from the start of `read` to the end of `load`. The bytes a file fixup made or
    /// rewrote and the game's bins taken as the mod's go into `store`; the archive's own are
    /// there from `read`.
    ///
    /// # Panics
    ///
    /// When the store keeps a file's bytes under another id than the file names: a store that
    /// does not hash as `ContentId::of` does.
    pub fn of(
        state: &LoadState<'_>,
        env: &Env,
        settings: LoadSettings,
        findings: Vec<Logged>,
        store: &dyn BlobStore,
    ) -> Self {
        let m = state.module;
        for file in m.files.values() {
            if file.source != FileSource::Archive {
                let id = store.put(file.bytes.clone());
                assert_eq!(
                    id, file.content,
                    "the store keeps a file's bytes under {id:?}"
                );
            }
        }
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

    /// The snapshot as JSON, every character a terminal acts on written as a JSON escape.
    ///
    /// # Panics
    ///
    /// When serde fails to write a snapshot into memory, which no value of one makes it do.
    pub fn to_json(&self) -> String {
        let json = serde_json::to_string(self)
            .unwrap_or_else(|error| panic!("a load snapshot does not write: {error}"));
        clean_json(&json).into_owned()
    }

    /// A snapshot from its JSON.
    pub fn from_json(json: &str) -> Result<Self, ResumeError> {
        serde_json::from_str(json).map_err(ResumeError::Json)
    }

    /// Refuses a run whose program, inputs or load settings are not the snapshot's.
    pub fn holds(&self, inputs: &LoadInputs, settings: &LoadSettings) -> Result<(), ResumeError> {
        if self.format != SNAPSHOT_FORMAT {
            return Err(ResumeError::Format { found: self.format });
        }
        if self.program != Program::this() {
            return Err(ResumeError::Program {
                wrote: self.program.clone(),
            });
        }
        if let Some(input) = self.inputs.differs(inputs) {
            return Err(ResumeError::Input(input));
        }
        if let Some(setting) = self.settings.differs(settings) {
            return Err(ResumeError::Setting(setting));
        }
        Ok(())
    }

    /// The state a run holds once `load` ends, its bytes from `store` and its bins parsed
    /// again.
    pub fn reopen(&self, store: &dyn BlobStore) -> Result<Reopened, ResumeError> {
        let mut m = Mod::new(self.meta.clone());
        let extras = self.extras.iter().map(|extra| extra.open(store));
        m.extras = extras
            .collect::<Result<_, _>>()
            .map_err(ResumeError::Store)?;
        let image = self.image.map(|image| image.open(store)).transpose();
        m.image = image.map_err(ResumeError::Store)?;
        self.reopen_files(&mut m, store)?;
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
            inventory: self.part.inventory.clone(),
            provenance,
            names: self.names.clone(),
        })
    }

    fn reopen_files(&self, m: &mut Mod, store: &dyn BlobStore) -> Result<(), ResumeError> {
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
            let bytes = store
                .get(row.content)
                .ok_or(ResumeError::Store(row.content))?;
            let pushed = m.push_file(FileNode {
                wad: row.wad,
                hash,
                path: row.path.clone(),
                size: row.size,
                content: row.content,
                kind: row.kind,
                bytes,
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
                let parsed = bin::parse(&m.files[row.file].bytes);
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
