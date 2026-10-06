//! What a unit's model is made of and which game skins it matches: the mesh, each submesh
//! against the candidate skins' meshes, the skeleton's joints, and the clips its graph plays.
//!
//! A mesh, a skeleton or a clip the archive does not ship, at a path the history says the game
//! shipped at earlier builds, is the game's file where Riot's renames put it
//! (#GAME-CHANGE-0009 to #GAME-CHANGE-0011): its facts are that file's.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use ltk_meta::{BinObject, PropertyValueEnum};
use serde::Serialize;

use super::copies::Copies;
use super::findings::{CopiedSubmesh, RigPart, RigUnreadable, SubmeshesCopied};
use super::origin::Origins;
use super::{Catalogs, FactsError, load_pool, pooled};
use crate::diag::Sink;
use crate::env::Env;
use crate::fixups::FixedMod;
use crate::game::{CatalogSkin, Game, SkinCatalog, graph_clips};
use crate::load::{FileProvenance, legacy, moved};
use crate::model::entity::{CharId, FileId, Mod, SkinUnit, UnitId, WadId};
use crate::model::known::{class, field};
use crate::model::rig::{self, MeshShape, Skeleton};
use crate::model::{
    ChunkPath, SideTable, SkinEntry, Slot, Spell, Spelled, WadHash, name_hash, value,
};

/// The share of a submesh's vertices that lie on a premium skin's mesh, and not on the base's,
/// from which the submesh is that skin's (#MOD-ARTIFACT-0024).
pub const COPIED_SHARE: f32 = 0.9;

/// The WADs the client mounts in every game, whatever the map and the champions, by their
/// lowercase stems.
pub const ALWAYS_MOUNTED: &[&str] = &["global", "data", "ui", "shaders", "common"];

/// Where a file a unit names comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "source", content = "file", rename_all = "lowercase")]
pub enum Source {
    /// The archive ships it.
    Mod(FileId),
    /// The archive does not ship it, and the game has it.
    Game,
    /// Neither has it.
    Missing,
}

impl Source {
    /// Whether the archive ships the file.
    pub fn is_shipped(self) -> bool {
        matches!(self, Source::Mod(_))
    }
}

/// Whether a skin shows a submesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Shown from the start.
    Visible,
    /// Hidden at the start, and shown by an animation event: a recall prop, a weapon swap.
    HiddenShown,
    /// Hidden at the start, and no event shows it.
    HiddenNever,
}

impl Visibility {
    /// The visibility's name in a message.
    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Visible => "visible",
            Visibility::HiddenShown => "hidden, shown by an event",
            Visibility::HiddenNever => "hidden, never shown",
        }
    }
}

/// A submesh of the mod's own mesh, against the game's meshes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubmeshFact {
    /// Its name.
    pub name: Spelled,
    /// How many distinct rounded positions its vertices have.
    pub vertices: usize,
    /// The share of them on the base's mesh.
    pub on_base: f32,
    /// The share of them on each candidate premium skin's mesh and not on the base's.
    pub on_skins: BTreeMap<Slot, f32>,
    /// Whether the skin shows it.
    pub visibility: Visibility,
}

impl SubmeshFact {
    /// The premium skin the submesh was copied from: the one holding the largest share of its
    /// vertices, from [`COPIED_SHARE`] up.
    pub fn copied_from(&self) -> Option<Slot> {
        let copied = self
            .on_skins
            .iter()
            .filter(|(_, share)| **share >= COPIED_SHARE);
        copied
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(slot, _)| *slot)
    }
}

fn opt_hash_hex<S: serde::Serializer>(hash: &Option<WadHash>, s: S) -> Result<S::Ok, S::Error> {
    match hash {
        Some(hash) => s.collect_str(&format_args!("{:016x}", hash.0)),
        None => s.serialize_none(),
    }
}

/// A file of the rig, as the unit names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Named {
    /// The path the entry or the graph names.
    #[serde(serialize_with = "opt_hash_hex")]
    pub reference: Option<WadHash>,
    /// Where the file comes from.
    #[serde(flatten)]
    pub source: Source,
    /// The game skins whose file this is, by path or by the bytes of a shipped copy.
    pub game_slots: BTreeSet<Slot>,
    /// Whether the file is the mod's own bytes.
    pub own: bool,
    /// Where the game ships the file, when the reference is a path it shipped at earlier
    /// builds.
    #[serde(serialize_with = "opt_hash_hex")]
    pub moved: Option<WadHash>,
}

/// The mesh of a unit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MeshFact {
    /// The file.
    #[serde(flatten)]
    pub file: Named,
    /// The submeshes of a mesh of the mod's own.
    pub submeshes: Vec<SubmeshFact>,
    /// The highest joint index a vertex of a shipped mesh binds to.
    pub max_blend: Option<u8>,
}

/// The skeleton of a unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkeletonFact {
    /// The file.
    #[serde(flatten)]
    pub file: Named,
    /// The joints' names in order, when the skeleton reads.
    pub joints: Option<Vec<String>>,
    /// How many joints a vertex may bind to.
    pub influences: Option<usize>,
}

/// A clip a unit's graph plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClipFact {
    /// The file. A clip's game slots are the skins whose graph plays it.
    #[serde(flatten)]
    pub file: Named,
    /// The hashes of the joints an own clip animates, when it reads.
    #[serde(skip)]
    pub joints: Option<Vec<u32>>,
}

impl ClipFact {
    /// The path the graph names.
    pub fn reference(&self) -> WadHash {
        self.file.reference.unwrap_or(WadHash(0))
    }
}

/// Where the graph a unit's entry names was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphFrom {
    /// Among the objects the client loads for the unit.
    Archive,
    /// Among the game's objects.
    Game,
}

/// The rig of one unit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnitRig {
    /// The mesh.
    pub mesh: MeshFact,
    /// The skeleton.
    pub skeleton: SkeletonFact,
    /// Where the graph was found. None when it was not.
    pub graph: Option<GraphFrom>,
    /// The clips the graph plays, by path.
    pub clips: Vec<ClipFact>,
    /// The parts whose file is there and does not parse.
    pub unreadable: Vec<RigPart>,
}

impl UnitRig {
    /// The hashes a clip's track may name a joint of the unit's skeleton by.
    pub fn joint_hashes(&self) -> Option<HashSet<u32>> {
        let joints = self.skeleton.joints.as_ref()?;
        Some(
            joints
                .iter()
                .flat_map(|joint| rig::joint_hashes(joint))
                .collect(),
        )
    }

    /// Whether the unit ships the model it names: its mesh, its skeleton and most of the
    /// clips it plays, whatever their bytes. A restoration ships the old game files
    /// (#ADR-0019).
    pub fn carries_model(&self) -> bool {
        let clips = self
            .clips
            .iter()
            .filter(|clip| clip.file.source.is_shipped())
            .count();
        self.mesh.file.source.is_shipped()
            && self.skeleton.file.source.is_shipped()
            && clips * 2 > self.clips.len()
    }
}

/// The rigs of the representative units, and the game skeletons they are compared with.
#[derive(Default)]
pub struct Rigs {
    units: SideTable<SkinUnit, UnitRig>,
    /// The joints of a game skin's skeleton, for the slots a unit of the character was
    /// compared with: its candidates and every skin a part of it matches. None for a skeleton
    /// that does not read.
    game: BTreeMap<(CharId, Slot), Option<Arc<Skeleton>>>,
}

/// What the rigs are found from.
pub struct RigInputs<'a> {
    /// The game and the history.
    pub env: &'a Env,
    /// The game's skins of each character.
    pub catalogs: &'a Catalogs,
    /// The roles of the units.
    pub copies: &'a Copies,
    /// Whose bytes the files are.
    pub files: &'a FileProvenance,
    /// The origins of the units.
    pub origins: &'a Origins,
    /// Spells a file a bin names by its hash.
    pub names: &'a dyn Spell,
}

/// The names of the submeshes a graph shows by an event: spelled, lowercase, and as hashes.
fn shown_submeshes(graph: &BinObject) -> (HashSet<String>, HashSet<u32>) {
    let event = name_hash(class::SUBMESH_VISIBILITY_EVENT_DATA);
    let list = name_hash(field::SHOW_SUBMESH_LIST);
    let mut spelled = HashSet::new();
    let mut hashed = HashSet::new();
    value::walk_object(graph, &mut |v| {
        let s = match v {
            PropertyValueEnum::Struct(s) => s,
            PropertyValueEnum::Embedded(e) => &e.0,
            _ => return,
        };
        let shown = s.properties.get(&list).filter(|_| s.class_hash == event);
        for item in shown.map_or(&[][..], value::items) {
            match item {
                PropertyValueEnum::String(text) => {
                    spelled.insert(text.value.to_ascii_lowercase());
                }
                PropertyValueEnum::Hash(hash) => {
                    hashed.insert(hash.value.0);
                }
                _ => {}
            }
        }
    });
    (spelled, hashed)
}

/// The skins whose file, as `pick` reads it, is at one of the paths.
fn slots_matching(
    catalog: &SkinCatalog,
    paths: &[WadHash],
    pick: impl Fn(&CatalogSkin) -> Option<WadHash>,
) -> BTreeSet<Slot> {
    let skins = catalog.skins.values();
    let matching = skins.filter(|skin| pick(skin).is_some_and(|path| paths.contains(&path)));
    matching.map(|skin| skin.slot).collect()
}

/// A file a unit names, found.
struct Located {
    source: Source,
    /// The game paths holding its bytes, for a shipped copy; the game's path otherwise.
    paths: Vec<WadHash>,
    own: bool,
    moved: Option<WadHash>,
}

impl Located {
    fn missing() -> Self {
        Self {
            source: Source::Missing,
            paths: Vec::new(),
            own: false,
            moved: None,
        }
    }

    fn named(self, reference: Option<WadHash>, game_slots: BTreeSet<Slot>) -> Named {
        Named {
            reference,
            source: self.source,
            game_slots,
            own: self.own,
            moved: self.moved,
        }
    }
}

impl RigInputs<'_> {
    /// Whether a WAD the client mounts when a champion plays holds a path: the champion's own
    /// WAD, a locale WAD of it, or one mounted in every game.
    fn mounted(&self, champion: &str, path: WadHash) -> bool {
        let mounted = |wad: &crate::model::WadName| {
            let stem = wad.lower().split('.').next().unwrap_or("");
            stem == champion || ALWAYS_MOUNTED.contains(&stem)
        };
        self.env.game().wads(path).iter().any(mounted)
    }

    /// Where the game ships a file it does not ship at `path`, mounted with the champion: in
    /// the format it reads at the same stem (#GAME-CHANGE-0007, #GAME-CHANGE-0008), or, when
    /// the history says the path was the game's, where Riot's renames put it.
    fn moved_to(
        &self,
        champion: &str,
        name: Option<String>,
        path: WadHash,
    ) -> Result<Option<WadHash>, FactsError> {
        let Some(name) = name.and_then(|name| ChunkPath::parse(&name).ok()) else {
            return Ok(None);
        };
        let mut candidates: Vec<ChunkPath> = legacy::twin(&name)
            .map(|(twin, _)| twin)
            .into_iter()
            .collect();
        if self.env.history().path(path)?.is_some() {
            candidates.extend(
                moved::candidates(&name)
                    .into_iter()
                    .map(|candidate| candidate.path),
            );
        }
        let found = candidates
            .iter()
            .map(ChunkPath::hash)
            .find(|hash| self.mounted(champion, *hash));
        Ok(found)
    }

    /// Where a reference of a unit finds its file: in the unit's WAD, else in the game, else
    /// where the game moved it.
    fn locate(
        &self,
        m: &Mod,
        wad: WadId,
        champion: &str,
        file: &value::FileRef,
    ) -> Result<Located, FactsError> {
        let path = file.hash();
        let game = |at: WadHash, moved| Located {
            source: Source::Game,
            paths: vec![at],
            own: false,
            moved,
        };
        if let Some(shipped) = m.file_at(wad, path) {
            return Ok(Located {
                source: Source::Mod(shipped),
                paths: self.files.game_paths(shipped).to_vec(),
                own: !self.files.is_game(shipped),
                moved: None,
            });
        }
        if self.env.game().contains(path) {
            return Ok(game(path, None));
        }
        let name = file
            .spelled()
            .map(str::to_string)
            .or_else(|| self.names.path(path));
        Ok(match self.moved_to(champion, name, path)? {
            Some(at) => game(at, Some(at)),
            None => Located::missing(),
        })
    }

    /// The submeshes of an own mesh against the base's mesh and each candidate premium skin's.
    fn submeshes(
        &self,
        shape: &MeshShape,
        catalog: &SkinCatalog,
        candidates: &BTreeSet<Slot>,
        hidden: &[String],
        graph: Option<&BinObject>,
    ) -> Result<Vec<SubmeshFact>, FactsError> {
        let game = self.env.game();
        let base = catalog.base().and_then(|base| base.mesh);
        let base_shape = match base {
            Some(path) => game.mesh_shape(path)?,
            None => None,
        };
        let mut premium = Vec::new();
        for &slot in candidates.iter().filter(|slot| !slot.is_base()) {
            let mesh = catalog.skin(slot).and_then(|skin| skin.mesh);
            if let Some(path) = mesh.filter(|path| Some(*path) != base)
                && let Some(shape) = game.mesh_shape(path)?
            {
                premium.push((slot, shape));
            }
        }
        let (shown_names, shown_hashes) = graph.map(shown_submeshes).unwrap_or_default();
        let on_base =
            |p: &rig::Rounded| base_shape.as_ref().is_some_and(|base| base.all.contains(p));
        let mut out = Vec::new();
        for submesh in &shape.submeshes {
            let set = &submesh.positions;
            let n = set.len().max(1) as f32;
            let share = |hits: usize| hits as f32 / n;
            let mut on_skins = BTreeMap::new();
            for (slot, skin) in &premium {
                let hits = set
                    .iter()
                    .filter(|p| skin.all.contains(p) && !on_base(p))
                    .count();
                on_skins.insert(*slot, share(hits));
            }
            let lower = submesh.name.to_ascii_lowercase();
            let shown =
                shown_names.contains(&lower) || shown_hashes.contains(&name_hash(&submesh.name).0);
            let visibility = match (hidden.contains(&lower), shown) {
                (false, _) => Visibility::Visible,
                (true, true) => Visibility::HiddenShown,
                (true, false) => Visibility::HiddenNever,
            };
            out.push(SubmeshFact {
                name: Spelled::new(submesh.name.as_str()),
                vertices: set.len(),
                on_base: share(set.iter().filter(|p| on_base(p)).count()),
                on_skins,
                visibility,
            });
        }
        Ok(out)
    }

    /// The rig of one unit with an entry.
    fn rig(
        &self,
        m: &Mod,
        id: UnitId,
        unit: &SkinUnit,
        object: &BinObject,
        catalog: &SkinCatalog,
    ) -> Result<UnitRig, FactsError> {
        let entry = SkinEntry::read(object);
        let wad = m.files[unit.file].wad;
        let champion = &m.characters[m.champion_of(unit.character)].name;
        let mut unreadable = Vec::new();

        // The candidates: the base, the unit's own slot, its origin.
        let mut candidates: BTreeSet<Slot> = [Slot::BASE, unit.source_slot].into_iter().collect();
        candidates.extend(self.origins.premium(id));
        candidates.retain(|slot| catalog.skins.contains_key(slot));

        let pool = load_pool(m, unit);
        let graph = entry.graph().and_then(|link| match pooled(m, &pool, link) {
            Some(object) => Some((&m.objects[object].value, GraphFrom::Archive)),
            None => catalog.object(link).map(|graph| (graph, GraphFrom::Game)),
        });

        let mesh_file = entry.mesh();
        let located = match &mesh_file {
            Some(file) => self.locate(m, wad, champion, file)?,
            None => Located::missing(),
        };
        let mut mesh = MeshFact {
            submeshes: Vec::new(),
            max_blend: None,
            file: Named {
                game_slots: slots_matching(catalog, &located.paths, |skin| skin.mesh),
                ..located.named(
                    mesh_file.as_ref().map(value::FileRef::hash),
                    BTreeSet::new(),
                )
            },
        };
        if let Source::Mod(file) = mesh.file.source {
            match MeshShape::parse(&m.files[file].bytes) {
                Some(shape) => {
                    mesh.max_blend = shape.max_blend;
                    if mesh.file.own {
                        let hidden = entry.hidden_submeshes();
                        mesh.submeshes = self.submeshes(
                            &shape,
                            catalog,
                            &candidates,
                            &hidden,
                            graph.map(|(graph, _)| graph),
                        )?;
                    }
                }
                None => unreadable.push(RigPart::Mesh),
            }
        }

        let skeleton_file = entry.skeleton();
        let located = match &skeleton_file {
            Some(file) => self.locate(m, wad, champion, file)?,
            None => Located::missing(),
        };
        let parsed = match located.source {
            Source::Mod(file) => Skeleton::parse(&m.files[file].bytes).map(Arc::new),
            Source::Game => match located.paths.first() {
                Some(&path) => self.env.game().skeleton(path)?,
                None => None,
            },
            Source::Missing => None,
        };
        // The game's own file is there too, and may not read.
        if parsed.is_none() && located.source != Source::Missing {
            unreadable.push(RigPart::Skeleton);
        }
        let skeleton = SkeletonFact {
            joints: parsed.as_ref().map(|skeleton| skeleton.joints.clone()),
            influences: parsed.as_ref().map(|skeleton| skeleton.influences),
            file: Named {
                game_slots: slots_matching(catalog, &located.paths, |skin| skin.skeleton),
                ..located.named(
                    skeleton_file.as_ref().map(value::FileRef::hash),
                    BTreeSet::new(),
                )
            },
        };

        let mut clips = Vec::new();
        for clip in graph
            .map(|(graph, _)| graph_clips(graph))
            .unwrap_or_default()
        {
            let located = self.locate(m, wad, champion, &value::FileRef::Hash(clip))?;
            let mut paths = located.paths.clone();
            if !paths.contains(&clip) {
                paths.push(clip);
            }
            let plays =
                |skin: &&CatalogSkin| skin.clips.iter().any(|played| paths.contains(played));
            let game_slots = catalog
                .skins
                .values()
                .filter(plays)
                .map(|skin| skin.slot)
                .collect();
            let joints = match located.source {
                Source::Mod(file) if located.own => rig::clip_joints(&m.files[file].bytes),
                _ => None,
            };
            clips.push(ClipFact {
                file: located.named(Some(clip), game_slots),
                joints,
            });
        }

        Ok(UnitRig {
            mesh,
            skeleton,
            graph: graph.map(|(_, from)| from),
            clips,
            unreadable,
        })
    }
}

impl Rigs {
    /// The rigs of a mod's representative units. A unit with no skin entry has none.
    pub fn find(fixed: &FixedMod, inputs: &RigInputs<'_>) -> Result<Self, FactsError> {
        let m: &Mod = fixed;
        let mut out = Self::default();
        for id in inputs.copies.representatives() {
            let unit = &m.units[id];
            let Some(catalog) = inputs.catalogs.of(unit.character) else {
                continue;
            };
            let Some(object) = unit.entry.map(|entry| &m.objects[entry].value) else {
                continue;
            };
            let rig = inputs.rig(m, id, unit, object, catalog)?;
            // Every skin's skeleton, so that a unit is judged by its joints at any slot: a
            // sub-character follows its champion, and a person may force a target.
            for &slot in catalog.skins.keys() {
                let key = (unit.character, slot);
                if out.game.contains_key(&key) {
                    continue;
                }
                let skeleton = match catalog.skin(slot).and_then(|skin| skin.skeleton) {
                    Some(path) => inputs.env.game().skeleton(path)?,
                    None => None,
                };
                out.game.insert(key, skeleton);
            }
            out.units.insert(id, rig);
        }
        Ok(out)
    }

    /// A unit's rig. None for a copy, and for a unit with no skin entry.
    pub fn of(&self, unit: UnitId) -> Option<&UnitRig> {
        self.units.get(unit)
    }

    /// Every unit with a rig, in unit order.
    pub fn iter(&self) -> impl Iterator<Item = (UnitId, &UnitRig)> {
        self.units.iter()
    }

    /// The skeleton of a character's game skin at a slot, when a unit of the character has a
    /// rig and the skeleton reads.
    pub fn game_skeleton(&self, character: CharId, slot: Slot) -> Option<&Skeleton> {
        self.game
            .get(&(character, slot))
            .and_then(|skeleton| skeleton.as_deref())
    }

    /// The joints of a character's game skin at a slot, in order.
    pub fn game_joints(&self, character: CharId, slot: Slot) -> Option<&[String]> {
        self.game_skeleton(character, slot)
            .map(|skeleton| skeleton.joints.as_slice())
    }

    /// Raises what the rigs show: submeshes copied from a premium skin, files that do not
    /// parse.
    pub fn report(&self, sink: &mut Sink<'_>) {
        for (unit, rig) in self.iter() {
            let copied = rig.mesh.submeshes.iter().filter_map(|submesh| {
                Some(CopiedSubmesh {
                    name: submesh.name.clone(),
                    from: submesh.copied_from()?,
                    visibility: submesh.visibility,
                })
            });
            let submeshes: Vec<CopiedSubmesh> = copied.collect();
            if !submeshes.is_empty() {
                sink.raise(SubmeshesCopied { unit, submeshes });
            }
            if !rig.unreadable.is_empty() {
                sink.raise(RigUnreadable {
                    unit,
                    parts: rig.unreadable.clone(),
                });
            }
        }
    }
}
