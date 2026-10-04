//! View types: a bin object of a known class, read by its field names.

use ltk_meta::{BinObject, PropertyValueEnum};
use serde::Serialize;

use super::hash::{BinHash, name_hash};
use super::known::{class, field};
use super::value::{self, FileRef};

/// A bank unit of an audio block: the banks the client loads together under one name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AudioUnit {
    /// The unit's name, `Lux_Base_SFX`.
    pub name: String,
    /// Whether it is voice.
    pub voice: bool,
    /// The names of its events.
    pub events: Vec<String>,
    /// The paths of its banks, as the bin spells them.
    pub paths: Vec<String>,
}

/// A skin entry's audio block, `skinAudioProperties`: the tags spell and buff sound hooks are
/// composed from (#GAME-BEHAVIOR-0011), and the bank units the client loads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AudioBlock {
    /// The tags, in order.
    pub tags: Vec<String>,
    /// The bank units, in order.
    pub units: Vec<AudioUnit>,
}

impl AudioBlock {
    /// Whether the block holds no tag and no unit.
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.units.is_empty()
    }
}

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

    /// The object read as a skin entry, whatever its class: the object under a skin's entry
    /// name is the one the client reads as that skin.
    pub fn read(object: &'a BinObject) -> Self {
        Self { object }
    }

    /// The tags of the skin's audio block.
    pub fn audio_tags(&self) -> Vec<&'a str> {
        let path = [field::SKIN_AUDIO_PROPERTIES, field::TAG_EVENT_LIST];
        let tags = value::get(self.object, &path);
        tags.map_or(Vec::new(), |tags| {
            let items = value::items(tags).iter();
            items.filter_map(value::as_str).collect()
        })
    }

    /// The skin's audio block: its tags and its bank units.
    pub fn audio(&self) -> AudioBlock {
        let strings = |list: Option<&'a PropertyValueEnum>| -> Vec<String> {
            let items = list.map_or(&[][..], value::items).iter();
            items
                .filter_map(value::as_str)
                .map(str::to_string)
                .collect()
        };
        let block = field::SKIN_AUDIO_PROPERTIES;
        let units = value::get(self.object, &[block, field::BANK_UNITS]);
        let units = units.map_or(&[][..], value::items).iter();
        let unit = |unit: &'a PropertyValueEnum| {
            let name = value::field_of(unit, field::BANK_UNIT_NAME).and_then(value::as_str);
            let voice = match value::field_of(unit, field::VOICE_OVER) {
                Some(PropertyValueEnum::Bool(flag)) => flag.value,
                Some(PropertyValueEnum::BitBool(flag)) => flag.value,
                _ => false,
            };
            AudioUnit {
                name: name.unwrap_or_default().to_string(),
                voice,
                events: strings(value::field_of(unit, field::BANK_UNIT_EVENTS)),
                paths: strings(value::field_of(unit, field::BANK_PATH)),
            }
        };
        AudioBlock {
            tags: self.audio_tags().into_iter().map(str::to_string).collect(),
            units: units.map(unit).collect(),
        }
    }

    /// The names of the submeshes the skin hides at first, lowercase.
    pub fn hidden_submeshes(&self) -> Vec<String> {
        let path = [field::SKIN_MESH_PROPERTIES, field::INITIAL_SUBMESH_TO_HIDE];
        let hidden = value::get(self.object, &path).and_then(value::as_str);
        hidden.map_or(Vec::new(), |names| {
            let names = names.split([' ', ',']).filter(|name| !name.is_empty());
            names.map(str::to_ascii_lowercase).collect()
        })
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
