//! The spell icons and HUD textures a mod overrides (#ADR-0029). The champion's root bin names
//! them and no skin's bin does (#GAME-BEHAVIOR-0004), so they show for the champion under
//! every skin. Each such file ships under the namespace, and the root bin's fields that name
//! it are edited to name the shipped copy.
//!
//! An edit replaces the deepest field that changed, never a whole spell, so that the game's
//! later changes to the rest still show.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ltk_meta::{BinObject, PropertyValueEnum};
use serde::Serialize;

use super::findings::{HudCarried, entry_hex, hashes_hex_list};
use super::layout::Layout;
use super::plan::{PlanError, PortInputs};
use crate::diag::Sink;
use crate::load::{LoadOptions, Staleness, legacy};
use crate::model::entity::{CharId, CharKind, FileId, FileKind, FileSource, Mod, UnitId};
use crate::model::hash::skin_paths;
use crate::model::{
    BinHash, ChunkPath, Dump, DumpCx, Fields, Spell, WadHash, clean, path_hash, value,
};

/// One field of a game object set to a value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldEdit {
    /// The object's entry hash.
    #[serde(serialize_with = "entry_hex")]
    pub entry: BinHash,
    /// The field, as the hashes of the fields on the way to it, through structs.
    #[serde(serialize_with = "hashes_hex_list")]
    pub fields: Vec<BinHash>,
    /// The value.
    #[serde(skip)]
    pub value: PropertyValueEnum,
}

/// The HUD edits of one champion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HudEdits {
    /// The champion.
    pub character: CharId,
    /// The first of its units with a target.
    pub unit: UnitId,
    /// The root bin the edits are on.
    pub root: ChunkPath,
    /// The edits.
    pub edits: Vec<FieldEdit>,
    /// The files shipped, each with its place.
    pub files: Vec<(FileId, ChunkPath)>,
    /// How many objects of the root bin name one of them.
    pub objects: usize,
    /// The objects left out: a field on the way to the file is one no table names (#ADR-0031).
    #[serde(serialize_with = "hashes_hex_list")]
    pub unspelled: Vec<BinHash>,
}

/// The HUD edits of a port, champion by champion.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct HudPlan {
    /// By champion.
    pub champions: Vec<HudEdits>,
}

/// The deepest fields of `ours` that differ from `theirs`, through structs of one class: each
/// as the fields on the way to it and the new value. A list or a map that changed is one
/// field.
fn changed_fields(
    theirs: &Fields,
    ours: &Fields,
    path: &mut Vec<BinHash>,
    out: &mut Vec<(Vec<BinHash>, PropertyValueEnum)>,
) {
    use PropertyValueEnum as V;
    for (field, new) in ours {
        let Some(old) = theirs.get(field).filter(|old| *old != new) else {
            continue;
        };
        path.push(*field);
        match (old, new) {
            (V::Struct(a), V::Struct(b)) if a.class_hash == b.class_hash => {
                changed_fields(&a.properties, &b.properties, path, out);
            }
            (V::Embedded(a), V::Embedded(b)) if a.0.class_hash == b.0.class_hash => {
                changed_fields(&a.0.properties, &b.0.properties, path, out);
            }
            _ => out.push((path.clone(), new.clone())),
        }
        path.pop();
    }
}

impl HudPlan {
    /// The edits of each champion's root bin for the files of the mod that override what it
    /// names. `shipped` are the files the port ships already.
    pub fn find(
        inputs: &PortInputs<'_>,
        layout: &mut Layout,
        shipped: &BTreeSet<FileId>,
        sink: &mut Sink<'_>,
    ) -> Result<Self, PlanError> {
        let m: &Mod = inputs.fixed;
        let game = inputs.env.game();
        let options = LoadOptions::default().keep_stale(inputs.options.keep_stale);
        let stale = Staleness::new(inputs.env, &options);
        // The champions the port writes, each with the first of its units that has a target.
        let mut champions: BTreeMap<CharId, UnitId> = BTreeMap::new();
        for (unit, node) in m.units.iter() {
            let champion = m.characters[node.character].kind == CharKind::Champion;
            if champion && inputs.targets.target(unit).is_some() {
                champions.entry(node.character).or_insert(unit);
            }
        }
        let mut out = Self::default();
        for (character, unit) in champions {
            let node = &m.characters[character];
            let Ok(root) = ChunkPath::parse(&skin_paths::root_bin(&node.name)) else {
                continue;
            };
            let Some(root_bin) = game.bin(root.hash())? else {
                continue;
            };
            // Each path the root bin names, and the path the client reads it at.
            let mut named: BTreeMap<WadHash, Option<ChunkPath>> = BTreeMap::new();
            for object in &root_bin.parsed.objects {
                for file in value::file_refs(&object.value) {
                    let spelled = file.spelled().map(str::to_string);
                    let spelled = spelled.or_else(|| inputs.names.path(file.hash()));
                    let spelled = spelled.and_then(|path| ChunkPath::parse(&path).ok());
                    if let Some((twin, _)) = spelled.as_ref().and_then(legacy::twin) {
                        named.entry(twin.hash()).or_insert(Some(twin));
                    }
                    named.entry(file.hash()).or_insert(spelled);
                }
            }
            // The mod's files there with bytes of its own, which nothing ships yet.
            let mut moved: BTreeMap<WadHash, (FileId, ChunkPath)> = BTreeMap::new();
            for &file in &m.wads[node.wad].files {
                let f = &m.files[file];
                let taken = f.kind == FileKind::Bin || f.source == FileSource::Game;
                if shipped.contains(&file) || taken || !named.contains_key(&f.hash) {
                    continue;
                }
                // Other bytes than the game's at the path, and no stale copy of its older ones.
                let overrides = game.contains(f.hash)
                    && !game.holds(f.hash, f.size, f.content)?
                    && !stale.of(m, file)?;
                if !overrides {
                    continue;
                }
                let name = f.path.clone().or_else(|| {
                    let spelled = inputs.names.path(f.hash)?;
                    ChunkPath::parse(&spelled).ok()
                });
                let name = name.or_else(|| named.get(&f.hash).cloned().flatten());
                let leaf = name.as_ref().and_then(|name| name.parts().last());
                if let Some(place) = leaf.and_then(|leaf| layout.place_hud(&node.name, leaf)) {
                    moved.insert(f.hash, (file, place));
                }
            }
            if moved.is_empty() {
                continue;
            }
            // A reference to a file the mod overrides names the shipped copy, and so does a
            // reference in a dropped format whose twin the mod overrides.
            let to = |hash: WadHash| -> Option<String> {
                let direct = moved.get(&hash);
                let through_twin = || {
                    let spelled = named.get(&hash)?.as_ref()?;
                    let (twin, _) = legacy::twin(spelled)?;
                    moved.get(&twin.hash())
                };
                let (_, place) = direct.or_else(through_twin)?;
                Some(place.as_str().to_string())
            };
            let mut edits = Vec::new();
            let (mut objects, mut unspelled) = (0, Vec::new());
            let mut used: BTreeSet<WadHash> = BTreeSet::new();
            for object in &root_bin.parsed.objects {
                let mut ours: BinObject = object.value.clone();
                if value::rewrite_files(&mut ours, &to) == 0 {
                    continue;
                }
                objects += 1;
                let mut fields = Vec::new();
                let theirs = &object.value.properties;
                changed_fields(theirs, &ours.properties, &mut Vec::new(), &mut fields);
                for (fields, new) in fields {
                    if fields
                        .iter()
                        .any(|field| inputs.names.field(*field).is_none())
                    {
                        unspelled.push(ours.path_hash);
                        continue;
                    }
                    value::walk(&new, &mut |v| {
                        if let Some(file) = value::as_file(v) {
                            used.insert(file.hash());
                        }
                    });
                    edits.push(FieldEdit {
                        entry: ours.path_hash,
                        fields,
                        value: new,
                    });
                }
            }
            if edits.is_empty() {
                continue;
            }
            let files: Vec<(FileId, ChunkPath)> = moved
                .into_values()
                .filter(|(_, place)| used.contains(&path_hash(place.as_str())))
                .collect();
            sink.raise(HudCarried {
                character,
                files: files.len(),
                objects,
                records: edits.len(),
                unspelled: unspelled.len(),
            });
            out.champions.push(HudEdits {
                character,
                unit,
                root,
                edits,
                files,
                objects,
                unspelled,
            });
        }
        Ok(out)
    }

    /// Whether no champion has an edit.
    pub fn is_empty(&self) -> bool {
        self.champions.is_empty()
    }
}

impl Dump for HudPlan {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = String::new();
        for hud in &self.champions {
            let name = cx.module.map_or(String::new(), |m| {
                format!(" {}", clean(&m.characters[hud.character].name))
            });
            let _ = writeln!(
                out,
                "hud {}{name} {} edits={} files={} objects={} unspelled={}",
                hud.character,
                hud.root,
                hud.edits.len(),
                hud.files.len(),
                hud.objects,
                hud.unspelled.len()
            );
        }
        out
    }
}
