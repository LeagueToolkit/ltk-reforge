//! The target build's type of each field of each class: the meta schema read at the install's
//! build, through the class's bases, and for a field it does not describe the shape the
//! install's own bins hold. #GAME-BEHAVIOR-0021: the game ships such a field and does not
//! read it, so a mod's copy keeps it in the game's shape.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use ltk_meta::{BinObject, PropertyKind as Kind};

use crate::env::Env;
use crate::game::{Build, FieldType, GameError, Schema};
use crate::model::entity::CharId;
use crate::model::{BinHash, Mod, Slot, Version, WadHash, walk};

/// Where the type of a field comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TypeSource {
    /// The meta schema database.
    Schema,
    /// A bin of the install holds the field in this shape.
    Install,
}

/// The types the target build reads.
#[derive(Debug, Clone, Default)]
pub struct TargetTypes<'a> {
    schema: Option<&'a Schema>,
    /// The build the schema is read at.
    build: Option<u64>,
    /// The install's version.
    version: Option<Version>,
    /// The shape the first bin read holds each field of each class in.
    witnessed: HashMap<(BinHash, BinHash), FieldType>,
    /// The classes of the structs the bins read hold.
    classes: HashSet<BinHash>,
    /// The skins of each character whose bins link more than the
    /// [`MAX_CHAIN`](crate::game::MAX_CHAIN) witnessed.
    capped: BTreeMap<CharId, Vec<Slot>>,
}

impl<'a> TargetTypes<'a> {
    /// The types of a schema read at an install's build, with nothing witnessed. Without a
    /// schema nothing is declared until a bin is witnessed.
    pub fn new(schema: Option<&'a Schema>, install: Option<&Build>) -> Self {
        Self {
            schema,
            build: schema.map(|schema| schema.at_install(install)),
            version: install.and_then(Build::version),
            witnessed: HashMap::new(),
            classes: HashSet::new(),
            capped: BTreeMap::new(),
        }
    }

    /// The version of the install the mod is fixed for, `16.19`. None when the install does
    /// not say its build.
    pub fn version(&self) -> Option<Version> {
        self.version
    }

    /// The types a mod is fixed to: the environment's schema at the game's build, and the
    /// shapes of the game's bins of every skin of the mod's characters, as the game's catalog
    /// of each character reads them: the skin bin and what that links, breadth first, up to
    /// [`MAX_CHAIN`](crate::game::MAX_CHAIN). A skin past the cap is in
    /// [`TargetTypes::capped`].
    pub fn of(m: &Mod, env: &'a Env) -> Result<Self, GameError> {
        let game = env.game();
        let mut types = Self::new(env.schema(), game.build());
        let mut read: BTreeSet<WadHash> = BTreeSet::new();
        for (id, character) in m.characters.iter() {
            let catalog = game.catalog(&character.name)?;
            for path in catalog.chains().values().flatten() {
                // Skins share bins: a bin read for one is not witnessed again.
                if !read.insert(*path) {
                    continue;
                }
                let Some(bin) = game.bin(*path)? else {
                    continue;
                };
                for object in &bin.parsed.objects {
                    types.witness(&object.value);
                }
            }
            if !catalog.capped().is_empty() {
                let slots = catalog.capped().iter().copied().collect();
                types.capped.insert(id, slots);
            }
        }
        Ok(types)
    }

    /// The skins of each character whose bins link more than the
    /// [`MAX_CHAIN`](crate::game::MAX_CHAIN) witnessed: the shapes of the rest are not known.
    pub fn capped(&self) -> &BTreeMap<CharId, Vec<Slot>> {
        &self.capped
    }

    /// Takes the shapes an object of the game holds its fields in, and the classes of its
    /// structs. The first shape seen of a field of a class stays.
    pub fn witness(&mut self, object: &BinObject) {
        walk::structs(object, |s| {
            self.classes.insert(s.class());
            for (field, value) in s.fields() {
                self.witnessed
                    .entry((s.class(), field))
                    .or_insert_with(|| FieldType::of(value));
            }
        });
    }

    /// The schema, when the run has one.
    pub fn schema(&self) -> Option<&'a Schema> {
        self.schema
    }

    /// The build the schema is read at. None without a schema.
    pub fn build(&self) -> Option<u64> {
        self.build
    }

    /// The type the target build reads for a field of a class, and where that is known from.
    pub fn type_of(&self, class: BinHash, field: BinHash) -> Option<(FieldType, TypeSource)> {
        let declared = self
            .schema
            .zip(self.build)
            .and_then(|(schema, build)| schema.field_type(class, field, build));
        match declared {
            Some(field_type) => Some((field_type, TypeSource::Schema)),
            None => self
                .witnessed
                .get(&(class, field))
                .map(|field_type| (*field_type, TypeSource::Install)),
        }
    }

    /// Whether the target build has a field on a class: the schema declares it there, or a
    /// bin of the install holds it there (#GAME-BEHAVIOR-0021).
    pub fn declares(&self, class: BinHash, field: BinHash) -> bool {
        self.type_of(class, field).is_some()
    }

    /// The kind the target build reads a field of a class as.
    pub fn kind_of(&self, class: BinHash, field: BinHash) -> Option<Kind> {
        self.type_of(class, field)
            .map(|(field_type, _)| field_type.kind)
    }

    /// Whether a bin of the install holds the field on the class.
    pub fn witnessed(&self, class: BinHash, field: BinHash) -> bool {
        self.witnessed.contains_key(&(class, field))
    }

    /// How many fields the install's bins witness.
    pub fn witnessed_len(&self) -> usize {
        self.witnessed.len()
    }

    /// Whether the schema has the class at the target build. None without a schema: nothing
    /// is known of the class then.
    pub fn has_class(&self, class: BinHash) -> Option<bool> {
        let (schema, build) = self.schema.zip(self.build)?;
        Some(schema.has_class_at(class, build))
    }

    /// Whether the target build declares the class: the schema has it at the build, or a bin
    /// of the install holds a struct of it. None without a schema. The schema lacks some
    /// classes the game's data holds.
    pub fn declares_class(&self, class: BinHash) -> Option<bool> {
        let held = self.classes.contains(&class);
        self.has_class(class).map(|has| has || held)
    }

    /// Every type a field of a class had from a build to the target's. None without a schema,
    /// or where the range starts before what the schema describes: nothing is known then.
    pub fn types_since(
        &self,
        class: BinHash,
        field: BinHash,
        since: u64,
    ) -> Option<Vec<FieldType>> {
        let (schema, build) = self.schema.zip(self.build)?;
        schema.field_types_between(class, field, since, build)
    }
}
