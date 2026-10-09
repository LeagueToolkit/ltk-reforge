//! What the client loads. Mods carry stale bins, working copies under old names and files
//! nothing uses, and only what the client would load is read.
//!
//! - **Roots.** Each unit's skin bin, and every bin the mod places at a path the game ships,
//!   since the game's own bins link it.
//! - **Links.** A bin's links are followed breadth first, up to [`MAX_LOAD_BINS`] bins per
//!   root. Each path leads to the mod's file (in the same WAD, else in any), else to the
//!   game's, else to nothing. A link to the game's bin is not followed.
//! - **A unit's load** is its skin bin and the mod's bins it reaches, in that order.
//! - **Files.** Each file gets its [`FileUse`].

use std::collections::BTreeSet;

use super::findings::{
    BinUnreadable, EntryDuplicates, FormerPathFiles, LinksMissing, LoadCapped, MissingLink,
    StaleFiles, UnusedFiles,
};
use super::stale::Staleness;
use super::{Fault, MAX_LOAD_BINS, Run};
use crate::diag::Sink;
use crate::history::HistoryError;
use crate::model::entity::{
    BinId, BinNode, CharId, FileId, FileUse, Link, LinkTo, ObjectId, ObjectNode, UnitId,
};
use crate::model::hash::skin_paths;
use crate::model::{
    MAX_SLOT, Mod, SkinEntry, Slot, Spell, Spelled, WadHash, bin, name_hash, path_hash, value,
};

/// The step that reads the bins the client loads and marks every file's use.
pub(super) struct ClientLoad<'a> {
    run: Run<'a>,
}

impl<'a> ClientLoad<'a> {
    pub(super) fn new(run: Run<'a>) -> Self {
        Self { run }
    }

    pub(super) fn run(mut self, sink: &mut Sink<'_>) -> Result<(), Fault> {
        self.load_units(sink);
        self.load_at_game_paths(sink);
        self.mark_files()?;
        self.report_files(sink);
        self.report_links(sink)?;
        self.spell_characters();
        Ok(())
    }

    /// The bin of a file, parsed once, with its links resolved.
    fn bin_of(&mut self, file: FileId, sink: &mut Sink<'_>) -> BinId {
        let env = self.run.env;
        let book = &mut *self.run.book;
        let module = &mut *self.run.module;
        if let Some(bin) = module.bin_of(file) {
            return bin;
        }
        let parsed = bin::parse(&module.files[file].bytes);
        let bin = module.push_bin(BinNode {
            file,
            links: Vec::new(),
            objects: Vec::new(),
            error: None,
        });
        match parsed {
            Ok(parsed) => {
                let wad = module.files[file].wad;
                for path in parsed.dependencies {
                    let hash = book.learn_path(&path);
                    let to = match module.find_file(wad, hash) {
                        Some(file) => LinkTo::Mod(file),
                        None if env.game().contains(hash) => LinkTo::Game,
                        None => LinkTo::Missing,
                    };
                    module.bins[bin].links.push(Link {
                        path: Spelled::new(path),
                        to,
                    });
                }
                if !parsed.duplicates.is_empty() {
                    sink.raise(EntryDuplicates {
                        bin,
                        entries: parsed.duplicates,
                    });
                }
                for object in parsed.objects {
                    let id = module.objects.push(ObjectNode {
                        bin,
                        entry: object.value.path_hash,
                        class: object.value.class_hash,
                        raw: object.raw,
                        body: object.body,
                        value: object.value,
                    });
                    module.bins[bin].objects.push(id);
                }
            }
            Err(why) => {
                module.bins[bin].error = Some(why.to_string());
                sink.raise(BinUnreadable { bin, why });
            }
        }
        module.files[file].used = FileUse::Loaded;
        bin
    }

    /// The readable bins loaded from a root, breadth first through the links the mod answers.
    fn load_from(&mut self, root: FileId, sink: &mut Sink<'_>) -> Vec<BinId> {
        let mut order = vec![self.bin_of(root, sink)];
        let mut next = 0;
        while next < order.len() && order.len() < MAX_LOAD_BINS {
            let links = self.run.module.bins[order[next]].links.iter();
            let linked: Vec<FileId> = links
                .filter_map(|link| match link.to {
                    LinkTo::Mod(file) => Some(file),
                    LinkTo::Game | LinkTo::Missing => None,
                })
                .collect();
            for file in linked {
                // A bin past the cap is not read: its objects are not the load's.
                if order.len() >= MAX_LOAD_BINS {
                    break;
                }
                let bin = self.bin_of(file, sink);
                if !order.contains(&bin) {
                    order.push(bin);
                }
            }
            next += 1;
        }
        if order.len() >= MAX_LOAD_BINS {
            let m = &self.run.module;
            let linked = order.iter().flat_map(|&bin| &m.bins[bin].links);
            let left: BTreeSet<FileId> = linked
                .filter_map(|link| match link.to {
                    LinkTo::Mod(file) => Some(file),
                    LinkTo::Game | LinkTo::Missing => None,
                })
                .filter(|&file| m.bin_of(file).is_none_or(|bin| !order.contains(&bin)))
                .collect();
            if !left.is_empty() {
                sink.raise(LoadCapped {
                    bin: order[0],
                    cap: MAX_LOAD_BINS,
                    left: left.len(),
                });
            }
        }
        order.retain(|&bin| self.run.module.bins[bin].error.is_none());
        order
    }

    fn load_units(&mut self, sink: &mut Sink<'_>) {
        let units: Vec<UnitId> = self.run.module.units.ids().collect();
        for unit in units {
            let file = self.run.module.units[unit].file;
            let loads = self.load_from(file, sink);
            let m = &mut *self.run.module;
            let bin = m.bin_of(file).filter(|&bin| m.bins[bin].error.is_none());
            let (entry, entry_name_slot) = Self::pick_entry(m, &loads, unit);
            let unit = &mut m.units[unit];
            unit.bin = bin;
            unit.loads = loads;
            unit.entry = entry;
            unit.entry_name_slot = entry_name_slot;
        }
    }

    /// The skin entry of a unit: the one under the slot's own name anywhere in its load, else
    /// the first of the skin class in load order; and the slot its name spells. A repathed mod
    /// may leave the skin bin a list of links and keep the entry in a linked bin.
    fn pick_entry(m: &Mod, loads: &[BinId], unit: UnitId) -> (Option<ObjectId>, Option<Slot>) {
        let character = &m.characters[m.units[unit].character].name;
        let name_of = |slot: Slot| name_hash(&skin_paths::skin_entry(character, slot));
        let own = name_of(m.units[unit].source_slot);
        let skin_class = SkinEntry::class();
        let named = loads.iter().find_map(|&bin| m.object_in(bin, own));
        let entry = named.or_else(|| {
            let objects = loads.iter().flat_map(|&bin| &m.bins[bin].objects);
            objects.copied().find(|&o| m.objects[o].class == skin_class)
        });
        let name_slot = entry.and_then(|object| {
            let entry = m.objects[object].entry;
            (0..=MAX_SLOT)
                .map(Slot)
                .find(|&slot| name_of(slot) == entry)
        });
        (entry, name_slot)
    }

    /// Bins at paths the game ships are loaded whenever the game's bins link them.
    fn load_at_game_paths(&mut self, sink: &mut Sink<'_>) {
        let game = self.run.env.game();
        let files = self.run.module.files.iter();
        let roots: Vec<FileId> = files
            .filter(|(_, file)| {
                file.kind.is_bin() && file.used == FileUse::Unused && game.contains(file.hash)
            })
            .map(|(id, _)| id)
            .collect();
        for root in roots {
            self.load_from(root, sink);
        }
    }

    /// Whether the game shipped a file at a path at some build.
    fn former_path(&self, path: WadHash) -> Result<bool, HistoryError> {
        Ok(self.run.env.history().path(path)?.is_some())
    }

    fn mark_files(&mut self) -> Result<(), Fault> {
        let env = self.run.env;
        let book = &mut *self.run.book;
        let module = &mut *self.run.module;
        let mut referenced: BTreeSet<FileId> = BTreeSet::new();
        for object in module.objects.values() {
            let wad = module.files[module.bins[object.bin].file].wad;
            for named in value::file_refs(&object.value) {
                // A path string spells the file it names, which a packed WAD does not.
                if let Some(path) = named.spelled() {
                    book.learn_path(path);
                }
                if let Some(file) = module.find_file(wad, named.hash()) {
                    referenced.insert(file);
                }
            }
        }
        let game = env.game();
        let stale = Staleness::new(env, self.run.options);
        let files: Vec<FileId> = module.files.ids().collect();
        for file in files {
            let node = &module.files[file];
            if node.used != FileUse::Unused {
                continue;
            }
            // #ADR-0023: a stale copy is stale whatever names it.
            let used = if stale.of(module, file)? {
                FileUse::Stale
            } else if referenced.contains(&file) {
                FileUse::Referenced
            } else if game.contains(node.hash) {
                FileUse::Override
            } else if env.history().path(node.hash)?.is_some() {
                FileUse::FormerPath
            } else {
                FileUse::Unused
            };
            module.files[file].used = used;
        }
        Ok(())
    }

    /// One finding per WAD and use: the stale copies, the files at former paths, the files
    /// nothing loads.
    fn report_files(&self, sink: &mut Sink<'_>) {
        let m = &*self.run.module;
        for (wad, node) in m.wads.iter() {
            let with = |used: FileUse| -> Vec<FileId> {
                let files = node.files.iter().copied();
                files.filter(|&file| m.files[file].used == used).collect()
            };
            let size = |files: &[FileId]| files.iter().map(|&file| m.files[file].size).sum();
            let files = with(FileUse::Stale);
            if !files.is_empty() {
                let bytes = size(&files);
                sink.raise(StaleFiles { wad, files, bytes });
            }
            let files = with(FileUse::FormerPath);
            if !files.is_empty() {
                let bytes = size(&files);
                sink.raise(FormerPathFiles { wad, files, bytes });
            }
            let files = with(FileUse::Unused);
            if !files.is_empty() {
                let bytes = size(&files);
                let bins = files.iter().copied();
                let bins = bins.filter(|&file| m.files[file].kind.is_bin()).collect();
                sink.raise(UnusedFiles {
                    wad,
                    files,
                    bytes,
                    bins,
                });
            }
        }
    }

    /// One finding per bin: the links nothing answers.
    fn report_links(&self, sink: &mut Sink<'_>) -> Result<(), HistoryError> {
        for (bin, node) in self.run.module.bins.iter() {
            let missing = node.links.iter().filter(|link| link.to == LinkTo::Missing);
            let mut links = Vec::new();
            for link in missing {
                links.push(MissingLink {
                    path: link.path.clone(),
                    former: self.former_path(path_hash(link.path.raw()))?,
                });
            }
            if !links.is_empty() {
                sink.raise(LinksMissing { bin, links });
            }
        }
        Ok(())
    }

    /// The spelling entry names use for each character, from its skin entry's name where the
    /// names spell it. The names of every slot's skin entry, resolver and graph are learnt, so
    /// that a later stage spells them whatever tables are installed.
    fn spell_characters(&mut self) {
        let characters: Vec<CharId> = self.run.module.characters.ids().collect();
        for character in characters {
            if let Some(spelled) = self.spelling(character) {
                self.run.module.characters[character].spelled = spelled;
            }
            let spelled = &self.run.module.characters[character].spelled;
            for slot in (0..=MAX_SLOT).map(Slot) {
                let book = &mut *self.run.book;
                book.learn_entry(&skin_paths::skin_entry(spelled, slot));
                book.learn_entry(&skin_paths::resolver_entry(spelled, slot));
                book.learn_entry(&skin_paths::graph_entry(spelled, slot));
            }
        }
    }

    fn spelling(&self, character: CharId) -> Option<String> {
        let m = &*self.run.module;
        let speller = self.run.speller();
        let node = &m.characters[character];
        let entries = node.units.iter().filter_map(|&unit| m.units[unit].entry);
        let name = entries
            .filter_map(|object| speller.entry(m.objects[object].entry))
            .next()?;
        let spelled = name.strip_prefix("Characters/")?.split('/').next()?;
        spelled
            .eq_ignore_ascii_case(&node.name)
            .then(|| spelled.to_string())
    }
}
