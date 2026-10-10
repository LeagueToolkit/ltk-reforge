//! A material's passes and what each asks of its shader's variants: the defines a pass is
//! drawn with, laid over a game shader's defaults in the client's order (#GAME-BEHAVIOR-0029).
//! The fixup that moves a material to a variant the cache holds and the package check that
//! refuses one it lacks read the same view.

use std::collections::BTreeMap;

use ltk_meta::{BinObject, PropertyValueEnum as V};

use super::value::{self, as_str, field_of, items};
use super::walk::Fields;
use super::{BinHash, name_hash};

/// A game shader as the shader bin defines it: where its programs are, and the defines a pass
/// of it starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameShader<'a> {
    /// The entry name of its programs' files in the cache.
    pub object_path: &'a str,
    /// Its feature defines.
    pub features: Vec<(&'a str, &'a str)>,
    /// Each static switch and whether it is on by default.
    pub switches: Vec<(&'a str, bool)>,
}

impl<'a> GameShader<'a> {
    /// The shader of a `CustomShaderDef`. None without an `objectPath`: then its programs are
    /// not found.
    pub fn of(object: &'a BinObject) -> Option<Self> {
        let object_path = value::get(object, &["objectPath"]).and_then(as_str)?;
        let switches = value::get(object, &["staticSwitches"]).map_or(&[][..], items);
        let switches = switches.iter().filter_map(|switch| {
            let name = field_of(switch, "name").and_then(as_str)?;
            Some((name, field_of(switch, "onByDefault").is_some_and(truth)))
        });
        Some(Self {
            object_path,
            features: string_pairs(value::get(object, &["featureDefines"])),
            switches: switches.collect(),
        })
    }

    /// Whether a name is one of the shader's static switches.
    pub fn has_switch(&self, name: &str) -> bool {
        self.switches.iter().any(|(switch, _)| *switch == name)
    }
}

/// What a `StaticMaterialDef` asks of its shaders' variants.
#[derive(Debug, Clone)]
pub struct Material<'a> {
    /// The switches it sets, by name.
    pub switches: BTreeMap<String, bool>,
    /// Its macros, in the map's order.
    pub macros: Vec<(String, String)>,
    techniques: &'a [V],
    children: Vec<Child<'a>>,
}

/// A child technique: a technique's passes with macros of its own on top.
#[derive(Debug, Clone)]
pub struct Child<'a> {
    /// Its name.
    pub name: &'a str,
    /// The technique it draws the passes of.
    pub parent: &'a str,
    /// Its macros.
    pub macros: Vec<(&'a str, &'a str)>,
}

/// A pass of a technique of a material, by the shader it links.
#[derive(Debug, Clone)]
pub struct Pass<'a> {
    /// The technique it is in.
    pub technique: &'a str,
    /// The shader it links.
    pub shader: BinHash,
    /// Its macros.
    pub macros: Vec<(&'a str, &'a str)>,
}

impl<'a> Material<'a> {
    /// The material of an object's fields.
    pub fn of(fields: &'a Fields) -> Self {
        let list = |field: &str| fields.get(&name_hash(field)).map_or(&[][..], items);
        // `StaticMaterialSwitchDef.on` is true where the switch does not hold it.
        let switches = list("switches").iter().filter_map(|switch| {
            let name = field_of(switch, "name").and_then(as_str)?;
            Some((name.to_owned(), field_of(switch, "on").is_none_or(truth)))
        });
        let children = list("childTechniques").iter().map(|child| Child {
            name: field_of(child, "name").and_then(as_str).unwrap_or_default(),
            parent: field_of(child, "parentName")
                .and_then(as_str)
                .unwrap_or_default(),
            macros: string_pairs(field_of(child, "shaderMacros")),
        });
        let macros = string_pairs(fields.get(&name_hash("shaderMacros")));
        Self {
            switches: switches.collect(),
            macros: macros
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
            techniques: list("techniques"),
            children: children.collect(),
        }
    }

    /// The material of an object.
    pub fn of_object(object: &'a BinObject) -> Self {
        Self::of(&object.properties)
    }

    /// Every pass of every technique that links an object.
    pub fn passes(&self) -> impl Iterator<Item = Pass<'a>> + '_ {
        self.techniques.iter().flat_map(|technique| {
            let name = field_of(technique, "name")
                .and_then(as_str)
                .unwrap_or_default();
            let passes = field_of(technique, "passes").map_or(&[][..], items);
            passes.iter().filter_map(move |pass| {
                Some(Pass {
                    technique: name,
                    shader: field_of(pass, "shader").and_then(value::as_link)?,
                    macros: string_pairs(field_of(pass, "shaderMacros")),
                })
            })
        })
    }

    /// The variants a pass is drawn in: on its own, and under each child technique of its
    /// technique.
    pub fn children_of(&self, pass: &Pass<'a>) -> Vec<Option<&Child<'a>>> {
        let of_technique = self
            .children
            .iter()
            .filter(|child| child.parent == pass.technique);
        std::iter::once(None)
            .chain(of_technique.map(Some))
            .collect()
    }

    /// The material with one switch set.
    #[must_use]
    pub fn with_switch(&self, name: &str, on: bool) -> Self {
        let mut material = self.clone();
        material.switches.insert(name.to_owned(), on);
        material
    }

    /// The material with one macro set, in place of its value where it holds the name.
    #[must_use]
    pub fn with_macro(&self, name: &str, value: &str) -> Self {
        let mut material = self.clone();
        match material.macros.iter_mut().find(|(held, _)| held == name) {
            Some((_, held)) => *held = value.to_owned(),
            None => material.macros.push((name.to_owned(), value.to_owned())),
        }
        material
    }

    /// The defines of a pass, in the order the client lays them over each other: the shader's
    /// feature defines, its static switches as the material sets them or by default, the
    /// material's macros, the pass's, and a child technique's on top. The client's own defines
    /// make no key.
    // #GAME-BEHAVIOR-0029, read off every material of the 16.19 and 16.20 installs.
    pub fn defines<'m>(
        &'m self,
        shader: &GameShader<'m>,
        pass: &Pass<'m>,
        child: Option<&Child<'m>>,
    ) -> BTreeMap<&'m str, &'m str> {
        let mut defines = BTreeMap::new();
        defines.extend(shader.features.iter().copied());
        for &(name, by_default) in &shader.switches {
            let on = self.switches.get(name).copied().unwrap_or(by_default);
            defines.insert(name, if on { "1" } else { "0" });
        }
        let macros = self.macros.iter();
        defines.extend(macros.map(|(name, value)| (name.as_str(), value.as_str())));
        defines.extend(pass.macros.iter().copied());
        if let Some(child) = child {
            defines.extend(child.macros.iter().copied());
        }
        defines
    }
}

/// The pairs of a map of strings to strings, in the map's order. None of another value.
fn string_pairs(value: Option<&V>) -> Vec<(&str, &str)> {
    let Some(V::Map(map)) = value else {
        return Vec::new();
    };
    let entries = map.entries().iter();
    entries
        .filter_map(|(name, value)| Some((as_str(name)?, as_str(value)?)))
        .collect()
}

fn truth(value: &V) -> bool {
    match value {
        V::Bool(flag) => flag.value,
        V::BitBool(flag) => flag.value,
        _ => false,
    }
}
