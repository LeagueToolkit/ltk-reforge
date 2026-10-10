//! The shaders a package's materials link, against the target's shader bin and shader cache. A
//! material whose pass links one the bin lacks crashes the client at load (#GAME-BEHAVIOR-0023,
//! #ADR-0069), and one whose pass asks a shader for a variant the cache lacks crashes it at the
//! material's draw (#GAME-BEHAVIOR-0029). Either way the package is not written.

use std::collections::BTreeSet;

use ltk_meta::{BinObject, PropertyValueEnum as V};

use super::findings::{ShaderLink, ShaderMissing, ShaderVariant, ShaderVariantMissing};
use crate::game::{GameError, GameSource, SHADERS_BIN, asked_variants};
use crate::model::material::Material;
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
        let material = Material::of_object(object);
        let asked = asked_variants(&material, &shaders, game, &|shader| own.contains(&shader))?;
        for asked in asked.into_iter().filter(|asked| !asked.held) {
            variants.insert(ShaderVariant {
                material: object.path_hash,
                shader: asked.shader,
                technique: asked.technique,
                child: asked.child,
                stage: asked.stage,
                filtered: asked.filtered,
            });
        }
    }
    let variants: Vec<ShaderVariant> = variants.into_iter().collect();
    Ok((!variants.is_empty()).then_some(ShaderVariantMissing { variants }))
}
