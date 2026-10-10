//! A port's package (#ADR-0026): for each unit a patch of the game's skin bin at its target,
//! the unit's own bins, and the manifest entry that links them; the files the port ships,
//! under its namespace; the HUD layer (#ADR-0029).
//!
//! Per unit:
//!
//! - a PTCH on the game's `skin<T>.bin`: one record per carried field of the skin entry that
//!   differs from the game's, and the resolver's map laid over the game's, written whole
//!   (#ADR-0006);
//! - a bin of the carried objects, and a bin of the graph when it ships;
//! - for a sub-character the game ships no skin bin of at the target, a whole `skin<T>.bin`
//!   in place of a patch (#ADR-0017);
//! - the audio block: the game's bank units with the port's after them in the unit's patch,
//!   and for each voice layer a patch of its own that sets the whole block (#ADR-0028). The
//!   banks the plan made ship as chunks of their layer.

use std::collections::BTreeSet;
use std::sync::Arc;

use indexmap::IndexMap;
use ltk_meta::property::values;
use ltk_meta::{Bin, BinObject, PropertyKind, PropertyValueEnum};

use super::config::{Made, ProjectConfig};
use super::findings::{
    Empty, EntryWhole, FieldKept, NewBin, NewBinFailed, NotWritten, Ported, ResolverFrozen,
    UnitPorted,
};
use super::inplace::BuildError;
use super::ptch::PtchBuilder;
use super::shaders;
use super::texture::TexContainer;
use super::wired::{LeftBehind, LeftOut, Wired};
use super::{BinWrite, Chunk, ChunkKey, Layer, Origin, Package, Patch, PatchOf, WrittenBin};
use crate::diag::Sink;
use crate::env::Env;
use crate::fixups::FixedMod;
use crate::game::GameBin;
use crate::load::Fault;
use crate::model::entity::{CharKind, FileId, Mod, UnitId};
use crate::model::hash::skin_paths;
use crate::model::known::field;
use crate::model::{BinHash, ChunkPath, Slot, Spell, WadHash, bin, name_hash, path_hash, value};
use crate::port::{
    AudioNotCarried, BankUnit, Base, CARRIED_FIELDS, FileVerdict, ICON_FIELDS, Layout, NotCarried,
    Port, PortUnit,
};

/// Builds a port's package from a fixed mod and its plan.
pub struct PortBuilder<'a> {
    fixed: &'a FixedMod,
    port: &'a Port,
    env: &'a Env,
    names: &'a dyn Spell,
    keep_dds: bool,
}

/// The objects of a bin and the bins it links.
type BinParts = (Vec<BinObject>, Vec<String>);

/// What lowering one unit made.
#[derive(Default)]
struct Lowered {
    /// The bins made, each with its place.
    bins: Vec<(ChunkPath, Vec<BinObject>, Vec<String>)>,
    /// The patch, when the game has a bin to patch.
    patch: Option<Patch>,
    /// The patch of each voice layer the unit's audio goes to (#ADR-0028).
    layers: Vec<(Layer, Patch)>,
    /// The layers whose audio record is written: a bank ships with the record that names it.
    voiced: BTreeSet<Layer>,
}

/// The value of an entry's `bankUnits`, and of its `tagEventList` when the port adds a tag.
type AudioRecord = (PropertyValueEnum, Option<PropertyValueEnum>);

/// What a unit's audio came to in the build.
#[derive(Default)]
struct Voiced {
    /// The patch of each voice layer.
    layers: Vec<(Layer, Patch)>,
    /// The layers whose record is written.
    voiced: BTreeSet<Layer>,
    /// What is not written, and why.
    left: Vec<NotCarried>,
}

/// A list of strings, as an audio block holds them.
fn string_list(items: impl Iterator<Item = String>) -> Option<PropertyValueEnum> {
    let items = items.map(|item| PropertyValueEnum::String(values::String::from(item)));
    let list = values::Container::new(PropertyKind::String, items.collect()).ok()?;
    Some(PropertyValueEnum::Container(list))
}

/// The fields of a bank unit.
fn unit_fields(unit: &mut PropertyValueEnum) -> Option<&mut IndexMap<BinHash, PropertyValueEnum>> {
    match unit {
        PropertyValueEnum::Embedded(embedded) => Some(&mut embedded.0.properties),
        PropertyValueEnum::Struct(held) => Some(&mut held.properties),
        _ => None,
    }
}

/// The game's `bankUnits` with the port's units after them, and the game's tags with the
/// port's after them when there are any. Each unit is modelled on the game's first, so its
/// types are the build's. A game unit the port's unit stands in for is left out
/// (#MOD-ARTIFACT-0028), and so is one of `left_out`, which the mod's own block leaves out
/// (#ADR-0063). None when the game's entry has no bank unit to model one on.
fn audio_record(
    theirs: &BinObject,
    added: &[&BankUnit],
    tags: &[String],
    left_out: &[String],
) -> Option<(PropertyValueEnum, Option<PropertyValueEnum>)> {
    let block = field::SKIN_AUDIO_PROPERTIES;
    let list = value::get(theirs, &[block, field::BANK_UNITS])?;
    let (item_kind, mut items, unordered) = match list {
        PropertyValueEnum::UnorderedContainer(c) => (c.0.item_kind(), c.0.items().to_vec(), true),
        PropertyValueEnum::Container(c) => (c.item_kind(), c.items().to_vec(), false),
        _ => return None,
    };
    let template = items.first()?.clone();
    let name_field = name_hash(field::BANK_UNIT_NAME);
    let replaced = added.iter().filter_map(|u| u.replaces.as_deref());
    let replaced: BTreeSet<&str> = replaced
        .chain(left_out.iter().map(String::as_str))
        .collect();
    items.retain_mut(|item| {
        let name = unit_fields(item).and_then(|fields| fields.get(&name_field));
        let name = name.and_then(value::as_str);
        name.is_none_or(|name| !replaced.contains(name))
    });
    for unit in added {
        let mut item = template.clone();
        let fields = unit_fields(&mut item)?;
        let banks = unit.banks.iter().map(|path| path.as_str().to_string());
        let name = PropertyValueEnum::String(values::String::from(unit.name.clone()));
        fields.insert(name_field, name);
        fields.insert(name_hash(field::BANK_PATH), string_list(banks)?);
        let events = string_list(unit.events.iter().cloned())?;
        fields.insert(name_hash(field::BANK_UNIT_EVENTS), events);
        let voice = name_hash(field::VOICE_OVER);
        let flag = match fields.get(&voice) {
            Some(PropertyValueEnum::BitBool(_)) => {
                PropertyValueEnum::BitBool(values::BitBool::new(unit.voice))
            }
            _ => PropertyValueEnum::Bool(values::Bool::new(unit.voice)),
        };
        fields.insert(voice, flag);
        items.push(item);
    }
    let units = values::Container::new(item_kind, items).ok()?;
    let units = match unordered {
        true => PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(units)),
        false => PropertyValueEnum::Container(units),
    };
    let tags = match tags.is_empty() {
        true => None,
        false => {
            let held = value::get(theirs, &[block, field::TAG_EVENT_LIST]);
            let held = held.map_or(&[][..], value::items).iter();
            let held = held.filter_map(value::as_str).map(str::to_string);
            Some(string_list(held.chain(tags.iter().cloned()))?)
        }
    };
    Some((units, tags))
}

impl<'a> PortBuilder<'a> {
    /// A builder for a plan of a fixed mod. The names spell the fields of a record's path.
    pub fn new(fixed: &'a FixedMod, port: &'a Port, env: &'a Env, names: &'a dyn Spell) -> Self {
        Self {
            fixed,
            port,
            env,
            names,
            keep_dds: false,
        }
    }

    /// Ships each DDS as it is (#ADR-0055).
    #[must_use]
    pub fn keep_dds(mut self, keep: bool) -> Self {
        self.keep_dds = keep;
        self
    }

    /// The package. A port that would hold nothing, or a material the client crashes on, ends
    /// the run.
    pub fn build(self, sink: &mut Sink<'_>) -> Result<Package, BuildError> {
        let m: &Mod = self.fixed;
        let port = self.port;
        let _span = tracing::info_span!("build", units = port.units.len()).entered();
        let game = self.env.game();
        if let Some(missing) = shaders::missing(port.objects.values(), game).map_err(Fault::from)? {
            return Err(sink.halt(missing).into());
        }
        let variants = shaders::variants_missing(port.objects.values(), game);
        if let Some(missing) = variants.map_err(Fault::from)? {
            return Err(sink.halt(missing).into());
        }
        let mut package = Package::new(ProjectConfig::of(m, Made::Ported));
        let mut found = Ported {
            files: 0,
            bins: 0,
            banks: 0,
            patches: 0,
            left: LeftBehind::default(),
        };
        let mut voiced: BTreeSet<(UnitId, Layer)> = BTreeSet::new();
        let shipped: BTreeSet<WadHash> = port.files.values().map(ChunkPath::hash).collect();

        for unit in &port.units {
            let wad = m.wads[m.characters[unit.character].wad].name.clone();
            let Some(lowered) = self.unit(unit, &shipped, sink)? else {
                continue;
            };
            for (path, objects, links) in lowered.bins {
                found.bins += 1;
                package.tables.add_path(path.as_str());
                let key = ChunkKey::new(wad.clone(), path);
                package.insert(Layer::Base, key, made_bin(objects, links));
            }
            if let Some(patch) = lowered.patch {
                found.patches += 1;
                package.patch(Layer::Base, patch);
            }
            for (layer, patch) in lowered.layers {
                found.patches += 1;
                package.patch(layer, patch);
            }
            voiced.extend(lowered.voiced.into_iter().map(|layer| (unit.unit, layer)));
        }
        // #ADR-0039: a bank ships with the record that names it.
        let mut recut: BTreeSet<FileId> = BTreeSet::new();
        for bank in &port.audio.banks {
            if !voiced.contains(&(bank.unit, bank.layer.clone())) {
                continue;
            }
            found.banks += 1;
            recut.insert(bank.from);
            let chunk = Chunk {
                bytes: bank.bank.write().into(),
                origin: Origin::Bank(bank.from),
            };
            let key = ChunkKey::new(bank.wad.clone(), bank.path.clone());
            package.insert(bank.layer.clone(), key, chunk);
        }

        let mut container = TexContainer::new().keep_dds(self.keep_dds);
        let mut ship = |package: &mut Package, layer: Layer, file: FileId, place: &ChunkPath| {
            let node = &m.files[file];
            let chunk = match container.contain(place, &node.bytes) {
                Some(tex) => {
                    container.shipped();
                    Chunk {
                        bytes: tex.into(),
                        origin: Origin::Contained(file),
                    }
                }
                None => Chunk::kept(file, node.bytes.clone()),
            };
            let key = ChunkKey::new(m.wads[node.wad].name.clone(), place.clone());
            package.insert(layer, key, chunk);
        };
        for (&file, place) in &port.files {
            found.files += 1;
            ship(&mut package, Layer::Base, file, place);
        }
        for hud in &port.hud.champions {
            for (file, place) in &hud.files {
                found.files += 1;
                ship(&mut package, Layer::Hud, *file, place);
            }
            let mut ptch = PtchBuilder::new(self.names);
            for edit in &hud.edits {
                // The plan left out every edit on a path no table names.
                let _ = ptch.set(edit.entry, &edit.fields, edit.value.clone());
            }
            let character = &m.characters[hud.character];
            let Some(path) = Layout::hud_patch(&character.name).filter(|_| !ptch.is_empty()) else {
                continue;
            };
            found.patches += 1;
            let patch = Patch {
                target: hud.root.clone(),
                path,
                records: ptch.len(),
                bytes: ptch.bytes().into(),
                links: Vec::new(),
                of: PatchOf::Root {
                    character: hud.character,
                },
            };
            package.patch(Layer::Hud, patch);
        }
        if package.is_empty() {
            let empty = Empty {
                files: m.files.len(),
            };
            return Err(sink.halt(empty).into());
        }
        container.report(sink);
        for path in &port.layout.made.paths {
            package.tables.add_path(path);
        }
        for entry in &port.layout.made.entries {
            package.tables.add_entry(entry);
        }
        found.left = self.left_behind(&recut);
        sink.raise(found);
        Ok(package)
    }

    /// The files of the mod the port leaves out, by the reason (#ADR-0039).
    fn left_behind(&self, recut: &BTreeSet<FileId>) -> LeftBehind {
        let m: &Mod = self.fixed;
        let port = self.port;
        let wired = Wired::by(port.objects.values());
        let hud = port.hud.champions.iter().flat_map(|hud| &hud.files);
        let hud: BTreeSet<FileId> = hud.map(|(file, _)| *file).collect();
        let verdicts = port.refs.units.values().flat_map(|refs| &refs.refs);
        let game: BTreeSet<WadHash> = verdicts
            .filter(|(_, verdict)| matches!(verdict, FileVerdict::Game { .. }))
            .map(|(path, _)| *path)
            .collect();
        let mut left = LeftBehind::default();
        for (id, file) in m.files.iter() {
            if port.files.contains_key(&id) || hud.contains(&id) {
                continue;
            }
            left.count(match wired.judge(file) {
                Err(reason) => reason,
                Ok(()) if recut.contains(&id) => LeftOut::Recut,
                Ok(()) if game.contains(&file.hash) => LeftOut::Game,
                Ok(()) => LeftOut::NotCarried,
            });
        }
        left
    }

    /// The game's skin0 bin of a sub-character with its entry and resolver renamed to a slot:
    /// what a skin bin the game does not ship is written from (#ADR-0017).
    fn sub_bin_at(&self, unit: &PortUnit) -> Result<Option<BinParts>, Fault> {
        let character = &self.fixed.characters[unit.character];
        if character.kind == CharKind::Champion {
            return Ok(None);
        }
        let path = path_hash(&skin_paths::skin_bin(&character.name, Slot::BASE));
        let Some(base) = self.env.game().bin(path)? else {
            return Ok(None);
        };
        let c = &character.spelled;
        let renamed = |name: fn(&str, Slot) -> String| {
            (
                name_hash(&name(c, Slot::BASE)),
                name_hash(&name(c, unit.target)),
            )
        };
        let renames = [
            renamed(skin_paths::skin_entry),
            renamed(skin_paths::resolver_entry),
        ];
        let objects = base.parsed.objects.iter();
        let objects = objects.map(|object| value::rename_hashes(&object.value, &renames));
        Ok(Some((objects.collect(), base.parsed.dependencies.clone())))
    }

    fn unit(
        &self,
        unit: &PortUnit,
        shipped: &BTreeSet<WadHash>,
        sink: &mut Sink<'_>,
    ) -> Result<Option<Lowered>, Fault> {
        let m: &Mod = self.fixed;
        let port = self.port;
        let character = &m.characters[unit.character];
        let t = unit.target;
        let Ok(target) = ChunkPath::parse(&skin_paths::skin_bin(&character.name, t)) else {
            return Ok(None);
        };
        let Some(ours) = port.object(unit.entry) else {
            return Ok(None);
        };
        // The bin the patch applies to, and the base the unit's values are compared with.
        let game: Option<Arc<GameBin>> = self.env.game().bin(target.hash())?;
        let new_bin = match &game {
            Some(_) => None,
            None => self.sub_bin_at(unit)?,
        };
        let (patched, dependencies): (Vec<&BinObject>, &[String]) = match (&game, &new_bin) {
            (Some(game), _) => {
                let objects = game.parsed.objects.iter().map(|object| &object.value);
                (objects.collect(), game.parsed.dependencies.as_slice())
            }
            (None, Some((objects, links))) => (objects.iter().collect(), links.as_slice()),
            (None, None) => {
                let unit = unit.unit;
                sink.raise(NotWritten { unit, target });
                return Ok(None);
            }
        };
        let in_patched = |entry: BinHash| patched.iter().copied().find(|o| o.path_hash == entry);
        let base = Base::at_target(self.env).loaded(&[target.hash()])?;
        let entry_name = name_hash(&skin_paths::skin_entry(&character.spelled, t));
        let theirs = match &new_bin {
            Some(_) => in_patched(entry_name),
            None => base.object(entry_name).map(|object| object.value),
        };
        let mut ptch = PtchBuilder::new(self.names);
        let hashes =
            |names: &[&str]| -> Vec<BinHash> { names.iter().map(|n| name_hash(n)).collect() };
        let (carried, icons) = (hashes(CARRIED_FIELDS), hashes(ICON_FIELDS));
        let mut audio = Voiced::default();

        match theirs {
            Some(theirs) => {
                for (field, value) in &ours.properties {
                    let icon = icons.contains(field) && names_shipped(value, shipped);
                    let wanted = carried.contains(field) || icon;
                    if wanted && theirs.properties.get(field) != Some(value) {
                        // A carried field has a name: the port's own code spells it.
                        let _ = ptch.set(entry_name, &[*field], value.clone());
                    }
                }
                self.audio(unit, (entry_name, theirs), &target, &mut ptch, &mut audio);
                let lacking = carried.iter().filter(|field| {
                    theirs.properties.contains_key(*field) && !ours.properties.contains_key(*field)
                });
                let lacking: Vec<BinHash> = lacking.copied().collect();
                if !lacking.is_empty() {
                    sink.raise(FieldKept {
                        unit: unit.unit,
                        fields: lacking,
                    });
                }
                self.resolver(unit, (entry_name, theirs), &in_patched, &mut ptch, sink);
            }
            None => {
                ptch.object(ours.clone());
                sink.raise(EntryWhole { unit: unit.unit });
            }
        }

        // The unit's own bins, and the links the patched bin lacks.
        let mut bins = Vec::new();
        let mut links: Vec<ChunkPath> = Vec::new();
        let objects_of = |ids: &[crate::model::entity::ObjectId]| -> Vec<BinObject> {
            ids.iter()
                .filter_map(|&id| port.object(id).cloned())
                .collect()
        };
        let mut made = |path: Option<ChunkPath>, objects: Vec<BinObject>| {
            if let Some(path) = path.filter(|_| !objects.is_empty()) {
                links.push(path.clone());
                bins.push((path, objects, Vec::new()));
            }
        };
        let mut objects = objects_of(&unit.carried);
        objects.extend(unit.copies.iter().cloned());
        made(Layout::objects_bin(&unit.dir), objects);
        let graph: Vec<_> = unit.graph.into_iter().collect();
        made(Layout::graph_bin(&unit.dir), objects_of(&graph));
        for link in &unit.game_links {
            let held = dependencies
                .iter()
                .any(|d| d.eq_ignore_ascii_case(link.as_str()));
            if !held && !links.contains(link) {
                links.push(link.clone());
            }
        }

        let records = ptch.len();
        let carried_objects = unit.carried.len();
        let is_new = new_bin.is_some();
        // A bin written whole is a chunk of the package, and no patch of a layer applies
        // over a chunk.
        if is_new && !audio.layers.is_empty() {
            for (layer, _) in audio.layers.drain(..) {
                audio.voiced.remove(&layer);
            }
            audio.left.push(NotCarried::NewBin);
        }
        if !audio.left.is_empty() {
            sink.raise(AudioNotCarried {
                unit: unit.unit,
                left: audio.left,
            });
        }
        let mut lowered = Lowered {
            bins,
            patch: None,
            layers: audio.layers,
            voiced: audio.voiced,
        };
        if let Some((objects, dependencies)) = new_bin {
            // No bin to patch: the game's skin0, renamed, with the records applied, ships
            // whole.
            let mut whole = Bin::new(objects, dependencies);
            let report = ptch.build().apply(&mut whole);
            for link in &links {
                if !whole
                    .dependencies
                    .iter()
                    .any(|d| d.eq_ignore_ascii_case(link.as_str()))
                {
                    whole.dependencies.push(link.as_str().to_string());
                }
            }
            sink.raise(NewBin {
                unit: unit.unit,
                target: target.clone(),
            });
            if !report.skipped.is_empty() {
                sink.raise(NewBinFailed {
                    unit: unit.unit,
                    target: target.clone(),
                    applied: report.applied,
                    skipped: report.skipped.len(),
                });
            }
            let dependencies = std::mem::take(&mut whole.dependencies);
            let objects = whole.objects.into_values().collect();
            lowered.bins.push((target.clone(), objects, dependencies));
        } else if let Some(path) = Layout::skin_patch(&character.name, t) {
            lowered.patch = Some(Patch {
                target: target.clone(),
                path,
                records,
                bytes: ptch.bytes().into(),
                links,
                of: PatchOf::Skin {
                    unit: unit.unit,
                    entry: entry_name,
                },
            });
        }
        sink.raise(UnitPorted {
            unit: unit.unit,
            target,
            records,
            carried: carried_objects,
            graph: unit.graph.is_some(),
            new_bin: is_new,
        });
        Ok(Some(lowered))
    }

    /// The audio block: the game's units with the port's after them, and the tags likewise.
    /// The base layer's units go in the unit's patch. Each voice layer gets a patch of its own
    /// that sets the whole block: the game's units, the base layer's, then the layer's
    /// (#ADR-0028). A game unit the mod's own block leaves out is out of every record
    /// (#ADR-0063).
    fn audio(
        &self,
        unit: &PortUnit,
        (entry_name, theirs): (BinHash, &BinObject),
        target: &ChunkPath,
        ptch: &mut PtchBuilder<'_>,
        out: &mut Voiced,
    ) {
        let Some(added) = self.port.audio.of(unit.unit) else {
            return;
        };
        let block = name_hash(field::SKIN_AUDIO_PROPERTIES);
        let units_path = [block, name_hash(field::BANK_UNITS)];
        let tags_path = [block, name_hash(field::TAG_EVENT_LIST)];
        // The fields of an audio block have names: the port's own code spells them.
        let set = |ptch: &mut PtchBuilder<'_>, (units, tags): AudioRecord| {
            let _ = ptch.set(entry_name, &units_path, units);
            if let Some(tags) = tags {
                let _ = ptch.set(entry_name, &tags_path, tags);
            }
        };
        let of_layer = |layer: &Layer| -> Vec<&BankUnit> {
            let units = added.units.iter().filter(|unit| unit.layer == *layer);
            units.collect()
        };
        let base = of_layer(&Layer::Base);
        if !base.is_empty() || !added.left_out.is_empty() {
            match audio_record(theirs, &base, &added.tags, &added.left_out) {
                Some(record) => {
                    set(ptch, record);
                    out.voiced.insert(Layer::Base);
                }
                None => out
                    .left
                    .push(NotCarried::NoUnitToModel { layer: Layer::Base }),
            }
        }
        let layers: BTreeSet<&Layer> = added.units.iter().map(|unit| &unit.layer).collect();
        let character = &self.fixed.characters[unit.character];
        for layer in layers.into_iter().filter(|layer| **layer != Layer::Base) {
            let mine = of_layer(layer);
            let mut tags = added.tags.clone();
            for tag in mine.iter().filter_map(|unit| unit.tag.as_ref()) {
                if !tags.contains(tag) {
                    tags.push(tag.clone());
                }
            }
            let units: Vec<&BankUnit> = base.iter().copied().chain(mine).collect();
            let record = audio_record(theirs, &units, &tags, &added.left_out);
            let path = Layout::layer_patch(&character.name, unit.target, layer);
            let (Some(record), Some(path)) = (record, path) else {
                let layer = layer.clone();
                out.left.push(NotCarried::NoUnitToModel { layer });
                continue;
            };
            let mut over = PtchBuilder::new(self.names);
            set(&mut over, record);
            let patch = Patch {
                target: target.clone(),
                path,
                records: over.len(),
                bytes: over.bytes().into(),
                links: Vec::new(),
                of: PatchOf::Skin {
                    unit: unit.unit,
                    entry: entry_name,
                },
            };
            out.layers.push((layer.clone(), patch));
            out.voiced.insert(layer.clone());
        }
    }

    /// The resolver: the mod's keys laid over the game's map, which is written whole under
    /// the game's own name (#GAME-BEHAVIOR-0008). Where the game's entry names no resolver
    /// the patched bin holds, the mod's goes in whole.
    fn resolver<'o>(
        &self,
        unit: &PortUnit,
        (entry_name, theirs): (BinHash, &BinObject),
        in_patched: &dyn Fn(BinHash) -> Option<&'o BinObject>,
        ptch: &mut PtchBuilder<'_>,
        sink: &mut Sink<'_>,
    ) {
        let Some(ours) = unit.resolver.and_then(|r| self.port.object(r)) else {
            return;
        };
        let map_field = name_hash(field::RESOURCE_MAP);
        let link_field = name_hash(field::RESOURCE_RESOLVER);
        let game = theirs.properties.get(&link_field).and_then(value::as_link);
        let game = game.and_then(|name| Some((name, in_patched(name)?)));
        match (game, ours.properties.get(&map_field)) {
            (Some((name, game)), Some(PropertyValueEnum::Map(mine))) => {
                let before = match game.properties.get(&map_field) {
                    Some(PropertyValueEnum::Map(map)) => map.entries().to_vec(),
                    _ => Vec::new(),
                };
                let mut entries = before.clone();
                for (key, value) in mine.entries() {
                    match entries.iter_mut().find(|(held, _)| held == key) {
                        Some(slot) => slot.1 = value.clone(),
                        None => entries.push((key.clone(), value.clone())),
                    }
                }
                if entries == before {
                    return;
                }
                let keys = entries.len();
                // The keys and values are of the kinds the mod's map holds, by its own make.
                let Ok(map) = values::Map::new(mine.key_kind(), mine.value_kind(), entries) else {
                    return;
                };
                if ptch
                    .set(name, &[map_field], PropertyValueEnum::Map(map))
                    .is_ok()
                {
                    let unit = unit.unit;
                    sink.raise(ResolverFrozen { unit, keys });
                }
            }
            _ => {
                let link = values::ObjectLink::new(ours.path_hash);
                ptch.object(ours.clone());
                let _ = ptch.set(
                    entry_name,
                    &[link_field],
                    PropertyValueEnum::ObjectLink(link),
                );
            }
        }
    }
}

/// Whether a value names a file the port ships.
fn names_shipped(value: &PropertyValueEnum, shipped: &BTreeSet<WadHash>) -> bool {
    let mut found = false;
    value::walk(value, &mut |v| {
        if let Some(file) = value::as_file(v) {
            found |= shipped.contains(&file.hash());
        }
    });
    found
}

/// A chunk of a bin the port made.
fn made_bin(objects: Vec<BinObject>, links: Vec<String>) -> Chunk {
    let entries = objects.iter().map(|object| object.path_hash).collect();
    let bytes = bin::write(&Bin::new(objects.iter().cloned(), links));
    Chunk {
        bytes: bytes.into(),
        origin: Origin::Made(WrittenBin {
            how: BinWrite::Whole,
            entries,
            edited: objects,
        }),
    }
}
