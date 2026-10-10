//! The shader cache's tables of contents: the variants of a game shader the cache holds programs
//! for, and the key a variant is found by (#GAME-BEHAVIOR-0029).

use std::collections::BTreeSet;
use std::io::Cursor;
use std::sync::Arc;

use ltk_shader::toc::ShaderToc;
use serde::Serialize;
use xxhash_rust::xxh64::xxh64;

use super::GameError;
use super::source::{GameBin, GameSource};
use crate::model::material::{GameShader, Material};
use crate::model::{BinHash, WadHash, path_hash};

/// A stage of a shader's programs, each a file of its own in the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShaderStage {
    /// The pixel shader.
    Pixel,
    /// The vertex shader.
    Vertex,
}

impl ShaderStage {
    /// Both stages, the pixel shader first.
    pub const ALL: [ShaderStage; 2] = [ShaderStage::Pixel, ShaderStage::Vertex];

    /// The extension of the stage's file in `ShaderCache.dx11.wad.client`.
    pub fn extension(self) -> &'static str {
        match self {
            ShaderStage::Pixel => "ps-dx11",
            ShaderStage::Vertex => "vs-dx11",
        }
    }
}

/// The path of the file of a game shader's programs of one stage, by the shader's `objectPath`.
// #GAME-BEHAVIOR-0029: every shader of the game's shader bin has both files here, in the one
// shader cache of the 16.19 and 16.20 installs.
pub fn shader_file(object_path: &str, stage: ShaderStage) -> WadHash {
    let extension = stage.extension();
    path_hash(&format!(
        "ASSETS/Shaders/Generated/{object_path}.{extension}"
    ))
}

/// The key of a variant: the XXH64, seed 0, of its filtered defines as they are written.
pub fn variant_key(filtered: &str) -> u64 {
    xxh64(filtered.as_bytes(), 0)
}

/// The variants of one file of a shader's programs that the cache holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderVariants {
    /// The defines a key is made of, each a name and a value. A name may come with two values.
    base: BTreeSet<(String, String)>,
    /// The key of each variant compiled.
    keys: BTreeSet<u64>,
}

impl ShaderVariants {
    /// The variants of a `TOC3.0` file's bytes. `None` for bytes that are no such file.
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let toc = ShaderToc::read(&mut Cursor::new(bytes)).ok()?;
        let base = toc.base_defines.into_iter();
        Some(Self::new(
            base.map(|define| (define.name, define.value)),
            toc.shader_hashes,
        ))
    }

    /// The variants of these base defines and keys.
    pub fn new(
        base: impl IntoIterator<Item = (String, String)>,
        keys: impl IntoIterator<Item = u64>,
    ) -> Self {
        Self {
            base: base.into_iter().collect(),
            keys: keys.into_iter().collect(),
        }
    }

    /// The defines a variant's key is made of: those whose name and value are a pair of the base
    /// defines, sorted by name, written `NAME=VALUE` with nothing between them.
    pub fn filtered<'d>(&self, defines: impl IntoIterator<Item = (&'d str, &'d str)>) -> String {
        let mut kept: Vec<(&str, &str)> = defines
            .into_iter()
            .filter(|(name, value)| self.is_base(name, value))
            .collect();
        kept.sort_unstable();
        kept.iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect()
    }

    /// Whether the file holds the variant of these filtered defines.
    pub fn holds(&self, filtered: &str) -> bool {
        self.keys.contains(&variant_key(filtered))
    }

    /// The base defines, each a name and a value, by name then value.
    pub fn base(&self) -> impl Iterator<Item = (&str, &str)> {
        self.base
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    fn is_base(&self, name: &str, value: &str) -> bool {
        self.base
            .iter()
            .any(|(base, paired)| base == name && paired == value)
    }
}

/// A variant a material's pass asks a game shader for, and whether one file of the cache holds
/// it.
#[derive(Debug, Clone)]
pub struct AskedVariant {
    /// The shader the pass links.
    pub shader: BinHash,
    /// The pass's technique.
    pub technique: String,
    /// The child technique the pass is drawn under, if any.
    pub child: Option<String>,
    /// The file.
    pub stage: ShaderStage,
    /// The defines the key is made of.
    pub filtered: String,
    /// Whether the file holds the variant.
    pub held: bool,
    /// The file's table.
    pub file: Arc<ShaderVariants>,
}

/// Every variant the passes of a material ask of the game's shaders, judged against the cache.
/// A pass whose shader `skipped` names or the shader bin lacks is left to #GAME-BEHAVIOR-0022
/// and #GAME-BEHAVIOR-0023; one whose shader has no file in the cache for either stage is not
/// judged.
pub fn asked_variants(
    material: &Material<'_>,
    shaders: &GameBin,
    game: &dyn GameSource,
    skipped: &dyn Fn(BinHash) -> bool,
) -> Result<Vec<AskedVariant>, GameError> {
    let mut asked = Vec::new();
    for pass in material.passes() {
        if skipped(pass.shader) {
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
        for child in material.children_of(&pass) {
            let defines = material.defines(&shader, &pass, child);
            for (stage, file) in &files {
                let filtered = file.filtered(defines.iter().map(|(n, v)| (*n, *v)));
                asked.push(AskedVariant {
                    shader: pass.shader,
                    technique: pass.technique.to_owned(),
                    child: child.map(|child| child.name.to_owned()),
                    stage: *stage,
                    held: file.holds(&filtered),
                    filtered,
                    file: Arc::clone(file),
                });
            }
        }
    }
    Ok(asked)
}

/// Whether a name is a static switch of a shader one of the material's passes links.
pub fn is_switch_of(material: &Material<'_>, shaders: &GameBin, name: &str) -> bool {
    material.passes().any(|pass| {
        shaders
            .object(pass.shader)
            .and_then(|definition| GameShader::of(&definition.value))
            .is_some_and(|shader| shader.has_switch(name))
    })
}

#[cfg(test)]
mod tests {
    use super::variant_key;

    #[test]
    fn a_variant_key_is_the_xxh64_of_the_filtered_defines() {
        // #GAME-BEHAVIOR-0029, the two `Filtered` lines of the game's log and their `Hash`.
        let base = "FEATURE_BLOOM=1USE_ALBEDO_REMAP=1USE_RIM=1USE_THIN_FILM=0";
        assert_eq!(variant_key(base), 15_589_586_251_724_288_417);
        let transition = "FEATURE_BLOOM=1TRANSITION=1USE_ALBEDO_REMAP=1USE_RIM=1USE_THIN_FILM=0";
        assert_eq!(variant_key(transition), 13_197_053_208_423_465_423);
    }
}
