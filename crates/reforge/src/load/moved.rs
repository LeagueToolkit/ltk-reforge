//! Files at paths the game shipped once, made where it ships the asset.
//!
//! The history says a path was the game's. The path the asset has at the target build is found
//! by the renames Riot made ([`RENAMES`]), and the game must ship it. The file is made there as
//! a legacy format's twin is made, beside the original.
//!
//! When several files of the mod name one asset, the mod's own file at the path wins, then a
//! file of the mod's own bytes before one that holds what the game shipped at its path
//! (#ADR-0023: a stale copy is no override, at a former path either), then the rename listed
//! first, the format the game ships before the file's own. Without a history no
//! path is known to be a former one, and nothing moves: an author's `foo.backup.dds` does not
//! take over `foo.tex`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Serialize, Serializer};

use super::files::{FileFixCx, FileFixup};
use super::legacy::{
    Conversion, DroppedFormat, Unconverted, count_by_conversion, make, record_codes, twin,
};
use crate::diag::{Channel, Code, Diag, Message, Record, Severity, Subject};
use crate::game::GameSource;
use crate::history::HistoryError;
use crate::model::entity::{FileId, FileKind, FileSource, MadeBy, WadId};
use crate::model::{ChunkPath, FileKey, FixupInfo, Phase};

/// A file name and the directory it is in, as a step of a rename reads and gives them.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Name {
    /// The directory, with no trailing separator. Empty for a path of one part.
    dir: String,
    /// The file name.
    base: String,
}

impl Name {
    fn of(path: &ChunkPath) -> Self {
        let (dir, base) = path
            .as_str()
            .rsplit_once('/')
            .unwrap_or(("", path.as_str()));
        Self {
            dir: dir.to_string(),
            base: base.to_string(),
        }
    }

    /// The same directory with another file name.
    fn renamed(&self, base: String) -> Self {
        Self {
            dir: self.dir.clone(),
            base,
        }
    }

    /// The path, when the name is one a chunk may have.
    fn path(&self) -> Option<ChunkPath> {
        if self.dir.is_empty() {
            return ChunkPath::parse(&self.base).ok();
        }
        ChunkPath::parse(&format!("{}/{}", self.dir, self.base)).ok()
    }
}

/// The extensions of the files that took a `_base` suffix: the mesh and the skeleton, and no
/// other file of the folder (#GAME-CHANGE-0011).
const BASE_SUFFIXED: [&str; 2] = ["skn", "skl"];

/// One step of a rename, from an asset's name at an earlier build to a later one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `yunara_base.yunara.skl` to `yunara_base.skl`: what stands between the name's first
    /// dot and its extension is dropped.
    DropVariantTag,
    /// `4x_lux_base_tx_cm.dds` to `lux_base_tx_cm.dds`: the prefix is dropped.
    DropPrefix(&'static str),
    /// `skins/base/vi.skn` to `skins/base/vi_base.skn`: a mesh or a skeleton in a base skin's
    /// folder takes the suffix, unless it has it or holds a variant tag.
    AddBaseSuffix,
    /// `data/darius_skins_skin0_skins_skin1.bin` to
    /// `data/characters/darius/darius_multi_skins_skin0_skins_skin1.bin`.
    IntoCharacterFolder,
}

impl Step {
    /// The name after the step, when the step applies to it.
    fn apply(self, name: &Name) -> Option<Name> {
        let (dir, base) = (name.dir.as_str(), name.base.as_str());
        match self {
            Step::DropVariantTag => {
                let parts: Vec<&str> = base.split('.').collect();
                let [stem, tag @ .., extension] = parts.as_slice() else {
                    return None;
                };
                (!tag.is_empty()).then(|| name.renamed(format!("{stem}.{extension}")))
            }
            Step::DropPrefix(prefix) => base
                .strip_prefix(prefix)
                .map(|rest| name.renamed(rest.to_string())),
            Step::AddBaseSuffix => {
                let (stem, extension) = base.rsplit_once('.')?;
                let applies = BASE_SUFFIXED.contains(&extension)
                    && dir.ends_with("/skins/base")
                    && !stem.ends_with("_base")
                    && !stem.contains('.');
                applies.then(|| name.renamed(format!("{stem}_base.{extension}")))
            }
            Step::IntoCharacterFolder => {
                let (character, slots) = base.strip_suffix(".bin")?.split_once("_skins_")?;
                let applies = dir == "data"
                    && !character.is_empty()
                    && character.bytes().all(|b| b.is_ascii_alphanumeric());
                applies.then(|| Name {
                    dir: format!("data/characters/{character}"),
                    base: format!("{character}_multi_skins_{slots}.bin"),
                })
            }
        }
    }
}

/// A rename Riot made to the names of assets.
#[derive(Debug, PartialEq, Eq)]
pub struct Rename {
    /// The name a finding and a test call it by.
    pub name: &'static str,
    /// The game changes it follows.
    pub records: &'static [Record],
    /// The steps from the earlier name to the later one, in order.
    pub steps: &'static [Step],
}

impl Rename {
    /// The rename of a name.
    pub fn named(name: &str) -> Option<&'static Rename> {
        RENAMES.iter().find(|rename| rename.name == name)
    }

    /// The later path of the asset at `path`, when every step applies to it.
    pub fn apply(&self, path: &ChunkPath) -> Option<ChunkPath> {
        let mut name = Name::of(path);
        for step in self.steps {
            name = step.apply(&name)?;
        }
        name.path()
    }
}

impl Serialize for Rename {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name)
    }
}

const VARIANT_TAG: Record = Record::game_change(9);
const RESOLUTION_PREFIX: Record = Record::game_change(10);
const BASE_SUFFIX: Record = Record::game_change(11);
const SHARED_BIN: Record = Record::game_change(12);

/// The renames, in the order they are tried. Among several files of a mod that name one asset,
/// the file of the rename listed first is the one made.
///
/// #ADR-0038: `2x_` before `4x_`, the half-size copy before the quarter-size one.
pub static RENAMES: [Rename; 7] = [
    Rename {
        name: "variant-tag",
        records: &[VARIANT_TAG],
        steps: &[Step::DropVariantTag],
    },
    Rename {
        name: "resolution-prefix-2x",
        records: &[RESOLUTION_PREFIX],
        steps: &[Step::DropPrefix("2x_")],
    },
    Rename {
        name: "resolution-prefix-4x",
        records: &[RESOLUTION_PREFIX],
        steps: &[Step::DropPrefix("4x_")],
    },
    Rename {
        name: "variant-tag-and-prefix-2x",
        records: &[VARIANT_TAG, RESOLUTION_PREFIX],
        steps: &[Step::DropVariantTag, Step::DropPrefix("2x_")],
    },
    Rename {
        name: "variant-tag-and-prefix-4x",
        records: &[VARIANT_TAG, RESOLUTION_PREFIX],
        steps: &[Step::DropVariantTag, Step::DropPrefix("4x_")],
    },
    Rename {
        name: "base-suffix",
        records: &[BASE_SUFFIX],
        steps: &[Step::AddBaseSuffix],
    },
    Rename {
        name: "shared-bin",
        records: &[SHARED_BIN],
        steps: &[Step::IntoCharacterFolder],
    },
];

/// A path a moved asset may have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The path.
    pub path: ChunkPath,
    /// The rename that gives it.
    pub rename: &'static Rename,
    /// The format the renamed path is in, when `path` is its twin: the path the game reads the
    /// asset at since it dropped that format.
    pub twin_of: Option<&'static DroppedFormat>,
    /// Its place among the candidates of every file: the lower one wins a path.
    pub rank: usize,
}

/// The paths the asset at `path` may have, best first: each rename that applies, in the order
/// of [`RENAMES`], as the twin the game reads and then in the file's own format.
pub fn candidates(path: &ChunkPath) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (at, rename) in RENAMES.iter().enumerate() {
        let Some(renamed) = rename.apply(path) else {
            continue;
        };
        if let Some((twin, format)) = twin(&renamed) {
            out.push(Candidate {
                path: twin,
                rename,
                twin_of: Some(format),
                rank: at * 2,
            });
        }
        out.push(Candidate {
            path: renamed,
            rename,
            twin_of: None,
            rank: at * 2 + 1,
        });
    }
    out
}

/// A file made where the game ships its asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Moved {
    /// The file at the former path.
    pub from: FileId,
    /// The file made of it.
    pub made: FileId,
    /// The rename that gave the path.
    pub rename: &'static Rename,
    /// How it was made.
    pub how: Conversion,
    /// The game changes the rename follows, then the one that dropped the file's format when
    /// the file was made the twin.
    #[serde(serialize_with = "record_codes")]
    pub records: Vec<Record>,
}

/// Files of a WAD at paths the game shipped at earlier builds, made where it ships the asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FilesMoved {
    /// The WAD.
    pub wad: WadId,
    /// The files made, in the order of their paths.
    pub files: Vec<Moved>,
    /// Files at former paths whose asset has another file of the mod: the mod's own at the
    /// path, or the one made of a file with a better rename.
    pub passed_over: Vec<FileId>,
}

impl Diag for FilesMoved {
    const CODE: Code = Code::new("file.moved", Channel::Output, Severity::Note);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file")
            .text(" at paths the game shipped at earlier builds, made where it ships the asset (");
        let mut first = true;
        for rename in &RENAMES {
            let n = self.files.iter().filter(|m| m.rename == rename).count();
            if n == 0 {
                continue;
            }
            if !first {
                f.text(", ");
            }
            first = false;
            let _ = write!(f, "{n} by {}", rename.name);
        }
        f.text("): ");
        let made: Vec<Conversion> = self.files.iter().map(|m| m.how).collect();
        count_by_conversion(f, &made, "as they were");
        if !self.passed_over.is_empty() {
            let _ = write!(
                f,
                "; {} passed over, the asset already had a file of the mod",
                self.passed_over.len()
            );
        }
    }
}

/// Files of a WAD at paths the game shipped at earlier builds that do not convert to the
/// format the game ships the asset in. They stay where they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MovesFailed {
    /// The WAD.
    pub wad: WadId,
    /// The files, in the order of the paths they would take.
    pub files: Vec<Unconverted>,
}

impl Diag for MovesFailed {
    const CODE: Code = Code::new("file.move-failed", Channel::Output, Severity::Warning);

    fn subject(&self) -> Subject {
        Subject::Wad(self.wad)
    }

    fn message(&self, f: &mut Message<'_>) {
        f.count(self.files.len(), "file")
            .text(" at former paths that do not convert, left where they are: ");
        f.list(&self.files, |f, e| {
            f.file(e.file)
                .text(" (")
                .spelled(&e.why.to_string())
                .text(")");
        });
    }
}

/// The codes the moved assets raise.
pub const CODES: &[Code] = &[FilesMoved::CODE, MovesFailed::CODE];

/// A file that would take a path.
struct Want {
    /// Whether the file holds bytes the game shipped at the file's own path. Asked only
    /// where several files would take the path.
    the_games: bool,
    rank: usize,
    file: FileId,
    rename: &'static Rename,
    twin_of: Option<&'static DroppedFormat>,
}

/// The files that would take one path of one WAD.
type Wanted = BTreeMap<(WadId, ChunkPath), Vec<Want>>;

/// What the fixup found in one WAD.
#[derive(Default)]
struct Found {
    moved: Vec<Moved>,
    passed_over: Vec<FileId>,
    failed: Vec<Unconverted>,
}

/// The file fixup `moved-assets`. It implements one game change per rename, so each file made
/// names the ones its rename follows.
#[derive(Debug, Clone, Copy, Default)]
pub struct MovedAssets;

impl MovedAssets {
    /// The first candidate path the game ships, for each file at a former path.
    fn wanted(cx: &FileFixCx<'_, '_>) -> Result<Wanted, HistoryError> {
        let made_from: BTreeSet<FileId> = cx
            .module
            .files
            .values()
            .filter_map(|file| match file.source {
                FileSource::Made { from, .. } => Some(from),
                _ => None,
            })
            .collect();
        let mut wanted = Wanted::new();
        for (id, file) in cx.module.files.iter() {
            // A file a fixup made, or made another from, has a file at its asset's path.
            let placed = file.source != FileSource::Archive || made_from.contains(&id);
            if placed || cx.game.contains(file.hash) {
                continue;
            }
            let Some(path) = cx.path_of(id) else {
                continue;
            };
            if cx.history.path(file.hash)?.is_none() {
                continue;
            }
            let shipped = candidates(&path)
                .into_iter()
                .find(|candidate| cx.game.contains(candidate.path.hash()));
            if let Some(candidate) = shipped {
                let wants = wanted.entry((file.wad, candidate.path)).or_default();
                wants.push(Want {
                    the_games: false,
                    rank: candidate.rank,
                    file: id,
                    rename: candidate.rename,
                    twin_of: candidate.twin_of,
                });
            }
        }
        for wants in wanted.values_mut().filter(|wants| wants.len() > 1) {
            for want in wants {
                let file = &cx.module.files[want.file];
                let key = FileKey::from_sha256(&file.content.0);
                want.the_games = cx.history.held(file.hash, key)?.is_some();
            }
        }
        Ok(wanted)
    }

    /// Makes the files, and raises what was made. The history is asked before any file is made:
    /// when it does not answer, nothing is made.
    pub fn try_fix(&self, cx: &mut FileFixCx<'_, '_>) -> Result<(), HistoryError> {
        let wanted = Self::wanted(cx)?;
        let mut found: BTreeMap<WadId, Found> = BTreeMap::new();
        for ((wad, to), mut wants) in wanted {
            let found = found.entry(wad).or_default();
            if cx.module.file_at(wad, to.hash()).is_some() {
                found.passed_over.extend(wants.iter().map(|want| want.file));
                continue;
            }
            wants.sort_by_key(|want| (want.the_games, want.rank, want.file));
            let mut done = false;
            for want in wants {
                if done {
                    found.passed_over.push(want.file);
                    continue;
                }
                let file = &cx.module.files[want.file];
                let kind = want.twin_of.map_or(file.kind, |format| format.kind);
                // A bin path is loaded as a bin: other bytes there are no bin the game moved.
                if to.extension() == Some("bin") && file.kind != FileKind::Bin {
                    continue;
                }
                match make(&file.bytes, file.kind, kind) {
                    Ok(None) => {}
                    Ok(Some(made)) => {
                        let how = made.how;
                        let new = made.at(want.file, to.clone(), MadeBy::MovedAsset);
                        // The path was free when the candidates were sorted, and a WAD holds
                        // one chunk per path: a file there since is the asset's.
                        let Ok(made) = cx.add(new) else {
                            found.passed_over.push(want.file);
                            done = true;
                            continue;
                        };
                        let format = want.twin_of.map(|format| format.record);
                        found.moved.push(Moved {
                            from: want.file,
                            made,
                            rename: want.rename,
                            how,
                            records: want.rename.records.iter().copied().chain(format).collect(),
                        });
                        done = true;
                    }
                    Err(why) => found.failed.push(Unconverted {
                        file: want.file,
                        why,
                    }),
                }
            }
        }
        for (wad, found) in found {
            if !found.moved.is_empty() {
                cx.sink.raise(FilesMoved {
                    wad,
                    files: found.moved,
                    passed_over: found.passed_over,
                });
            }
            if !found.failed.is_empty() {
                cx.sink.raise(MovesFailed {
                    wad,
                    files: found.failed,
                });
            }
        }
        Ok(())
    }
}

impl FileFixup for MovedAssets {
    fn info(&self) -> &FixupInfo {
        static INFO: FixupInfo = FixupInfo::built_in(
            "moved-assets",
            "Files at a path the game shipped once, made where it ships the asset",
            &[VARIANT_TAG, RESOLUTION_PREFIX, BASE_SUFFIX, SHARED_BIN],
            Phase::Files,
        );
        &INFO
    }

    fn destinations(&self, path: &ChunkPath, game: &dyn GameSource) -> Vec<ChunkPath> {
        let shipped = candidates(path).into_iter();
        let shipped = shipped.filter(|candidate| game.contains(candidate.path.hash()));
        shipped.map(|candidate| candidate.path).collect()
    }

    fn fix(&self, cx: &mut FileFixCx<'_, '_>) -> Result<(), HistoryError> {
        self.try_fix(cx)
    }
}
