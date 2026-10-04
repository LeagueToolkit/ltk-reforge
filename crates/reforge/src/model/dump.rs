//! A stage's result as text, for tests and for a person debugging a run. A dump holds ids, names
//! and counts in creation order, and no address, clock or machine path, so two runs over the
//! same inputs print the same text.

use std::fmt::Write as _;

use super::entity::{CharKind, FileKind, FileSource, LinkTo, Mod, WadLayout, WadRole};
use super::names::Spell;
use super::text::clean;

/// What a dump is written with.
pub struct DumpCx<'a> {
    /// Spells the hashes no path or name of the mod spells.
    pub names: &'a dyn Spell,
    /// Lists every file. Without it only bins, meshes, skeletons and clips are listed.
    pub every_file: bool,
    /// The mod the ids of a later stage's result point into.
    pub module: Option<&'a Mod>,
}

impl<'a> DumpCx<'a> {
    /// A dump with these names, listing bins, meshes, skeletons and clips.
    pub fn new(names: &'a dyn Spell) -> Self {
        Self {
            names,
            every_file: false,
            module: None,
        }
    }

    /// Gives the mod a result's ids point into.
    #[must_use]
    pub fn of(mut self, module: &'a Mod) -> Self {
        self.module = Some(module);
        self
    }

    /// Lists every file.
    #[must_use]
    pub fn every_file(mut self) -> Self {
        self.every_file = true;
        self
    }
}

/// A stage's result as text.
pub trait Dump {
    /// The text. Every name a mod spells is cleaned in it.
    fn dump(&self, cx: &DumpCx<'_>) -> String;
}

impl Mod {
    /// A file as a person reads it: its path, else its hash as a table spells it, else hex.
    /// Cleaned: a table's name is not the mod's, and a path learnt from a mod's bin may be.
    pub fn file_name(&self, file: super::entity::FileId, names: &dyn Spell) -> String {
        let node = &self.files[file];
        match &node.path {
            Some(path) => path.to_string(),
            None => clean(&names.path_or_hex(node.hash)).into_owned(),
        }
    }
}

impl Dump for Mod {
    fn dump(&self, cx: &DumpCx<'_>) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "; reforge mod={} archive=\"{}\"",
            self.meta.mod_id, self.meta.file_name
        );
        for (id, wad) in self.wads.iter() {
            let role = match &wad.role {
                WadRole::Champion { champion } => format!("champion({})", clean(champion)),
                WadRole::Locale { champion, locale } => {
                    format!("locale({}, {locale})", clean(champion))
                }
                WadRole::Other => "other".to_string(),
            };
            let layout = match wad.layout {
                WadLayout::Loose => "loose",
                WadLayout::Packed => "packed",
            };
            let _ = write!(
                out,
                "wad {id} \"{}\" {role} {layout} files={}",
                wad.name,
                wad.files.len()
            );
            if let Some(locale) = &wad.packed_locale {
                let _ = write!(out, " packed-locale={locale}");
            }
            if self.restorations.contains(&id) {
                out.push_str(" restoration");
            }
            out.push('\n');
        }
        for (id, file) in self.files.iter() {
            let listed = matches!(
                file.kind,
                FileKind::Bin | FileKind::Ptch | FileKind::Skn | FileKind::Skl | FileKind::Anm
            );
            if !cx.every_file && !listed {
                continue;
            }
            let _ = write!(
                out,
                "file {id} {} {:016x} {} {} {}B sha={}",
                file.wad,
                file.hash.0,
                file.kind.as_str(),
                file.used.as_str(),
                file.size,
                &file.content.hex()[..16]
            );
            match file.source {
                FileSource::Archive => {}
                FileSource::Made { from, by } => {
                    let _ = write!(out, " made({from}, {})", by.as_str());
                }
                FileSource::Rewritten { by, .. } => {
                    let _ = write!(out, " rewritten({})", by.as_str());
                }
                FileSource::Game => out.push_str(" game"),
            }
            let _ = writeln!(out, "\n     {}", self.file_name(id, cx.names));
        }
        for (id, bin) in self.bins.iter() {
            if let Some(error) = &bin.error {
                let _ = writeln!(out, "bin {id} {} unreadable: {}", bin.file, clean(error));
                continue;
            }
            let _ = writeln!(
                out,
                "bin {id} {} objects={} links={}",
                bin.file,
                bin.objects.len(),
                bin.links.len()
            );
            for link in &bin.links {
                let to = match link.to {
                    LinkTo::Mod(file) => file.to_string(),
                    LinkTo::Game => "game".to_string(),
                    LinkTo::Missing => "missing".to_string(),
                };
                let _ = writeln!(out, "     links {} -> {to}", link.path);
            }
        }
        for (id, character) in self.characters.iter() {
            let kind = match character.kind {
                CharKind::Champion => "champion".to_string(),
                CharKind::Sub { champion } => format!("sub of {champion}"),
            };
            let units: Vec<String> = character.units.iter().map(ToString::to_string).collect();
            let _ = write!(
                out,
                "char {id} {} ({}) {kind} units=[{}]",
                clean(&character.name),
                clean(&character.spelled),
                units.join(" ")
            );
            if self.equal_to_game.contains(&id) {
                out.push_str(" equal-to-game");
            }
            out.push('\n');
        }
        for (id, unit) in self.units.iter() {
            let entry = unit.entry.map_or_else(
                || "-".to_string(),
                |object| {
                    let name = cx.names.entry_or_hex(self.objects[object].entry);
                    format!("{object} \"{}\"", clean(&name))
                },
            );
            let bin = unit
                .bin
                .map_or_else(|| "-".to_string(), |bin| bin.to_string());
            let _ = writeln!(
                out,
                "unit {id} {} slot={} file={} bin={bin} loads={} entry={entry}",
                unit.character,
                unit.source_slot,
                unit.file,
                unit.loads.len()
            );
        }
        out
    }
}
