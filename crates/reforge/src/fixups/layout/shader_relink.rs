//! A material pass's link to a shader the target's shader bin lacks, relinked to the shader
//! Riot's own materials took at the build that removed it (#GAME-CHANGE-0024, #ADR-0069). The
//! relink draws, and the rest of the material stays the author's: Riot rewrote its materials
//! along with the link.
//!
//! A link with no relink in the table is left. The package refuses it: the client crashes while
//! it loads such a material (#GAME-BEHAVIOR-0023).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ltk_meta::PropertyValueEnum as V;
use ltk_meta::property::values;

use super::Stated;
use crate::fixups::rule::Name;
use crate::fixups::{BinFixup, Classes, FixCx, TargetTypes};
use crate::game::SHADERS_BIN;
use crate::model::{BinHash, FixupInfo, Phase, Record, StructMut, Version, name_hash, path_hash};

static INFO: FixupInfo = FixupInfo::built_in(
    "material-shader-relink",
    "A material's link to a removed shader, to the shader Riot's materials took",
    &[Record::game_change(24)],
    Phase::Layout {
        build: Version::new(12, 7),
    },
);

const PASS: &str = "StaticMaterialPassDef";
const SHADER: &str = "shader";

const STATED: Stated = Stated {
    classes: &[PASS],
    fields: &[SHADER],
};

/// Each removed shader, and the shader most of Riot's materials that named it link from the
/// build that removed it. A name is under `Shaders/SkinnedMesh/`, a hash is the shader's own.
const RELINKS: [(&str, &str); 34] = [
    (
        "ScrollingUVs_MultiLayer_Alpha",
        "ScrollingUVs_MultiLayer_Alpha_Bloom_Wobble",
    ),
    ("IridesceneAshe", "Outline_Iridescent_Add_Scroll"),
    ("0x3094c02d", "2_Diffuse_Lerp"),
    ("Iridescence", "Outline_Iridescent_Add_Scroll"),
    ("HKG_Prestige_Gold", "LLM_Prestige_Gold"),
    ("HKG_Prestige_Diamond", "Outline_MatCap_RMA_VertexDeform"),
    (
        "CustomFresnel_ColorAndAlpha",
        "AlphaBlend_Additive_Scroll_Packed",
    ),
    ("MaskedLighting", "PBR_VFX"),
    (
        "ScrollingUVs_Simple_TextureBlend",
        "AlphaBlend_Additive_Scroll_Packed",
    ),
    ("ScrollingUVs_Janna", "AlphaBlend_Additive_Scroll_Packed"),
    ("Braum_Skin24_Poro", "Outline_MatCap_RMA_VertexDeform"),
    ("HKG_DragonScale", "Outline_Iridescent_Add_Scroll"),
    ("HKG_Zoe_Hat", "AlphaBlend_Additive_Scroll_Packed"),
    ("HKG_Zoe_Translucent", "AlphaBlend_Additive_Scroll_Packed"),
    ("Thresh_Pulsefire_Mat", "AlphaBlend_Additive_Scroll_Packed"),
    ("ScrollingUVs_MultiLayer_Scale_Alpha", "PBR_VFX"),
    ("Aatrox_Skin09_Ult_Wings", "Outline_Iridescent_Add_Scroll"),
    ("AtlasWalk_Simple", "Flipbook_Frame_Select"),
    ("Coloured_Edges_PsyOps", "Outline_Iridescent_Add_Scroll"),
    ("Gradient_Remaped_Mask", "Outline_Iridescent_Add_Scroll"),
    (
        "Addative/ScrollingUVs_MultiLayered_Addative",
        "AlphaBlend_Additive_Scroll_Packed",
    ),
    ("Ekko_Skin18_Clone", "Outline_Iridescent_Add_Scroll"),
    ("Nunu_Bot_Faces", "Outline_Iridescent_Add_Scroll"),
    ("NunuTest", "Outline_Iridescent_Add_Scroll"),
    ("Nunu_ScreenBackdrop", "Outline_Iridescent_Add_Scroll"),
    (
        "MultiMask_Panner_Bloom",
        "AlphaBlend_Additive_Scroll_Packed",
    ),
    (
        "MultiMask_Panner_Fresnel",
        "AlphaBlend_Additive_Scroll_Packed",
    ),
    (
        "MultiMask_Panner_Bloom_Fresnel",
        "AlphaBlend_Additive_Scroll_Packed",
    ),
    (
        "HKG_MatCap_Translucent_VsSimRef",
        "Outline_MatCap_RMA_VertexDeform",
    ),
    ("HKG_Pantheon_Cloak", "Outline_Iridescent_Add_Scroll"),
    ("HKG_FakeSpec_CloakAnim", "Outline_MatCap_RMA_VertexDeform"),
    ("HKG_FakeSpec_Translucent", "Glass"),
    ("HKG_MatCap_Masked", "PBR_VFX"),
    ("Cloth_Base", "Diffuse_Alpha_mult"),
];

/// A shader of the table, by its hash.
fn shader(name: &str) -> BinHash {
    if name.starts_with("0x") {
        Name::new(name).hash()
    } else {
        name_hash(&format!("Shaders/SkinnedMesh/{name}"))
    }
}

/// [`RELINKS`] by hash, hashed once: a material pass asks for every removed shader.
static RELINKED: LazyLock<BTreeMap<BinHash, BinHash>> = LazyLock::new(|| {
    let rows = RELINKS.iter();
    rows.map(|(from, to)| (shader(from), shader(to))).collect()
});

/// The shader Riot's materials took in place of a removed one.
fn relink_of(removed: BinHash) -> Option<BinHash> {
    RELINKED.get(&removed).copied()
}

/// The fixup `material-shader-relink`.
pub struct ShaderRelink;

impl BinFixup for ShaderRelink {
    fn info(&self) -> &FixupInfo {
        &INFO
    }

    fn applies(&self, types: &TargetTypes<'_>) -> bool {
        types.declares(name_hash(PASS), name_hash(SHADER))
    }

    fn classes(&self) -> Classes {
        Classes::named(&[PASS])
    }

    fn fix(&self, cx: &mut FixCx<'_>, mut s: StructMut<'_>) {
        let field = name_hash(SHADER);
        let Some(V::ObjectLink(link)) = s.fields().get(&field) else {
            return;
        };
        let linked = link.value;
        let Some(shaders) = cx.game_bin(path_hash(SHADERS_BIN)) else {
            return;
        };
        if shaders.object(linked).is_some() {
            return;
        }
        let Some(to) = relink_of(linked).filter(|&to| shaders.object(to).is_some()) else {
            return;
        };
        let after = V::ObjectLink(values::ObjectLink::new(to));
        if let Some(before) = s.fields_mut().insert(field, after.clone()) {
            cx.converted(s.path_of(field), &before, &after);
        }
        STATED.teach(cx);
    }
}
