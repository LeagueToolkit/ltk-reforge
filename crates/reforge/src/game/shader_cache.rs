//! The shader cache's tables of contents: the variants of a game shader the cache holds programs
//! for, and the key a variant is found by (#GAME-BEHAVIOR-0029).

use std::collections::BTreeSet;
use std::io::Cursor;

use ltk_shader::toc::ShaderToc;
use serde::Serialize;
use xxhash_rust::xxh64::xxh64;

use crate::model::{WadHash, path_hash};

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

    fn is_base(&self, name: &str, value: &str) -> bool {
        self.base
            .iter()
            .any(|(base, paired)| base == name && paired == value)
    }
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
