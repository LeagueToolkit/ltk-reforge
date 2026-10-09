# GAME-BEHAVIOR-0029: A material whose shader variant the cache lacks crashes the client at its draw

- **Status:** Confirmed in game (2026-10-09)
- **Builds:** 16.20; the cache read at 16.19 too
- **Affects:** `StaticMaterialDef.switches`, `StaticMaterialDef.shaderMacros`,
  `StaticMaterialChildTechniqueDef.shaderMacros`, `CustomShaderDef.staticSwitches`,
  `CustomShaderDef.featureDefines`, `ShaderCache.dx11.wad.client`
- **Related:** #GAME-BEHAVIOR-0022, #GAME-BEHAVIOR-0023

## Behaviour

A game shader's compiled programs are stored per variant. For `Shaders/SkinnedMesh/<Name>`,
`ShaderCache.dx11.wad.client` holds `ASSETS/Shaders/Generated/Shaders/SkinnedMesh/<Name>.ps-dx11`
and `.vs-dx11`. Each is a `TOC3.0` file that starts with `baseDefines`, a list of define names
with one value each, and holds one key per compiled variant.

A pass's defines are the shader's `featureDefines`, each static switch as `NAME=1` or `NAME=0`
(the material's `switches` where it sets one, else the shader's `onByDefault`), the material's
`shaderMacros`, a child technique's `shaderMacros` and the client's own defines. The key is the
XXH64, seed 0, of the defines whose name and value are a pair of `baseDefines`, sorted by name and
written `NAME=VALUE` with nothing between them, in the case they are written in. A define whose
value differs from the pair's, or whose name is not in the list, is left out.

When the file holds no such key, the client logs during the load, for each variant:

```
Unable to find correct hash for shader 'ASSETS/Shaders/Generated/Shaders/SkinnedMesh/<Name>.ps-dx11' in wad.
Hash: <the key, in decimal>
Base defines: <every define of the pass>
Filtered:     <the defines the key is made of>
```

then `Failed to compile shader.` with the vertex and pixel shader named. The loading screen
closes. The client logs `Material Missing Pipeline: <hash>` and crashes.

The hash on that line is not established for this case. It matched no name the material is known
by (see Evidence). Where the shader has no programs at all, it is the material's name
(#GAME-BEHAVIOR-0022).

## In a mod

A material on a shader that the game's shader bin holds still crashes the game when its switches
and macros ask for a variant Riot did not compile. A shader's defaults are not always a compiled
variant: a material that leaves a switch at the shader's default can ask for one no material of
Riot's uses.

Which variants exist is the build's. A material is judged against the install's cache.

A material whose switches are set to a compiled combination draws.

## Evidence

- Seen in game (2026-10-09, the 16.20.8248524 copy, a replay at skin 0). A port of a mod with one
  material on the game's `Shaders/SkinnedMesh/Leona_Texture_Transition`, with
  `OFF_VALUE_ON_TEXTURE` off, the shader's default:
  - as ported: during the load, `Unable to find correct hash` for `Leona_Texture_Transition.ps-dx11`
    with `Hash: 15589586251724288417`,
    `Filtered: FEATURE_BLOOM=1USE_ALBEDO_REMAP=1USE_RIM=1USE_THIN_FILM=0`, and with
    `Hash: 13197053208423465423`, the same with `TRANSITION=1` from the material's child
    technique; each followed by `Failed to compile shader.` naming
    `Leona_Texture_Transition.vs` and `.ps`; after the loading screen closed,
    `Material Missing Pipeline: 261d2c7e23688adf`; the log ends there and the game crashed;
  - the same port with that one switch on: the game ran;
  - the same port with the material and the overrides that name it removed: the game ran.

  The four games played with the last two logged no shader line and ended clean
  (`ALE-8SDFH23F`).
- The two hashes are the XXH64 of the two `Filtered` strings (computed 2026-10-09). The `Base
  defines` also held `NUM_BLEND_WEIGHTS=4` and `OFF_VALUE_ON_TEXTURE=0`, and the global defines
  `COLORPALETTE_DEFAULT=1` and `MRT_SUPPORTED=1`. None is a pair of the file's `baseDefines`.
- The cache's `Leona_Texture_Transition.ps-dx11` (read 2026-10-09). Its 13 `baseDefines` at
  16.20.8248524: `USE_THIN_FILM=0`, `FEATURE_BLOOM=1`, `DISABLE_SHADOWS=1`, `TRANSITION=1`,
  `USE_MAP_RIM_LIGHT=1`, `USE_RIM=1`, `DISABLE_FOW=1`, `FORCE_MANTIS_LIGHTING=1`,
  `LOW_QUALITY_MODE=1`, `USE_ALBEDO_REMAP=1`, `OFF_VALUE_ON_TEXTURE=1`, `USE_DYNAMIC_LIGHTING=1`,
  `GENERATE_SHADOW_MAP=1`. Of the keys of every subset of them, the file holds 256. Every one has
  `FEATURE_BLOOM=1`, `OFF_VALUE_ON_TEXTURE=1`, `USE_ALBEDO_REMAP=1`, `USE_RIM=1` and
  `USE_THIN_FILM=0`, and the other eight defines appear in every combination. At 16.19.8207193 the
  file holds 256 keys of the same five values. `IS_AROUND_GAME=1` stands where 16.20 has
  `USE_MAP_RIM_LIGHT=1`.
- The game's `data/shaders/shaders.bin` at 16.20.8248524. `Leona_Texture_Transition` has the
  static switches `USE_RIM` and `USE_ALBEDO_REMAP` on by default and `USE_THIN_FILM` and
  `OFF_VALUE_ON_TEXTURE` off, and the feature define `FEATURE_BLOOM=1`.
- `261d2c7e23688adf` is not the XXH3-64 of the material's entry name or its `name` field, as
  written or lowercased, with or without a technique name appended. It is not that of any material
  name in the game's `Global`, `DATA` and two champion WADs (457 materials), or of any of 844,938
  CommunityDragon bin entry names (computed 2026-10-09).
- LTK Manager's documentation of its game-log diagnostics (`docs/ux/LEAGUE_DIAGNOSTICS.md`, "A
  shader failed", read 2026-10-09) separates this case from #GAME-BEHAVIOR-0022's: a failed
  variant that names its programs "failed for a reason of its own".
