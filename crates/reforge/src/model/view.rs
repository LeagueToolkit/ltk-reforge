//! View types: a bin object of a known class, read by its field names.

use ltk_meta::BinObject;

use super::hash::{BinHash, name_hash};
use super::known::{class, field};
use super::value::{self, FileRef};

/// A skin entry: the object that defines a skin, `Characters/<C>/Skins/Skin<N>`.
#[derive(Debug, Clone, Copy)]
pub struct SkinEntry<'a> {
    object: &'a BinObject,
}

impl<'a> SkinEntry<'a> {
    /// The object as a skin entry, when it is of the skin class.
    pub fn of(object: &'a BinObject) -> Option<Self> {
        (object.class_hash == Self::class()).then_some(Self { object })
    }

    /// The hash of the skin class.
    pub fn class() -> BinHash {
        name_hash(class::SKIN_CHARACTER_DATA_PROPERTIES)
    }

    /// The object read.
    pub fn object(&self) -> &'a BinObject {
        self.object
    }

    /// The entry hash.
    pub fn entry(&self) -> BinHash {
        self.object.path_hash
    }

    fn mesh_file(&self, name: &str) -> Option<FileRef> {
        value::get(self.object, &[field::SKIN_MESH_PROPERTIES, name]).and_then(value::as_file)
    }

    /// The mesh file.
    pub fn mesh(&self) -> Option<FileRef> {
        self.mesh_file(field::SIMPLE_SKIN)
    }

    /// The skeleton file.
    pub fn skeleton(&self) -> Option<FileRef> {
        self.mesh_file(field::SKELETON)
    }

    /// The mesh's texture file.
    pub fn texture(&self) -> Option<FileRef> {
        self.mesh_file(field::TEXTURE)
    }

    /// The graph the entry names.
    pub fn graph(&self) -> Option<BinHash> {
        let path = [
            field::SKIN_ANIMATION_PROPERTIES,
            field::ANIMATION_GRAPH_DATA,
        ];
        value::get(self.object, &path).and_then(value::as_link)
    }

    /// The resolver the entry names.
    pub fn resolver(&self) -> Option<BinHash> {
        value::get(self.object, &[field::RESOURCE_RESOLVER]).and_then(value::as_link)
    }

    /// The skin's name, `LuxSkin07`.
    pub fn skin_name(&self) -> Option<&'a str> {
        value::get(self.object, &[field::CHAMPION_SKIN_NAME]).and_then(value::as_str)
    }

    /// The number of the skin's classification, as the entry holds it.
    pub fn classification(&self) -> Option<u32> {
        value::get(self.object, &[field::SKIN_CLASSIFICATION]).and_then(value::as_u32)
    }

    /// The slot of the skin this one recolors.
    pub fn parent(&self) -> Option<u32> {
        value::get(self.object, &[field::SKIN_PARENT]).and_then(value::as_u32)
    }

    /// Whether the skin is a chroma: the entry holds a parent, of whatever type.
    pub fn is_chroma(&self) -> bool {
        value::get(self.object, &[field::SKIN_PARENT]).is_some()
    }
}

#[cfg(test)]
mod tests {
    use ltk_meta::property::values;

    use super::*;

    #[test]
    fn a_skin_entry_is_read_by_its_field_names() {
        let object = BinObject::builder(
            name_hash("Characters/Lux/Skins/Skin7"),
            name_hash(class::SKIN_CHARACTER_DATA_PROPERTIES),
        )
        .property(
            name_hash(field::RESOURCE_RESOLVER),
            values::ObjectLink::new(name_hash("Characters/Lux/Skins/Skin7/Resources")),
        )
        .property(
            name_hash(field::CHAMPION_SKIN_NAME),
            values::String::from("LuxSkin07"),
        )
        .property(name_hash(field::SKIN_PARENT), values::U32::new(5))
        .build();
        let entry = SkinEntry::of(&object).expect("of the skin class");
        assert_eq!(
            entry.resolver(),
            Some(name_hash("Characters/Lux/Skins/Skin7/Resources"))
        );
        assert_eq!(entry.skin_name(), Some("LuxSkin07"));
        assert_eq!(entry.parent(), Some(5));
        assert!(entry.is_chroma());
        assert_eq!(entry.mesh(), None);
        assert_eq!(entry.graph(), None);
    }

    #[test]
    fn an_object_of_another_class_is_no_skin_entry() {
        let object =
            BinObject::builder(name_hash("x"), name_hash(class::RESOURCE_RESOLVER)).build();
        assert!(SkinEntry::of(&object).is_none());
    }
}
