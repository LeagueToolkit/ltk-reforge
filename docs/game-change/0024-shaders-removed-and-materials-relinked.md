# GAME-CHANGE-0024: Shaders removed and materials relinked

- **Build:** many builds from 10.1, one or a few shaders at each; the ones that touch a
  champion's materials are listed under "Riot's conversion"
- **Affects:** `CustomShaderDef` objects of `data/shaders/shaders.bin`, and the link
  `StaticMaterialDef.techniques[].passes[].shader` that names one
- **Related:** #GAME-BEHAVIOR-0006

## Before

A material's pass names its shader by a link to a `CustomShaderDef` of `data/shaders/shaders.bin`,
which the client loads whatever plays (#GAME-BEHAVIOR-0006). A shader is named by its path,
`Shaders/<folder>/<name>`: `Shaders/SkinnedMesh/Iridescence`.

## After

At a build Riot removes shaders from the bin, and their names resolve to nothing from that build.
The bin held 84 shaders at 8.20 and 350 at 16.19, with 130 removals in between. A few names left
and came back: `Flowmap_Diff` left at 12.6 and again at 16.17, `Diffuse_Alpha_vertex` at 14.14
and 15.24, and `Sylas_DarkStar` left and came back within the builds of 14.21.

Three builds removed many at once:

- **13.4.4929018.** 34 shaders under `Shaders/Environment/` became the same names under
  `Shaders/StaticMesh/`, and `Shaders/Environment/SRX_Brush` and three shaders no table names
  left. No champion's material named one of them.
- **12.9.4387711** removed 12 shaders of `Shaders/StaticMesh/` and `Shaders/Environment/`, and
  **15.9.6763588** six more of `Shaders/StaticMesh/`. No champion's material named one.

## Riot's conversion

At the build that removed a shader, Riot's own materials that named it name another. The other
is seldom the closest name: most went to a few general shaders.

| build | removed | Riot's materials link instead (object files) |
|---|---|---|
| 12.7 | `ScrollingUVs_MultiLayer_Alpha` | `ScrollingUVs_MultiLayer_Alpha_Bloom_Wobble` (6) |
| 13.1 | `IridesceneAshe` | `Outline_Iridescent_Add_Scroll` (10), `AlphaBlend_Additive_Scroll_Packed` (2) |
| 13.3 | `Iridescence` | `Outline_Iridescent_Add_Scroll` (25) |
| 13.8 | `HKG_Prestige_Gold` | `LLM_Prestige_Gold` (6) |
| 13.10 | `HKG_Prestige_Diamond` | `Outline_MatCap_RMA_VertexDeform` (5), `Outline_Iridescent_Add_Scroll` (2) |
| 14.5 | `CustomFresnel_ColorAndAlpha` | `AlphaBlend_Additive_Scroll_Packed` (5) |
| 14.5 | `MaskedLighting` | `PBR_VFX` (2) |
| 14.5 | `ScrollingUVs_Simple_TextureBlend` | `AlphaBlend_Additive_Scroll_Packed` (2) |
| 14.6 | `ScrollingUVs_Janna` | `AlphaBlend_Additive_Scroll_Packed` (11) |
| 14.7 | `Braum_Skin24_Poro` | `Outline_MatCap_RMA_VertexDeform` (9) |
| 14.7 | `HKG_DragonScale` | `Outline_Iridescent_Add_Scroll` (7) |
| 14.7 | `HKG_Zoe_Hat`, `HKG_Zoe_Translucent`, `Thresh_Pulsefire_Mat` | `AlphaBlend_Additive_Scroll_Packed` (3, 1, 3) |
| 14.7 | `ScrollingUVs_MultiLayer_Scale_Alpha` | `PBR_VFX` (7) |
| 15.6 | `Aatrox_Skin09_Ult_Wings` | `Outline_Iridescent_Add_Scroll` (1) |
| 15.7 | `AtlasWalk_Simple` | `Flipbook_Frame_Select` (2) |
| 15.7 | `Coloured_Edges_PsyOps`, `Gradient_Remaped_Mask` | `Outline_Iridescent_Add_Scroll` (1, 2) |
| 15.8 | `Addative/ScrollingUVs_MultiLayered_Addative` | `AlphaBlend_Additive_Scroll_Packed` (2) |
| 15.9 | `Ekko_Skin18_Clone`, `Nunu_Bot_Faces` | `Outline_Iridescent_Add_Scroll` (2, 1) |
| 15.10 | `NunuTest`, `Nunu_ScreenBackdrop` | `Outline_Iridescent_Add_Scroll` (1, 1) |
| 15.15 | `MultiMask_Panner_Fresnel` | `AlphaBlend_Additive_Scroll_Packed` (24) |
| 15.20 | `MultiMask_Panner_Bloom_Fresnel` | `AlphaBlend_Additive_Scroll_Packed` (166), `Outline_Iridescent_Add_Scroll` (14) |
| 15.22 | `HKG_MatCap_Translucent_VsSimRef` | `Outline_MatCap_RMA_VertexDeform` (9) |
| 15.22 | `HKG_Pantheon_Cloak` | `Outline_Iridescent_Add_Scroll` (9) |
| 15.23 | `HKG_FakeSpec_CloakAnim` | `Outline_MatCap_RMA_VertexDeform` (7) |
| 15.23 | `HKG_FakeSpec_Translucent` | `Glass` (14), `PBR_VFX` (1) |
| 15.23 | `HKG_MatCap_Masked` | `PBR_VFX` (10) |
| 16.10 | `Cloth_Base` | `Diffuse_Alpha_mult` (9) |

Every name is under `Shaders/SkinnedMesh/`. A count is of object files of the champions WADs.

- Where every material that named a removed shader left the game at the same build, no
  conversion is read: `Basic_FresnelHighlights_Bloom` (10.13), `AkaliTest` (10.20),
  `MultiMask_Panner_Bloom_Variant` (12.15), `1Blend_1Add_Pan` (12.18), `Hologram` (15.14),
  `Addative/ScrollingUVs_MultiLayered_Fresnel_Addative` (15.15), `ScrollingUVs_Alpha_Bloom`
  (15.16), `Fake_SSS` (15.17), `MordekaiserMetal` (15.18), `Varus_Skin09_QW` (15.19),
  `Sylas_Frosty_Mat`, `Sylas_Skin01_PulseTattoo` (15.20), `HKG_Pantheon_Glasses` (15.22),
  `Viktor_Skin04_Deathsworne_Shader` (15.23), `Pulsefire_Ezrael`, `Sylas_R_Clone` (16.2).
- At 12.19 a shader no table names, `0x3094c02d`, left, and its one material links
  `2_Diffuse_Lerp`.
- Whether a relinked material's other values (its parameters and samplers) changed with the link
  is not read here.

## In a mod

A mod made before the build keeps the old link, and from the build it names no shader the
client loads (#GAME-BEHAVIOR-0006). What the client draws for a pass with no shader is not
checked in game. Two other fixers state that the model is not drawn: Hematite's source says
"Missing shaders cause invisible models".

Both relink such a material to the current shader of the same folder whose name shares the most
`_`-separated words with the old one, after replacing `MultiLayered` with `MultiLayer` and
`Addative` with `Additive`. That is not Riot's conversion. By that rule
`ScrollingUVs_MultiLayered_Addative` goes to `ScrollingUVs_MultiLayer`, `ScrollingUVs_Janna` to
`ScrollingUVs_Evelynn`, `MultiMask_Panner_Fresnel` to `Panner_Fresnel_Mask_Bloom` and
`Iridescence` to `Dissolve_Iridescence_Motion`; Riot linked the first three to
`AlphaBlend_Additive_Scroll_Packed` and the last to `Outline_Iridescent_Add_Scroll`.

## Evidence

- The census history, branch `history-v2`, read 2026-10-05: the `CustomShaderDef` objects of
  `data/shaders/shaders.bin` at every build that changed the bin, 8.20.2483196 to
  16.19.8230722, named with the CommunityDragon name tables.
- For each build that removed shaders: every object file under the champions WADs whose
  material links a removed shader at the build before, and the same file's shader links at the
  build, paired by position where the file holds as many links at both. A file absent at the
  build gives no pair.
- The other fixers' rule is read from the source of TopazModFixerCLI and of Hematite
  (2026-10-05), and applied by a script of its steps to the shaders the bin holds at
  16.19.8230722.
- How many mods of the datasets name a removed shader is not counted.
