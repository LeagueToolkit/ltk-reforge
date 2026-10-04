//! What a fixup is given beside the struct it fixes: the target build's types, what the mod's
//! own build read, the names, and the record of what it changed.

use ltk_meta::{PropertyKind as Kind, PropertyValueEnum};

use super::changes::{After, Change, Changes, Snapshot};
use super::set::FixError;
use super::types::{TargetTypes, TypeSource};
use crate::game::{FieldType, NameBook, Names, Speller};
use crate::model::entity::{Character, ObjectId};
use crate::model::value::kind_name;
use crate::model::{Arena, BinHash, FieldPath, FixupId, WadHash};

/// The context of one fixup over one run: what it reads, and where it records its changes.
pub struct FixCx<'a> {
    pub(super) types: &'a TargetTypes<'a>,
    /// The number of the build the mod was made at or after.
    pub(super) source: Option<u64>,
    pub(super) characters: &'a Arena<Character>,
    pub(super) tables: &'a Names,
    pub(super) book: &'a mut NameBook,
    pub(super) fixup: &'a FixupId,
    pub(super) object: ObjectId,
    pub(super) changes: &'a mut Changes,
    pub(super) fault: &'a mut Option<FixError>,
}

impl FixCx<'_> {
    /// The types the target build reads.
    pub fn types(&self) -> &TargetTypes<'_> {
        self.types
    }

    /// Whether the target build reads a field on a class.
    pub fn declares(&self, class: BinHash, field: BinHash) -> bool {
        self.types.declares(class, field)
    }

    /// The type the target build reads for a field of a class.
    pub fn type_of(&self, class: BinHash, field: BinHash) -> Option<FieldType> {
        self.types
            .type_of(class, field)
            .map(|(field_type, _)| field_type)
    }

    /// The type the target build reads for a field of a class, with where it is known from.
    pub fn type_and_source(
        &self,
        class: BinHash,
        field: BinHash,
    ) -> Option<(FieldType, TypeSource)> {
        self.types.type_of(class, field)
    }

    /// The number of the build the mod was made at or after. None for a mod the history does
    /// not date.
    pub fn source_build(&self) -> Option<u64> {
        self.source
    }

    /// Every type a field of a class had from the mod's own build to the target's. None when
    /// that is not known: no schema, a mod with no date, or a date before what the schema
    /// describes. An empty list is a field no build of the range declared.
    pub fn types_since_source(&self, class: BinHash, field: BinHash) -> Option<Vec<FieldType>> {
        self.types.types_since(class, field, self.source?)
    }

    /// Whether some build from the mod's own to the target's read a value of this kind in a
    /// field of a class. True where that is not known: only a value known to have never
    /// shown is left where it is.
    pub fn read_at_source(&self, class: BinHash, field: BinHash, kind: Kind) -> bool {
        self.types_since_source(class, field)
            .is_none_or(|types| types.iter().any(|field_type| field_type.kind == kind))
    }

    /// The object being fixed.
    pub fn object(&self) -> ObjectId {
        self.object
    }

    /// The mod's characters.
    pub fn characters(&self) -> &Arena<Character> {
        self.characters
    }

    /// The name tables, without what the run learnt.
    pub fn tables(&self) -> &Names {
        self.tables
    }

    /// Spells hashes from the run's names and the tables.
    pub fn speller(&self) -> Speller<'_> {
        Speller::new(self.book, self.tables)
    }

    /// The run's own names, which learn what a fixup writes.
    pub fn book(&mut self) -> &mut NameBook {
        self.book
    }

    /// Has the run's names learn a field name a fixup wrote, so that a path spells it.
    pub fn learn_field(&mut self, name: &str) -> BinHash {
        self.book.learn_field(name)
    }

    /// Has the run's names learn a class name a fixup wrote.
    pub fn learn_class(&mut self, name: &str) -> BinHash {
        self.book.learn_class(name)
    }

    /// Has the run's names learn a path a fixup hashed away, so that the package's hash
    /// tables keep it readable.
    pub fn learn_path(&mut self, path: &str) -> WadHash {
        self.book.learn_path(path)
    }

    /// Reports an input of the run that did not answer: a name table or a chunk of the game
    /// that does not read. The first fault reported ends the run once the fixup returns, and
    /// there is no fixed mod: a value is not judged against an input that did not answer.
    pub fn fault(&mut self, error: impl Into<FixError>) {
        self.fault.get_or_insert_with(|| error.into());
    }

    /// Whether a fault was reported.
    pub fn faulted(&self) -> bool {
        self.fault.is_some()
    }

    /// A value as a change records it.
    pub fn snapshot(&self, value: &PropertyValueEnum) -> Snapshot {
        Snapshot::of(value, &self.speller())
    }

    /// Records what the fixup did at one place of the object being fixed.
    pub fn change(&mut self, path: FieldPath, before: Option<&PropertyValueEnum>, after: After) {
        let before = before.map(|value| self.snapshot(value));
        self.changes.push(Change {
            fixup: self.fixup.clone(),
            object: self.object,
            path,
            before,
            after,
        });
    }

    /// Records a value moved, unchanged, from one place to another.
    pub fn moved(&mut self, from: FieldPath, to: FieldPath, value: &PropertyValueEnum) {
        self.change(from, Some(value), After::Moved { to });
    }

    /// Records a value put in the form the target build reads, at its place.
    pub fn converted(
        &mut self,
        path: FieldPath,
        before: &PropertyValueEnum,
        after: &PropertyValueEnum,
    ) {
        let after = After::Value(self.snapshot(after));
        self.change(path, Some(before), after);
    }

    /// Records a value the fixup added where there was none.
    pub fn added(&mut self, path: FieldPath, value: &PropertyValueEnum) {
        let after = After::Value(self.snapshot(value));
        self.change(path, None, after);
    }

    /// Records a value removed with nothing lost: no build since the mod's own read it, or a
    /// tool wrote it and no build read it.
    pub fn removed(&mut self, path: FieldPath, value: &PropertyValueEnum) {
        self.change(path, Some(value), After::Removed);
    }

    /// Records a value removed that the mod's own build read and the target cannot hold.
    pub fn lost(&mut self, path: FieldPath, value: &PropertyValueEnum) {
        self.change(path, Some(value), After::Lost);
    }

    /// Records a value left as it is, with no conversion to the kind the target build reads.
    pub fn unconvertible(&mut self, path: FieldPath, value: &PropertyValueEnum, wants: Kind) {
        let wants = kind_name(wants);
        self.change(path, Some(value), After::Unconvertible { wants });
    }
}
