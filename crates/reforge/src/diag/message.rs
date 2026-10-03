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

/// The line being written. It takes text through `write!`, and ids and hashes through its
/// methods, which spell them.
pub struct Message<'a> {
    out: String,
    cx: &'a Render<'a>,
}

impl<'a> Message<'a> {
    pub(super) fn new(cx: &'a Render<'a>) -> Self {
        Self {
            out: String::new(),
            cx,
        }
    }

    pub(super) fn finish(self) -> String {
        self.out
    }

    /// Plain text.
    pub fn text(&mut self, text: &str) -> &mut Self {
        self.out.push_str(text);
        self
    }

    /// A count with its noun, plural when not one: `1 file`, `3 files`.
    pub fn count(&mut self, n: usize, noun: &str) -> &mut Self {
        use fmt::Write as _;
        let _ = write!(self.out, "{n} {noun}{}", if n == 1 { "" } else { "s" });
        self
    }

    /// Text a mod spells, cleaned.
    pub fn spelled(&mut self, text: &str) -> &mut Self {
        self.out.push_str(&clean(text));
        self
    }

    /// A chunk path, by its hash.
    pub fn path(&mut self, hash: WadHash) -> &mut Self {
        let name = self.cx.names.path_or_hex(hash);
        self.spelled(&name)
    }

    /// An entry name, by its hash.
    pub fn entry(&mut self, hash: BinHash) -> &mut Self {
        let name = self.cx.names.entry_or_hex(hash);
        self.spelled(&name)
    }

    /// A field name, by its hash.
    pub fn field(&mut self, hash: BinHash) -> &mut Self {
        let name = self.cx.names.field_or_hex(hash);
        self.spelled(&name)
    }

    /// A class name, by its hash.
    pub fn class(&mut self, hash: BinHash) -> &mut Self {
        let name = self.cx.names.class_or_hex(hash);
        self.spelled(&name)
    }

    /// A WAD of the mod, by its name.
    pub fn wad(&mut self, wad: WadId) -> &mut Self {
        let name = self.cx.subject_name(&Subject::Wad(wad));
        self.spelled(&name)
    }

    /// A character, by its name.
    pub fn character(&mut self, character: CharId) -> &mut Self {
        let name = self.cx.subject_name(&Subject::Character(character));
        self.spelled(&name)
    }

    /// A file of the mod, by its path where one is known.
    pub fn file(&mut self, file: FileId) -> &mut Self {
        let name = self.cx.subject_name(&Subject::File(file));
        self.spelled(&name)
    }

    /// An object of the mod, by its entry name where one is known.
    pub fn object(&mut self, object: ObjectId) -> &mut Self {
        let name = self.cx.subject_name(&Subject::Object(object));
        self.spelled(&name)
    }

    /// A unit, as `varus skin0`.
    pub fn unit(&mut self, unit: UnitId) -> &mut Self {
        let name = self.cx.subject_name(&Subject::Unit(unit));
        self.spelled(&name)
    }

    /// The first items of a list, each written by `each`, then how many more there are.
    pub fn list<T>(&mut self, items: &[T], mut each: impl FnMut(&mut Self, &T)) -> &mut Self {
        use fmt::Write as _;
        for (i, item) in items.iter().take(SHOWN).enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            each(self, item);
        }
        if items.len() > SHOWN {
            let _ = write!(self.out, " and {} more", items.len() - SHOWN);
        }
        self
    }
}

impl fmt::Write for Message<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.out.push_str(s);
        Ok(())
    }
}
