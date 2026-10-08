//! The shaders a package's materials link, against the target's shader bin. A material whose
//! pass links one the bin lacks crashes the client at load (#GAME-BEHAVIOR-0023), so such a
//! package is not written (#ADR-0069).

use std::collections::BTreeSet;

use ltk_meta::{BinObject, PropertyValueEnum as V};

use super::findings::{ShaderLink, ShaderMissing};
use crate::game::{GameError, GameSource, SHADERS_BIN};
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
