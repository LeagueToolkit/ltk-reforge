//! Bins in memory, for tests: the objects of a skin (its entry, graph, resolver, particles)
//! built from names, and a bin of them as bytes.

use ltk_meta::property::values;
use ltk_meta::{Bin, BinObject, PropertyKind, PropertyValueEnum};
use reforge::model::bin;
use reforge::model::known::{class, field};
use reforge::model::name_hash;

/// What a skin entry holds.
#[derive(Debug, Clone)]
pub struct Skin {
    /// The mesh file.
    pub mesh: String,
    /// The skeleton file.
    pub skeleton: String,
    /// The texture file of the mesh.
    pub texture: String,
    /// The entry name of the graph.
    pub graph: String,
    /// The entry name of the resolver.
    pub resolver: String,
    /// The name of the skin, `TestSkin03`.
    pub name: String,
    /// The number of the classification, when the entry holds one.
    pub class: Option<u32>,
    /// The slot of the skin this one recolors, when it is a chroma.
    pub parent: Option<u32>,
}

/// A string value.
pub fn text(text: &str) -> PropertyValueEnum {
    PropertyValueEnum::String(values::String::from(text))
}

/// A link to the object of an entry name.
pub fn link(name: &str) -> PropertyValueEnum {
    PropertyValueEnum::ObjectLink(values::ObjectLink::new(name_hash(name)))
}

/// A boolean value.
pub fn flag(value: bool) -> PropertyValueEnum {
    PropertyValueEnum::Bool(values::Bool::new(value))
}

/// A float value.
pub fn float(value: f32) -> PropertyValueEnum {
    PropertyValueEnum::F32(values::F32::new(value))
}

fn strukt(class: &str, fields: Vec<(&str, PropertyValueEnum)>) -> values::Struct {
    let properties = fields.into_iter().map(|(k, v)| (name_hash(k), v));
    values::Struct {
        class_hash: name_hash(class),
        properties: properties.collect(),
    }
}

/// An embedded struct of a class with these fields.
pub fn embedded(class: &str, fields: Vec<(&str, PropertyValueEnum)>) -> PropertyValueEnum {
    PropertyValueEnum::Embedded(values::Embedded(strukt(class, fields)))
}

/// A pointer to a struct of a class with these fields.
pub fn pointer(class: &str, fields: Vec<(&str, PropertyValueEnum)>) -> PropertyValueEnum {
    PropertyValueEnum::Struct(strukt(class, fields))
}

/// An object of a class with these fields, under an entry name.
pub fn object(entry_name: &str, class: &str, fields: Vec<(&str, PropertyValueEnum)>) -> BinObject {
    let mut object = BinObject::builder(name_hash(entry_name), name_hash(class));
    for (field, value) in fields {
        object = object.property(name_hash(field), value);
    }
    object.build()
}

/// The value of a field of a struct value, by the field's name.
pub fn field_of<'a>(value: &'a PropertyValueEnum, field: &str) -> Option<&'a PropertyValueEnum> {
    reforge::model::value::field_of(value, field)
}

/// A skin entry under an entry name.
pub fn entry(entry_name: &str, skin: &Skin) -> BinObject {
    let mesh = vec![
        (field::SIMPLE_SKIN, text(&skin.mesh)),
        (field::SKELETON, text(&skin.skeleton)),
        (field::TEXTURE, text(&skin.texture)),
    ];
    let animation = vec![(field::ANIMATION_GRAPH_DATA, link(&skin.graph))];
    let mut object = BinObject::builder(
        name_hash(entry_name),
        name_hash(class::SKIN_CHARACTER_DATA_PROPERTIES),
    )
    .property(
        name_hash(field::SKIN_MESH_PROPERTIES),
        embedded("SkinMeshDataProperties", mesh),
    )
    .property(
        name_hash(field::SKIN_ANIMATION_PROPERTIES),
        embedded("SkinAnimationProperties", animation),
    )
    .property(name_hash(field::RESOURCE_RESOLVER), link(&skin.resolver))
    .property(name_hash(field::CHAMPION_SKIN_NAME), text(&skin.name));
    if let Some(class) = skin.class {
        object = object.property(
            name_hash(field::SKIN_CLASSIFICATION),
            values::U32::new(class),
        );
    }
    if let Some(parent) = skin.parent {
        object = object.property(name_hash(field::SKIN_PARENT), values::U32::new(parent));
    }
    object.build()
}

/// A graph whose clips are these animation files.
pub fn graph(entry_name: &str, clips: &[&str]) -> BinObject {
    let items = clips.iter().map(|clip| text(clip)).collect();
    let clips = values::Container::new(PropertyKind::String, items)
        .unwrap_or_else(|error| panic!("a list of strings does not build: {error}"));
    BinObject::builder(
        name_hash(entry_name),
        name_hash(class::ANIMATION_GRAPH_DATA),
    )
    .property(name_hash("clips"), PropertyValueEnum::Container(clips))
    .build()
}

/// A particle system naming one texture.
pub fn particle(entry_name: &str, texture: &str) -> BinObject {
    BinObject::builder(name_hash(entry_name), name_hash("VfxSystemDefinitionData"))
        .property(name_hash("particleName"), text(entry_name))
        .property(name_hash("texture"), text(texture))
        .build()
}

/// A resolver with no key.
pub fn resolver(entry_name: &str) -> BinObject {
    BinObject::builder(name_hash(entry_name), name_hash(class::RESOURCE_RESOLVER)).build()
}

/// A resolver whose map holds these keys, each naming the object of an entry name.
pub fn resolver_with(entry_name: &str, keys: &[(&str, &str)]) -> BinObject {
    let entries = keys.iter().map(|(key, target)| {
        let key = PropertyValueEnum::Hash(values::Hash::new(name_hash(key)));
        (key, link(target))
    });
    let map = values::Map::new(
        PropertyKind::Hash,
        PropertyKind::ObjectLink,
        entries.collect(),
    )
    .unwrap_or_else(|error| panic!("a map of links does not build: {error}"));
    BinObject::builder(name_hash(entry_name), name_hash(class::RESOURCE_RESOLVER))
        .property(name_hash(field::RESOURCE_MAP), PropertyValueEnum::Map(map))
        .build()
}

/// The same object with one more property: a link to `target` under `field`.
pub fn with_link(mut object: BinObject, field: &str, target: &str) -> BinObject {
    object.properties.insert(name_hash(field), link(target));
    object
}

/// The same skin entry with submeshes hidden at first: the names as its mesh block spells
/// them, separated by spaces.
///
/// # Panics
///
/// When the entry holds no mesh block.
pub fn hiding(mut entry: BinObject, submeshes: &str) -> BinObject {
    let block = name_hash(field::SKIN_MESH_PROPERTIES);
    let Some(PropertyValueEnum::Embedded(block)) = entry.properties.get_mut(&block) else {
        panic!("the entry {:08x} holds no mesh block", entry.path_hash.0);
    };
    let hidden = name_hash(field::INITIAL_SUBMESH_TO_HIDE);
    block.0.properties.insert(hidden, text(submeshes));
    entry
}

/// The same graph with one visibility event more, which shows these submeshes.
pub fn showing(mut graph: BinObject, submeshes: &[&str]) -> BinObject {
    let items = submeshes.iter().map(|name| text(name)).collect();
    let shown = values::Container::new(PropertyKind::String, items)
        .unwrap_or_else(|error| panic!("a list of strings does not build: {error}"));
    let shown = PropertyValueEnum::Container(shown);
    let event = pointer(
        class::SUBMESH_VISIBILITY_EVENT_DATA,
        vec![(field::SHOW_SUBMESH_LIST, shown)],
    );
    graph.properties.insert(name_hash("showEvent"), event);
    graph
}

/// A bin of these objects that links no bin.
pub fn bin(objects: Vec<BinObject>) -> Vec<u8> {
    linked_bin(objects, &[])
}

/// A bin of these objects that links other bins, as a skin bin lists them.
pub fn linked_bin(objects: Vec<BinObject>, links: &[&str]) -> Vec<u8> {
    let links: Vec<String> = links.iter().map(ToString::to_string).collect();
    bin::write(&Bin::new(objects, links))
}

/// Bytes of no format, distinct for each text, so a byte copy is recognised.
pub fn blob(text: &str) -> Vec<u8> {
    format!("not a real file: {text}").into_bytes()
}
