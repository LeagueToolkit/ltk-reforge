//! The shaders a package's materials link, against the target's shader bin and shader cache. A
//! material whose pass links one the bin lacks crashes the client at load (#GAME-BEHAVIOR-0023,
//! #ADR-0069), and one whose pass asks a shader for a variant the cache lacks crashes it at the
//! material's draw (#GAME-BEHAVIOR-0029). Either way the package is not written.

use std::collections::{BTreeMap, BTreeSet};

use ltk_meta::{BinObject, PropertyValueEnum as V};

use super::findings::{ShaderLink, ShaderMissing, ShaderVariant, ShaderVariantMissing};
use crate::game::{GameError, GameSource, SHADERS_BIN, ShaderStage, shader_file};
use crate::model::value::{self, as_str, field_of, items};
use crate::model::walk::structs;
use crate::model::{BinHash, name_hash, path_hash};

/// The materials of these objects whose pass links a shader the game's shader bin lacks. None
/// where every link is held, or where the game holds no shader bin: then nothing is known of
/// its shaders.
pub(super) fn missing<'o>(
    objects: impl IntoIterator<Item = &'o BinObject>,
    game: &dyn GameSource,
) -> Result<Option<ShaderMissing>, GameError> {
    let Some(shaders) = game.bin(path_hash(SHADERS_BIN))? else {
        return Ok(None);
    };
    let objects_of_bin = shaders.parsed.objects.iter();
    let held: BTreeSet<BinHash> = objects_of_bin.map(|o| o.value.path_hash).collect();
    let (pass, shader) = (name_hash("StaticMaterialPassDef"), name_hash("shader"));
    let mut links = BTreeSet::new();
    for object in objects {
        structs(object, |s| {
            if s.class() != pass {
                return;
            }
            if let Some(V::ObjectLink(link)) = s.get(shader)
                && !held.contains(&link.value)
            {
                links.insert(ShaderLink {
                    material: s.entry(),
                    shader: link.value,
                });
            }
        });
    }
    let links: Vec<ShaderLink> = links.into_iter().collect();
    Ok((!links.is_empty()).then_some(ShaderMissing { links }))
}

/// The variants of game shaders that the materials of these objects ask for and the game's
/// shader cache lacks. A pass whose shader the game's shader bin lacks, or the objects define,
/// is left to [`missing`] and to #GAME-BEHAVIOR-0022; one whose shader has no file in the
/// cache is not judged. None where every variant is held, or where the game holds no shader
/// bin.
pub(super) fn variants_missing<'o>(
    objects: impl IntoIterator<Item = &'o BinObject>,
    game: &dyn GameSource,
) -> Result<Option<ShaderVariantMissing>, GameError> {
    let Some(shaders) = game.bin(path_hash(SHADERS_BIN))? else {
        return Ok(None);
    };
    let objects: Vec<&BinObject> = objects.into_iter().collect();
    let shader_class = name_hash("CustomShaderDef");
    let own: BTreeSet<BinHash> = objects
        .iter()
        .filter(|object| object.class_hash == shader_class)
        .map(|object| object.path_hash)
        .collect();
    let material_class = name_hash("StaticMaterialDef");
    let mut variants = BTreeSet::new();
    for object in objects.iter().filter(|o| o.class_hash == material_class) {
        let material = Material::of(object);
        for pass in material.passes() {
            if own.contains(&pass.shader) {
                continue;
            }
            let Some(definition) = shaders.object(pass.shader) else {
                continue;
            };
            let Some(shader) = GameShader::of(&definition.value) else {
                continue;
            };
            let mut files = Vec::new();
            for stage in ShaderStage::ALL {
                match game.shader_variants(shader_file(shader.object_path, stage))? {
                    Some(held) => files.push((stage, held)),
                    None => break,
                }
            }
            if files.len() < ShaderStage::ALL.len() {
                continue;
            }
            for child in pass.children(&material) {
                let defines = defines(&shader, &material, &pass, child);
                for (stage, held) in &files {
                    let filtered = held.filtered(defines.iter().map(|(n, v)| (*n, *v)));
                    if !held.holds(&filtered) {
                        variants.insert(ShaderVariant {
                            material: object.path_hash,
                            shader: pass.shader,
                            technique: pass.technique.to_owned(),
                            child: child.map(|child| child.name.to_owned()),
                            stage: *stage,
                            filtered,
                        });
                    }
                }
            }
        }
    }
    let variants: Vec<ShaderVariant> = variants.into_iter().collect();
    Ok((!variants.is_empty()).then_some(ShaderVariantMissing { variants }))
}

/// The defines of a pass, in the order the client lays them over each other: the shader's
/// feature defines, its static switches as the material sets them or by default, the material's
/// macros, the pass's, and a child technique's on top. The client's own defines make no key.
// #GAME-BEHAVIOR-0029, read off every material of the 16.19 and 16.20 installs.
fn defines<'a>(
    shader: &GameShader<'a>,
    material: &Material<'a>,
    pass: &Pass<'a>,
    child: Option<&Child<'a>>,
) -> BTreeMap<&'a str, &'a str> {
    let mut defines = BTreeMap::new();
    defines.extend(shader.features.iter().copied());
    for &(name, by_default) in &shader.switches {
        let on = material.switches.get(name).copied().unwrap_or(by_default);
        defines.insert(name, if on { "1" } else { "0" });
    }
    defines.extend(material.macros.iter().copied());
    defines.extend(pass.macros.iter().copied());
    if let Some(child) = child {
        defines.extend(child.macros.iter().copied());
    }
    defines
}

/// A game shader as the shader bin defines it: where its programs are, and the defines a pass
/// of it starts from.
struct GameShader<'a> {
    object_path: &'a str,
    features: Vec<(&'a str, &'a str)>,
    /// Each static switch and whether it is on by default.
    switches: Vec<(&'a str, bool)>,
}

impl<'a> GameShader<'a> {
    /// The shader of a `CustomShaderDef`. None without an `objectPath`: then its programs are
    /// not found.
    fn of(object: &'a BinObject) -> Option<Self> {
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
}

/// What a `StaticMaterialDef` asks of its shaders' variants.
struct Material<'a> {
    /// The switches it sets, by name.
    switches: BTreeMap<&'a str, bool>,
    macros: Vec<(&'a str, &'a str)>,
    techniques: &'a [V],
    children: Vec<Child<'a>>,
}

/// A child technique: a technique's passes with macros of its own on top.
struct Child<'a> {
    name: &'a str,
    parent: &'a str,
    macros: Vec<(&'a str, &'a str)>,
}

/// A pass of a technique of a material, by the shader it links.
struct Pass<'a> {
    technique: &'a str,
    shader: BinHash,
    macros: Vec<(&'a str, &'a str)>,
}

impl<'a> Material<'a> {
    fn of(object: &'a BinObject) -> Self {
        let list = |field: &str| value::get(object, &[field]).map_or(&[][..], items);
        // `StaticMaterialSwitchDef.on` is true where the switch does not hold it.
        let switches = list("switches").iter().filter_map(|switch| {
            let name = field_of(switch, "name").and_then(as_str)?;
            Some((name, field_of(switch, "on").is_none_or(truth)))
        });
        let children = list("childTechniques").iter().map(|child| Child {
            name: field_of(child, "name").and_then(as_str).unwrap_or_default(),
            parent: field_of(child, "parentName")
                .and_then(as_str)
                .unwrap_or_default(),
            macros: string_pairs(field_of(child, "shaderMacros")),
        });
        Self {
            switches: switches.collect(),
            macros: string_pairs(value::get(object, &["shaderMacros"])),
            techniques: list("techniques"),
            children: children.collect(),
        }
    }

    /// Every pass of every technique that links an object.
    fn passes(&self) -> impl Iterator<Item = Pass<'a>> + '_ {
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
}

impl<'a> Pass<'a> {
    /// The variants the pass is drawn in: on its own, and under each child technique of its
    /// technique.
    fn children<'m>(&self, material: &'m Material<'a>) -> Vec<Option<&'m Child<'a>>> {
        let of_technique = material
            .children
            .iter()
            .filter(|child| child.parent == self.technique);
        std::iter::once(None)
            .chain(of_technique.map(Some))
            .collect()
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
