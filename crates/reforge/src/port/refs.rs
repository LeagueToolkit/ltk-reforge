//! Where the client finds each file a unit's port names.
//!
//! The game has a file for a champion only where a WAD mounted with the champion holds it
//! (#GAME-BEHAVIOR-0007): the champion's own WAD and the ones mounted in every game. A mod that
//! uses another champion's texture ships the bytes for that reason, and so does the port.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::findings::{RefDangling, RefUnmounted, path_of};
use super::plan::{Objects, PlanError, PortInputs};
use super::reach::{Fate, Reach};
use crate::diag::Sink;
use crate::load::provenance::{AtPath, BytesOrigin};
use crate::load::{LoadOptions, Staleness, legacy, moved};
use crate::model::entity::{FileId, Mod, UnitId};
use crate::model::value::{self, FileRef};
use crate::model::{ChunkPath, Dump, DumpCx, Spell, WadHash, WadName, clean, name_hash};

/// The WADs mounted in every game, whatever the map and the champions, by their stems.
pub const ALWAYS_MOUNTED: &[&str] = &["global", "data", "ui", "shaders", "common"];

/// Skin entry fields a port carries from the mod whatever they hold.
pub const CARRIED_FIELDS: &[&str] = &[
    "skinMeshProperties",
    "skinAnimationProperties",
    "idleParticlesEffects",
    "mAdditionalResourceResolvers",
    "armorMaterial",
    "particleOverride_ChampionKillDeathParticle",
    "particleOverride_DeathParticle",
    "PersistentEffectConditions",
    "healthBarData",
];

/// Skin entry fields a port carries only when they name a file it ships: the mod's own icons
/// and loading screens.
pub const ICON_FIELDS: &[&str] = &[
    "iconAvatar",
    "iconCircle",
    "iconSquare",
    "alternateIconsCircle",
    "alternateIconsSquare",
    "uncensoredIconCircles",
    "uncensoredIconSquares",
    "loadscreen",
    "loadscreenVintage",
];

fn path_hex<S: Serializer>(hash: &WadHash, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(&format_args!("{:016x}", hash.0))
}

/// Where the client finds a file the port names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "kebab-case")]
pub enum FileVerdict {
    /// The game holds the bytes at this path, in a WAD mounted when the champion plays: the
    /// reference names it and nothing ships.
    Game {
        /// The path the game holds them at.
        #[serde(serialize_with = "path_hex", deserialize_with = "hex_path")]
        path: WadHash,
    },
    /// The mod's own bytes, or bytes the game does not hold where the champion plays: shipped.
    Own {
        /// The file.
        file: FileId,
    },
    /// Only a WAD that is not mounted with the champion holds it, and the mod ships nothing
    /// for it.
    Unmounted {
        /// The first WAD holding it.
        wad: WadName,
    },
    /// Nothing holds it.
    Dangling,
}

/// The files one unit's port names, each with its verdict, by path hash.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnitRefs {
    /// Per path hash named.
    pub refs: BTreeMap<WadHash, FileVerdict>,
}

impl Serialize for UnitRefs {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let rows = self.refs.iter();
        s.collect_map(rows.map(|(path, verdict)| (format!("{:016x}", path.0), verdict)))
    }
}

impl<'de> Deserialize<'de> for UnitRefs {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let rows = BTreeMap::<String, FileVerdict>::deserialize(d)?;
        let refs = rows
            .into_iter()
            .map(|(path, verdict)| Ok((path_of(&path)?, verdict)));
        Ok(Self {
            refs: refs.collect::<Result<_, D::Error>>()?,
        })
    }
}

fn hex_path<'de, D: Deserializer<'de>>(d: D) -> Result<WadHash, D::Error> {
    path_of(&String::deserialize(d)?)
}

/// Where each file a port names is found, unit by unit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRefs {
    /// By unit.
    pub units: BTreeMap<UnitId, UnitRefs>,
    /// The paths a reference was pointed to where the game ships the asset since it moved it,
    /// by their hashes.
    #[serde(skip)]
    pub moved: BTreeMap<WadHash, ChunkPath>,
    /// How many dangling files of each unit the game shipped at earlier builds.
    #[serde(skip)]
    former: BTreeMap<UnitId, usize>,
}

/// The search of one mod's references.
struct Finder<'a> {
    inputs: &'a PortInputs<'a>,
    stale: Staleness<'a>,
    /// The file of the archive at each path: the one the client uses most.
    shipped: HashMap<WadHash, FileId>,
}

/// Whether a WAD holding a path is mounted when a champion plays. `Err` names the first WAD
/// holding it when none is. `None` when the game has no file at the path.
fn mounted(inputs: &PortInputs<'_>, champion: &str, path: WadHash) -> Option<Result<(), WadName>> {
    let wads = inputs.env.game().wads(path);
    let first = wads.first()?.clone();
    let is_mounted = |wad: &WadName| wad.stem() == champion || ALWAYS_MOUNTED.contains(&wad.stem());
    Some(match wads.iter().any(is_mounted) {
        true => Ok(()),
        false => Err(first),
    })
}

impl Finder<'_> {
    /// The paths the game may ship an asset at since it dropped its path: the format it reads
    /// at the same stem, and, when the history says the path was the game's, where Riot's
    /// renames put it.
    fn moved_candidates(
        &self,
        name: Option<&str>,
        path: WadHash,
    ) -> Result<Vec<ChunkPath>, PlanError> {
        let Some(name) = name.and_then(|name| ChunkPath::parse(name).ok()) else {
            return Ok(Vec::new());
        };
        let mut paths: Vec<ChunkPath> = legacy::twin(&name)
            .map(|(twin, _)| twin)
            .into_iter()
            .collect();
        if self.inputs.env.history().path(path)?.is_some() {
            paths.extend(moved::candidates(&name).into_iter().map(|c| c.path));
        }
        Ok(paths)
    }

    fn verdict(
        &self,
        champion: &str,
        file: &FileRef,
    ) -> Result<(FileVerdict, Option<ChunkPath>), PlanError> {
        let inputs = self.inputs;
        let m: &Mod = inputs.fixed;
        let path = file.hash();
        let is_mounted = |path: WadHash| matches!(mounted(inputs, champion, path), Some(Ok(())));
        if let Some(&shipped) = self.shipped.get(&path) {
            let fact = inputs.files.fact(shipped);
            // The game's bytes at the path, or a stale copy of its older ones: the reference
            // names the game's file (#ADR-0023).
            let same = fact.is_some_and(|fact| fact.at_path == AtPath::SameBytes);
            let here = (same || self.stale.of(m, shipped)?) && is_mounted(path);
            let elsewhere = match fact.map(|fact| &fact.origin) {
                Some(BytesOrigin::Install { paths }) => {
                    paths.iter().copied().find(|p| is_mounted(*p))
                }
                _ => None,
            };
            let verdict = match (here, elsewhere) {
                (true, _) => FileVerdict::Game { path },
                (false, Some(path)) => FileVerdict::Game { path },
                (false, None) => FileVerdict::Own { file: shipped },
            };
            return Ok((verdict, None));
        }
        match mounted(inputs, champion, path) {
            Some(Ok(())) => Ok((FileVerdict::Game { path }, None)),
            Some(Err(wad)) => Ok((FileVerdict::Unmounted { wad }, None)),
            None => {
                let spelled = file.spelled().map(str::to_string);
                let name = spelled.or_else(|| inputs.names.path(path));
                let candidates = self.moved_candidates(name.as_deref(), path)?;
                if let Some(to) = candidates.iter().find(|to| is_mounted(to.hash())) {
                    let verdict = FileVerdict::Game { path: to.hash() };
                    return Ok((verdict, Some(to.clone())));
                }
                let unmounted = candidates
                    .iter()
                    .find_map(|to| mounted(inputs, champion, to.hash())?.err());
                Ok(match unmounted {
                    Some(wad) => (FileVerdict::Unmounted { wad }, None),
                    None => (FileVerdict::Dangling, None),
                })
            }
        }
    }
}

impl FileRefs {
    /// The verdict of every file the units of `reach` name, by their ported objects and their
    /// copies, the mod's objects read as `objects` holds them. Of a skin entry only the fields
    /// a port carries count.
    pub fn find(
        inputs: &PortInputs<'_>,
        objects: &Objects<'_>,
        reach: &Reach,
    ) -> Result<Self, PlanError> {
        let m: &Mod = inputs.fixed;
        let mut shipped: HashMap<WadHash, FileId> = HashMap::new();
        for (id, file) in m.files.iter() {
            let held = shipped.entry(file.hash).or_insert(id);
            if m.files[*held].used < file.used {
                *held = id;
            }
        }
        let options = LoadOptions::default().keep_stale(inputs.options.keep_stale);
        let finder = Finder {
            inputs,
            stale: Staleness::new(inputs.env, &options),
            shipped,
        };
        let carried: Vec<_> = CARRIED_FIELDS
            .iter()
            .chain(ICON_FIELDS)
            .map(|name| name_hash(name))
            .collect();
        let mut out = Self::default();
        for (&unit, reached) in &reach.units {
            let champion = m.champion_of(m.units[unit].character);
            let champion = m.characters[champion].name.as_str();
            let mut refs = UnitRefs::default();
            let mut former = 0;
            let ported = reached.ported().map(|(object, fate)| {
                let value = objects.value(object);
                match fate {
                    Fate::Entry => {
                        let mut entry = value.clone();
                        entry.properties.retain(|field, _| carried.contains(field));
                        value::file_refs(&entry)
                    }
                    _ => value::file_refs(value),
                }
            });
            // The files a copy names are the port's to place too (#ADR-0088).
            let copies = reached.copies.iter();
            let copies = copies.map(|copied| value::file_refs(&copied.object));
            for named in ported.chain(copies) {
                for file in named {
                    let path = file.hash();
                    if refs.refs.contains_key(&path) {
                        continue;
                    }
                    let (verdict, moved) = finder.verdict(champion, &file)?;
                    if let Some(to) = moved {
                        out.moved.insert(to.hash(), to);
                    }
                    if verdict == FileVerdict::Dangling
                        && inputs.env.history().path(path)?.is_some()
                    {
                        former += 1;
                    }
                    refs.refs.insert(path, verdict);
                }
            }
            out.former.insert(unit, former);
            out.units.insert(unit, refs);
        }
        Ok(out)
    }

    /// The verdicts of a unit's files.
    pub fn of(&self, unit: UnitId) -> Option<&UnitRefs> {
        self.units.get(&unit)
    }

    /// Raises the files that nothing holds and the ones no mounted WAD holds, for each unit that
    /// ships (#ADR-0072).
    pub fn report(&self, ships: &[UnitId], sink: &mut Sink<'_>) {
        let shipping = self.units.iter().filter(|(unit, _)| ships.contains(unit));
        for (&unit, refs) in shipping {
            let of = |wanted: fn(&FileVerdict) -> bool| -> Vec<WadHash> {
                let rows = refs.refs.iter().filter(|(_, verdict)| wanted(verdict));
                rows.map(|(path, _)| *path).collect()
            };
            let dangling = of(|verdict| matches!(verdict, FileVerdict::Dangling));
            if !dangling.is_empty() {
                sink.raise(RefDangling {
                    unit,
                    files: dangling,
                    former: self.former.get(&unit).copied().unwrap_or(0),
                });
            }
            let unmounted = of(|verdict| matches!(verdict, FileVerdict::Unmounted { .. }));
            if !unmounted.is_empty() {
                sink.raise(RefUnmounted {
                    unit,
                    files: unmounted,
                });
            }
        }
    }
}

impl Dump for FileRefs {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = String::new();
        for (unit, refs) in &self.units {
            let count =
                |wanted: fn(&FileVerdict) -> bool| refs.refs.values().filter(|v| wanted(v)).count();
            let _ = writeln!(
                out,
                "refs {unit} files named {}: own {}, the game's {}, unmounted {}, dangling {}",
                refs.refs.len(),
                count(|v| matches!(v, FileVerdict::Own { .. })),
                count(|v| matches!(v, FileVerdict::Game { .. })),
                count(|v| matches!(v, FileVerdict::Unmounted { .. })),
                count(|v| matches!(v, FileVerdict::Dangling)),
            );
            for (named, verdict) in &refs.refs {
                if let FileVerdict::Game { path } = verdict
                    && path != named
                {
                    let from = clean(&cx.names.path_or_hex(*named)).into_owned();
                    let to = clean(&cx.names.path_or_hex(*path)).into_owned();
                    let _ = writeln!(out, "   {from} -> the game's {to}");
                }
            }
        }
        out
    }
}
