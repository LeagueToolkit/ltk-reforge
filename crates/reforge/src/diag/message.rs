//! A diagnostic's one line for a person, written from its fields when it is printed, so that a
//! hash a table can name is never printed as a hash.

use std::fmt;

use super::Subject;
use crate::model::entity::{CharId, FileId, Mod, ObjectId, UnitId, WadId};
use crate::model::names::{NoNames, Spell};
use crate::model::{BinHash, WadHash, clean};

/// How many items of a list a message shows before it counts the rest.
pub const SHOWN: usize = 8;

/// What a message is written with: the names that spell hashes, and the mod its ids point into.
pub struct Render<'a> {
    /// Spells hashes.
    pub names: &'a dyn Spell,
    /// The mod, once `load` has made it.
    pub module: Option<&'a Mod>,
}

impl Render<'static> {
    /// Renders with no names and no mod: hashes as hex, ids as printed.
    pub fn plain() -> Self {
        Render {
            names: &NoNames,
            module: None,
        }
    }
}

impl Render<'_> {
    /// A subject as a person reads it.
    pub fn subject_name(&self, subject: &Subject) -> String {
        let Some(m) = self.module else {
            return match subject {
                Subject::Entry(name) => name.to_string(),
                other => other.id(),
            };
        };
        match subject {
            Subject::Run | Subject::Input(_) | Subject::Chunk(_) => subject.id(),
            Subject::Entry(name) => name.to_string(),
            Subject::Wad(id) => m.wads[*id].name.to_string(),
            Subject::File(id) => self.file_name(m, *id),
            Subject::Bin(id) => self.file_name(m, m.bins[*id].file),
            Subject::Object(id) => self.names.entry_or_hex(m.objects[*id].entry),
            Subject::Character(id) => m.characters[*id].name.clone(),
            Subject::Unit(id) => m.unit_name(*id),
        }
    }

    fn file_name(&self, m: &Mod, file: FileId) -> String {
        let f = &m.files[file];
        f.path
            .as_ref()
            .map_or_else(|| self.names.path_or_hex(f.hash), ToString::to_string)
    }
}

/// One piece of a message: text, or an id or a hash spelled when the message is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    /// Text, cleaned where the mod spells it.
    Text(String),
    /// A chunk path, by its hash.
    Path(WadHash),
    /// An entry name, by its hash.
    Entry(BinHash),
    /// A field name, by its hash.
    Field(BinHash),
    /// A class name, by its hash.
    Class(BinHash),
    /// A WAD of the mod, by its name.
    Wad(WadId),
    /// A character, by its name.
    Character(CharId),
    /// A file of the mod, by its path where one is known.
    File(FileId),
    /// An object of the mod, by its entry name where one is known.
    Object(ObjectId),
    /// A unit, as `varus skin0`.
    Unit(UnitId),
}

impl Part {
    /// The piece as a person reads it, its names from `cx`.
    fn write(&self, cx: &Render<'_>) -> String {
        let name = match self {
            Part::Text(text) => return text.clone(),
            Part::Path(hash) => cx.names.path_or_hex(*hash),
            Part::Entry(hash) => cx.names.entry_or_hex(*hash),
            Part::Field(hash) => cx.names.field_or_hex(*hash),
            Part::Class(hash) => cx.names.class_or_hex(*hash),
            Part::Wad(wad) => cx.subject_name(&Subject::Wad(*wad)),
            Part::Character(character) => cx.subject_name(&Subject::Character(*character)),
            Part::File(file) => cx.subject_name(&Subject::File(*file)),
            Part::Object(object) => cx.subject_name(&Subject::Object(*object)),
            Part::Unit(unit) => cx.subject_name(&Subject::Unit(*unit)),
        };
        clean(&name).into_owned()
    }
}

/// The line being written, as parts. It takes text through `write!`, and ids and hashes
/// through its methods. They are spelled when the line is written for a person, with the
/// names known then.
#[derive(Debug, Default)]
pub struct Message {
    parts: Vec<Part>,
}

impl Message {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// The parts written, in order.
    pub(super) fn into_parts(self) -> Vec<Part> {
        self.parts
    }

    /// The line the parts make, its names from `cx`.
    pub(super) fn write(parts: &[Part], cx: &Render<'_>) -> String {
        parts.iter().map(|part| part.write(cx)).collect()
    }

    fn part(&mut self, part: Part) -> &mut Self {
        self.parts.push(part);
        self
    }

    /// Plain text.
    pub fn text(&mut self, text: &str) -> &mut Self {
        match self.parts.last_mut() {
            Some(Part::Text(last)) => last.push_str(text),
            _ => self.parts.push(Part::Text(text.to_string())),
        }
        self
    }

    /// A count with its noun, plural when not one: `1 file`, `3 files`.
    pub fn count(&mut self, n: usize, noun: &str) -> &mut Self {
        let plural = if n == 1 { "" } else { "s" };
        self.text(&format!("{n} {noun}{plural}"))
    }

    /// Text a mod spells, cleaned.
    pub fn spelled(&mut self, text: &str) -> &mut Self {
        self.text(&clean(text))
    }

    /// A chunk path, by its hash.
    pub fn path(&mut self, hash: WadHash) -> &mut Self {
        self.part(Part::Path(hash))
    }

    /// An entry name, by its hash.
    pub fn entry(&mut self, hash: BinHash) -> &mut Self {
        self.part(Part::Entry(hash))
    }

    /// A field name, by its hash.
    pub fn field(&mut self, hash: BinHash) -> &mut Self {
        self.part(Part::Field(hash))
    }

    /// A class name, by its hash.
    pub fn class(&mut self, hash: BinHash) -> &mut Self {
        self.part(Part::Class(hash))
    }

    /// A WAD of the mod, by its name.
    pub fn wad(&mut self, wad: WadId) -> &mut Self {
        self.part(Part::Wad(wad))
    }

    /// A character, by its name.
    pub fn character(&mut self, character: CharId) -> &mut Self {
        self.part(Part::Character(character))
    }

    /// A file of the mod, by its path where one is known.
    pub fn file(&mut self, file: FileId) -> &mut Self {
        self.part(Part::File(file))
    }

    /// An object of the mod, by its entry name where one is known.
    pub fn object(&mut self, object: ObjectId) -> &mut Self {
        self.part(Part::Object(object))
    }

    /// A unit, as `varus skin0`.
    pub fn unit(&mut self, unit: UnitId) -> &mut Self {
        self.part(Part::Unit(unit))
    }

    /// The first items of a list, each written by `each`, then how many more there are.
    pub fn list<T>(&mut self, items: &[T], mut each: impl FnMut(&mut Self, &T)) -> &mut Self {
        for (i, item) in items.iter().take(SHOWN).enumerate() {
            if i > 0 {
                self.text(", ");
            }
            each(self, item);
        }
        if items.len() > SHOWN {
            self.text(&format!(" and {} more", items.len() - SHOWN));
        }
        self
    }
}

impl fmt::Write for Message {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.text(s);
        Ok(())
    }
}
