//! A package's patches applied as LTK Manager's overlay applies them: each layer's manifest
//! is loaded with `ltk_game_data` and its edits applied to the game's bin, the base layer
//! first and each other layer over the base layer's result.
//!
//! The result is what the apply check reads and what a report lists: for each patch the bin
//! after it, object by object.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Arc;

use ltk_game_data::{ApplyDiagnosticKind, NoSchema, Selector};
use ltk_meta::BinObject;

use super::{Layer, Package};
use crate::game::{GameBin, GameError, GameSource, SHADERS_BIN};
use crate::model::{BinHash, ChunkPath, WadHash, WadName, bin, path_hash};

/// How many bins the load of one patched bin follows.
// #ADR-0070: twice the bins read of one game skin, so a skin's chain and the package's own bins
// fit. Measured 2026-10-06: a patched bin and the shader bin link at most 44 bins at 16.19.
pub const MAX_OVERLAY_BINS: usize = 128;

/// An object a patched bin loads.
#[derive(Debug, Clone)]
pub enum Held {
    /// Of the patched bin or of a bin the package ships.
    Package(BinObject),
    /// Of a bin of the game.
    Game(Arc<GameBin>, usize),
}

impl Held {
    /// The object.
    pub fn object(&self) -> &BinObject {
        match self {
            Held::Package(object) => object,
            Held::Game(bin, place) => &bin.parsed.objects[*place].value,
        }
    }
}

/// Everything a patched bin loads: itself, then breadth first the bins it links, the
/// package's or the game's, and the bin of shader definitions.
#[derive(Debug, Clone, Default)]
pub struct Loaded {
    /// The objects, the first under each entry hash.
    pub objects: HashMap<BinHash, Held>,
    /// The entries of the objects the package ships.
    pub shipped: BTreeSet<BinHash>,
    /// The links the patch adds that lead to no bin.
    pub missing_links: Vec<String>,
    /// Whether the load reached [`MAX_OVERLAY_BINS`] with bins left to follow.
    pub capped: bool,
}

/// A bin after a patch.
#[derive(Debug, Clone)]
pub struct AppliedBin {
    /// Its bytes.
    pub bytes: Vec<u8>,
    /// Its objects, in file order.
    pub objects: Vec<BinObject>,
    /// The bins it links.
    pub dependencies: Vec<String>,
    /// The edits the overlay skipped, each as the overlay says it.
    pub skipped: Vec<String>,
    /// The objects the patch changed, each by its place in `objects` with the top-level
    /// fields that differ from the bin before the patch.
    pub changed: Vec<(usize, Vec<BinHash>)>,
    /// What the patched bin loads. For a patch of the base layer.
    pub loaded: Option<Loaded>,
}

/// What applying a patch came to.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// The game has no bin at the patch's target.
    NoGameBin,
    /// The overlay refused the bin or the patch.
    Failed {
        /// Why, as the overlay says it.
        reason: String,
    },
    /// The bin after the patch.
    Applied(Box<AppliedBin>),
}

/// One patch applied.
#[derive(Debug, Clone)]
pub struct AppliedPatch {
    /// The patch's layer.
    pub layer: Layer,
    /// The patch's place among its layer's patches.
    pub index: usize,
    /// The WAD the game reads the patched bin from.
    pub wad: Option<WadName>,
    /// The game's bin, before any patch.
    pub game: Option<Arc<GameBin>>,
    /// What came of it.
    pub outcome: Outcome,
}

/// A package's patches applied to the game's bins.
#[derive(Debug, Clone, Default)]
pub struct Overlay {
    /// The layers whose manifest does not load, each with why.
    pub manifests: Vec<(Layer, String)>,
    /// Every patch applied, the base layer's first.
    pub patches: Vec<AppliedPatch>,
}

/// A bin's objects in file order. None for bytes that are no bin.
fn objects_of(bytes: &[u8]) -> Option<(Vec<BinObject>, Vec<String>)> {
    let parsed = bin::parse(bytes).ok()?;
    let objects = parsed.objects.into_iter().map(|object| object.value);
    Some((objects.collect(), parsed.dependencies))
}

/// The top-level fields of each object of `after` that differ from the object of `before`
/// under its entry.
fn changed(before: &[BinObject], after: &[BinObject]) -> Vec<(usize, Vec<BinHash>)> {
    let old: HashMap<BinHash, &BinObject> = before.iter().map(|o| (o.path_hash, o)).collect();
    let mut out = Vec::new();
    for (place, object) in after.iter().enumerate() {
        let fields: Vec<BinHash> = match old.get(&object.path_hash) {
            Some(held) if held.class_hash == object.class_hash => {
                let fields = object.properties.iter();
                let differing = fields.filter(|(f, v)| held.properties.get(*f) != Some(*v));
                differing.map(|(field, _)| *field).collect()
            }
            _ => object.properties.keys().copied().collect(),
        };
        if !fields.is_empty() {
            out.push((place, fields));
        }
    }
    out
}

impl Overlay {
    /// Applies every patch of a package to the game's bins.
    pub fn apply(package: &Package, game: &dyn GameSource) -> Result<Self, GameError> {
        let _span = tracing::info_span!("overlay").entered();
        let mut out = Self::default();
        // Each target bin as the base layer leaves it, for the other layers to apply over.
        let mut under: BTreeMap<WadHash, (Vec<u8>, Vec<BinObject>)> = BTreeMap::new();
        let base_first = std::iter::once(&Layer::Base);
        let layers = base_first.chain(package.layers().filter(|layer| **layer != Layer::Base));
        for layer in layers {
            let Some(text) = package.manifest(layer) else {
                continue;
            };
            let missing = |_: &str| {
                Err(ltk_game_data::Error::new(
                    ltk_game_data::ErrorKind::InputMissing,
                ))
            };
            let declarations =
                match ltk_game_data::load_declarations("game_data.yaml", &text, missing) {
                    Ok(declarations) => declarations,
                    Err(error) => {
                        out.manifests.push((layer.clone(), error.to_string()));
                        continue;
                    }
                };
            let patches = package.patches(layer);
            for (index, (module, patch)) in declarations.modules.iter().zip(patches).enumerate() {
                let Selector::Target { edits, .. } = &module.selector else {
                    continue;
                };
                let target = patch.target.hash();
                let game_bin = game.bin(target)?;
                let before = match (under.get(&target), &game_bin) {
                    (Some((bytes, objects)), _) => Some((bytes.clone(), objects.clone())),
                    (None, Some(bin)) => {
                        let objects = bin.parsed.objects.iter().map(|o| o.value.clone());
                        Some((bin.bytes.to_vec(), objects.collect()))
                    }
                    (None, None) => None,
                };
                let mut applied = AppliedPatch {
                    layer: layer.clone(),
                    index,
                    wad: game.wad_of(target),
                    game: game_bin,
                    outcome: Outcome::NoGameBin,
                };
                let Some((bytes, objects)) = before else {
                    out.patches.push(applied);
                    continue;
                };
                let read = |path: &ltk_game_data::OverridePath| {
                    let ours = path.as_str() == patch.path.as_str();
                    let bytes = if ours {
                        patch.bytes.to_vec()
                    } else {
                        Vec::new()
                    };
                    Ok::<_, ltk_game_data::Error>(bytes)
                };
                applied.outcome =
                    match ltk_game_data::apply(&bytes, edits, read, |_| Ok(None), &NoSchema) {
                        Err(error) => Outcome::Failed {
                            reason: error.to_string(),
                        },
                        Ok(result) => {
                            let problems = result.diagnostics.iter();
                            let problems =
                                problems.filter(|d| d.kind != ApplyDiagnosticKind::SchemaFallback);
                            let skipped = problems
                                .map(|d| format!("{:?} {}", d.kind, d.path))
                                .collect();
                            let after = objects_of(&result.bytes).map(|(objects, _)| objects);
                            let after = after.unwrap_or_default();
                            let mut bin = AppliedBin {
                                changed: changed(&objects, &after),
                                bytes: result.bytes,
                                objects: after,
                                dependencies: result.dependencies,
                                skipped,
                                loaded: None,
                            };
                            if *layer == Layer::Base {
                                let links: Vec<&ChunkPath> = patch.links.iter().collect();
                                bin.loaded = Some(Self::load(package, game, &bin, &links)?);
                                under.insert(target, (bin.bytes.clone(), bin.objects.clone()));
                            }
                            Outcome::Applied(Box::new(bin))
                        }
                    };
                out.patches.push(applied);
            }
        }
        Ok(out)
    }

    /// Everything a patched bin loads.
    fn load(
        package: &Package,
        game: &dyn GameSource,
        bin: &AppliedBin,
        added: &[&ChunkPath],
    ) -> Result<Loaded, GameError> {
        let mut loaded = Loaded::default();
        for object in &bin.objects {
            let held = Held::Package(object.clone());
            loaded.objects.entry(object.path_hash).or_insert(held);
        }
        let mut queue: VecDeque<String> = bin.dependencies.iter().cloned().collect();
        queue.push_back(SHADERS_BIN.to_string());
        let mut visited: BTreeSet<WadHash> = BTreeSet::new();
        while let Some(link) = queue.pop_front() {
            let hash = path_hash(&link);
            if visited.contains(&hash) {
                continue;
            }
            if visited.len() >= MAX_OVERLAY_BINS {
                loaded.capped = true;
                continue;
            }
            visited.insert(hash);
            if let Some(chunk) = package.by_hash(hash) {
                let Some((objects, links)) = objects_of(&package.bytes(chunk)) else {
                    continue;
                };
                for object in objects {
                    loaded.shipped.insert(object.path_hash);
                    let entry = loaded.objects.entry(object.path_hash);
                    entry.or_insert(Held::Package(object));
                }
                queue.extend(links);
                continue;
            }
            match game.bin(hash)? {
                Some(held) => {
                    for (place, object) in held.parsed.objects.iter().enumerate() {
                        let entry = loaded.objects.entry(object.value.path_hash);
                        entry.or_insert_with(|| Held::Game(Arc::clone(&held), place));
                    }
                    queue.extend(held.parsed.dependencies.iter().cloned());
                }
                None if added
                    .iter()
                    .any(|path| path.as_str().eq_ignore_ascii_case(&link)) =>
                {
                    loaded.missing_links.push(link);
                }
                None => {}
            }
        }
        Ok(loaded)
    }

    /// The outcome of the patch at a place of a layer.
    pub fn of(&self, layer: &Layer, index: usize) -> Option<&AppliedPatch> {
        let mut patches = self.patches.iter();
        patches.find(|patch| patch.layer == *layer && patch.index == index)
    }

    /// The outcome of each patch of the package, in the package's order
    /// ([`Package::all_patches`]): none for a patch of a layer whose manifest does not load.
    pub fn in_order<'a>(&'a self, package: &'a Package) -> Vec<Option<&'a AppliedPatch>> {
        let mut at: BTreeMap<&Layer, usize> = BTreeMap::new();
        let patches = package.all_patches().map(|(layer, _)| {
            let index = at.entry(layer).or_default();
            let applied = self.of(layer, *index);
            *index += 1;
            applied
        });
        patches.collect()
    }
}
