//! One archive taken through `load` and `fix`, for tests of the fixups.

use ltk_meta::property::values::Struct;
use ltk_meta::{BinObject, PropertyValueEnum};
use reforge::diag::{Diag, Diagnostics, Render, Stage};
use reforge::env::Env;
use reforge::fixups::{Changes, FixInputs, FixedMod, FixupSet, ObjectSet};
use reforge::game::{NameBook, Speller};
use reforge::load::{Dating, LoadOptions};
use reforge::model::entity::ObjectId;
use reforge::model::name_hash;

use crate::bins::bin;
use crate::mods::{LoadRun, ModArchive};
use crate::skins::{WAD, skin_bin};

/// An archive whose one skin bin, the base skin's of the champion `test`, holds these objects.
pub fn skin_archive(objects: Vec<BinObject>) -> ModArchive {
    ModArchive::new().file(WAD, &skin_bin("test", 0), bin(objects))
}

/// A mod loaded and fixed, with what the fixups changed and raised.
pub struct FixRun<'a> {
    env: &'a Env,
    /// The mod after the fixups.
    pub fixed: FixedMod,
    /// What the fixups changed.
    pub changes: Changes,
    /// The dating the fixups were given.
    pub dating: Dating,
    /// Every diagnostic of `read`, `load` and `fix`.
    pub log: Diagnostics,
    /// The names the run learnt.
    pub names: NameBook,
}

impl<'a> FixRun<'a> {
    /// Loads an archive with the standard options, dates it against the environment's history
    /// and applies a set of fixups to every loaded object.
    ///
    /// # Panics
    ///
    /// When the archive does not load, or an input does not answer: a test of the fixups
    /// builds ones that do.
    pub fn of(env: &'a Env, set: &FixupSet, archive: &ModArchive) -> Self {
        Self::of_objects(env, set, archive, &|_| ObjectSet::every())
    }

    /// As [`FixRun::of`], on the objects a function of the loaded mod picks.
    ///
    /// # Panics
    ///
    /// As [`FixRun::of`].
    pub fn of_objects(
        env: &'a Env,
        set: &FixupSet,
        archive: &ModArchive,
        objects: &dyn Fn(&reforge::model::Mod) -> ObjectSet,
    ) -> Self {
        let options = LoadOptions::default();
        Self::under(env, set, archive, (options, objects))
    }

    /// As [`FixRun::of_objects`], with `load` under options of the caller's. The set's file
    /// fixups take the place of the options' own.
    ///
    /// # Panics
    ///
    /// As [`FixRun::of`].
    pub fn under(
        env: &'a Env,
        set: &FixupSet,
        archive: &ModArchive,
        (options, objects): (LoadOptions, &dyn Fn(&reforge::model::Mod) -> ObjectSet),
    ) -> Self {
        let options = options.file_fixups(set.file_fixups());
        let LoadRun {
            result, log, names, ..
        } = LoadRun::of(env, options, archive);
        let m = result.unwrap_or_else(|error| panic!("the archive loads: {error}"));
        let dating = Dating::find(&m, env.history())
            .unwrap_or_else(|error| panic!("the history dates the mod: {error}"));
        let objects = objects(&m);
        Self::apply(env, set, (m, objects), (dating, log, names))
    }

    /// Applies a set to the fixed mod once more: the second run of a test of idempotence.
    ///
    /// # Panics
    ///
    /// When an input does not answer.
    pub fn again(self, set: &FixupSet) -> Self {
        let m = self.fixed.into_mod();
        let carried = (self.dating, self.log, self.names);
        Self::apply(self.env, set, (m, ObjectSet::every()), carried)
    }

    fn apply(
        env: &'a Env,
        set: &FixupSet,
        (m, objects): (reforge::model::Mod, ObjectSet),
        (dating, mut log, mut names): (Dating, Diagnostics, NameBook),
    ) -> Self {
        let mut inputs = FixInputs {
            env,
            dating: &dating,
            names: &mut names,
        };
        let applied = set.apply(m, &objects, &mut inputs, &mut log.at(Stage::Fix));
        let (fixed, changes) = applied.unwrap_or_else(|error| panic!("the fixups apply: {error}"));
        Self {
            env,
            fixed,
            changes,
            dating,
            log,
            names,
        }
    }

    /// Spells hashes as the run does.
    pub fn speller(&self) -> Speller<'_> {
        Speller::new(&self.names, self.env.names())
    }

    /// The id of the object under an entry name.
    ///
    /// # Panics
    ///
    /// When the mod holds no such object.
    pub fn id(&self, entry: &str) -> ObjectId {
        let hash = name_hash(entry);
        let mut objects = self.fixed.objects.iter();
        let found = objects.find(|(_, object)| object.entry == hash);
        found
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no object {entry}"))
    }

    /// The object under an entry name, as the fixups left it.
    ///
    /// # Panics
    ///
    /// When the mod holds no such object.
    pub fn object(&self, entry: &str) -> &BinObject {
        &self.fixed.objects[self.id(entry)].value
    }

    /// The value of a field of an object.
    pub fn field(&self, entry: &str, field: &str) -> Option<&PropertyValueEnum> {
        self.object(entry).properties.get(&name_hash(field))
    }

    /// The struct a field of an object points to.
    ///
    /// # Panics
    ///
    /// When the field holds no pointer.
    pub fn child(&self, entry: &str, field: &str) -> &Struct {
        match self.field(entry, field) {
            Some(PropertyValueEnum::Struct(child)) => child,
            other => panic!("{entry}.{field} holds no pointer: {other:?}"),
        }
    }

    /// Every finding of a type the fixup of an id raised.
    pub fn raised<T: Diag>(&self, fixup: &str) -> Vec<&T> {
        let by_fixup = self.log.iter().filter(|d| d.by.as_deref() == Some(fixup));
        by_fixup.filter_map(|d| d.downcast_ref()).collect()
    }

    /// Every diagnostic of `fix` as its code, the fixup that raised it and its line for a
    /// person.
    pub fn messages(&self) -> Vec<String> {
        let speller = self.speller();
        let cx = Render {
            names: &speller,
            module: Some(&self.fixed),
        };
        let of_fix = self.log.iter().filter(|d| d.stage == Stage::Fix);
        of_fix
            .map(|d| {
                let by = d.by.as_deref().unwrap_or("-");
                format!("{} {by}: {}", d.code, d.message(&cx))
            })
            .collect()
    }
}
