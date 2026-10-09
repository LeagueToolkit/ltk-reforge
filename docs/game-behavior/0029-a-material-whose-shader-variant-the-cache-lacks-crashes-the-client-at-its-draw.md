# GAME-BEHAVIOR-0029: A material whose shader variant the cache lacks crashes the client at its draw

- **Status:** Confirmed in game (2026-10-09, again 2026-10-10)
- **Builds:** 16.19, 16.20
- **Affects:** `StaticMaterialDef.switches`, `StaticMaterialDef.shaderMacros`,
  `StaticMaterialPassDef.shaderMacros`, `StaticMaterialChildTechniqueDef.shaderMacros`,
  `CustomShaderDef.staticSwitches`, `CustomShaderDef.featureDefines`,
  `CustomShaderDef.objectPath`, `ShaderCache.dx11.wad.client`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-BEHAVIOR-0022, #GAME-BEHAVIOR-0023

## Behaviour

A game shader's compiled programs are stored per variant. For a `CustomShaderDef` of the game's
shader bin, `ShaderCache.dx11.wad.client` holds `ASSETS/Shaders/Generated/<objectPath>.ps-dx11`
and `.vs-dx11`, the pixel and the vertex programs. A shader's `objectPath` is its entry name,
`Shaders/SkinnedMesh/<Name>` for example. Each file is a `TOC3.0` file that starts with
`baseDefines`, a list of define names each with a value, and holds one key per compiled variant.
A name can come twice in the list, with two values.

A pass's defines are these, each laid over those before it where it names the same define:

1. the shader's `featureDefines`;
2. each static switch of the shader as `NAME=1` or `NAME=0`: the material's `switches` where it
   sets one, else the shader's `onByDefault`. A material's switch that holds no `on` is on;
3. the material's `shaderMacros`;
4. the pass's `shaderMacros`;
5. a child technique's `shaderMacros`. A pass is drawn in the variant of its technique, and in
   the variant of each child technique whose `parentName` names that technique.

The client adds defines of its own, and no file's `baseDefines` pairs them. The key is the XXH64,
seed 0, of the defines whose name and value are a pair of `baseDefines`, sorted by name and
written `NAME=VALUE` with nothing between them, in the case they are written in. A define whose
value differs from every pair of its name, or whose name is not in the list, is left out. With
none kept, the key is the XXH64 of the empty string.

When either file holds no such key, the client logs during the load, for each variant, once
without and once with its global defines `COLORPALETTE_DEFAULT=1` and `MRT_SUPPORTED=1`:

```
Unable to find correct hash for shader 'ASSETS/Shaders/Generated/Shaders/SkinnedMesh/<Name>.ps-dx11' in wad.
Hash: <the key, in decimal>
Base defines: <every define of the pass>
Filtered:     <the defines the key is made of>
```

with `.vs-dx11` where the vertex file lacks it, then `Failed to compile shader.` with the vertex
and pixel shader named. The loading screen closes. The client logs
`Material Missing Pipeline: <hash>` and crashes.

The hash on that line is the material's: two different variants one material lacks give the same
one. What it is made of is not established. For one material it is the XXH3-64 of the material's
entry name, lowercased, as where the shader has no programs at all (#GAME-BEHAVIOR-0022); for
another it matched no name the material is known by (see Evidence).

## In a mod

A material on a shader that the game's shader bin holds still crashes the game when its switches
and macros ask for a variant Riot did not compile. A shader's defaults are not always a compiled
variant: a material that leaves a switch at the shader's default can ask for one no material of
Riot's uses. A switch the material sets off, where the shader has it on by default, asks for a
variant of its own as one set on does.

Which variants exist is the build's. A material is judged against the install's cache.

A material whose switches are set to a compiled combination draws.

The vertex programs of 223 game shaders are compiled only with `NUM_BLEND_WEIGHTS=4`, which
Riot's materials on them set in their `shaderMacros` and the client does not add. A material on
one of them without that macro asks for a vertex variant the cache lacks, and crashes the game.

A material's `switches` of another kind than the build reads, such as pointers in a plain list
where it reads `List2<Embed<StaticMaterialSwitchDef>>`, is dropped whole (#GAME-BEHAVIOR-0002):
every switch takes the shader's default.

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
  The port's material sets `NUM_BLEND_WEIGHTS=4` in its `shaderMacros`, and `TRANSITION=1` in
  those of its one child technique (read 2026-10-10).
- Seen in game (2026-10-10, the 16.19.8207193 copy, a replay of Braum at skin 0, each mod in a
  game of its own). Three mods, each Braum's `skin0.bin` with one material on the whole model:
  Vex's skin 33 shadow material on `Shaders/SkinnedMesh/Diffuse_Alpha_mult`, under the entry
  name `Characters/Braum/Skins/Skin0/Materials/Reforge_Variant_Test`, its one child technique
  adding `TRANSITION=1`:
  - the material as the game holds it, `NUM_BLEND_WEIGHTS=4` among its macros: no shader line;
    the game ran and ended clean (`ALE-8SDFH23F`);
  - the same without `NUM_BLEND_WEIGHTS`: during the load, for `Diffuse_Alpha_mult.vs-dx11`,
    `Hash: 17241709254077376921`, `Base defines: USE_ALBEDO_REMAP=1USE_RIM=1USE_THIN_FILM=0`
    and an empty `Filtered` line, and the same with `TRANSITION=1` for the child technique, each
    followed by `Failed to compile shader.`; after the loading screen closed,
    `Material Missing Pipeline: 93a65ee9c30cfdc3`, and the game crashed. The `Base defines` hold
    no `NUM_BLEND_WEIGHTS`;
  - the same with the static switch `USE_RIM` set off in the material's `switches`, where the
    shader has it on by default and the pixel file pairs `USE_RIM=1` only: during the load, for
    `Diffuse_Alpha_mult.ps-dx11`, `Hash: 17784433496380586952`,
    `Base defines: NUM_BLEND_WEIGHTS=4USE_ALBEDO_REMAP=1USE_RIM=0USE_THIN_FILM=0`,
    `Filtered: USE_ALBEDO_REMAP=1USE_THIN_FILM=0`, and for the child technique
    `Hash: 14135653901136204318` with `TRANSITION=1` in both lines; after the loading screen,
    `Material Missing Pipeline: 93a65ee9c30cfdc3`, and the game crashed.

  Each variant was logged twice, the second time with the global defines. The logged filtered
  strings are those the rule gives for each mod, and each hash is the XXH64 of its `Filtered`
  line, `17241709254077376921` that of the empty string (computed 2026-10-10).
- A first build of the switch mod wrote the switch as a pointer in a plain list. LTK Manager
  warned of the mod, and the game drew it as the material as the game holds it, with no shader
  line (2026-10-10).
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
- The cache's files at 16.20.8248524 and 16.19.8207193 (read 2026-10-10). The install's one
  shader cache is `ShaderCache.dx11.wad.client`. Each of the 350 `CustomShaderDef` of the shader
  bin has its `objectPath` equal to its entry name, and both files at
  `ASSETS/Shaders/Generated/<objectPath>`. At 16.20, 197 of the cache's 871 `TOC3.0` files list a
  name twice in `baseDefines`, most with `0` and `1`. `COLORPALETTE_DEFAULT=1` and
  `MRT_SUPPORTED=1` are a pair of no file of the 350 shaders. `NUM_BLEND_WEIGHTS=4` is a pair of
  the `.vs-dx11` of 223 of them, and every key those files hold is made with it.
- The key read off the game's own materials (computed 2026-10-10): every `StaticMaterialDef` of
  the game's bins, 31,887 at 16.20.8248524 and 31,614 at 16.19.8207193, each pass on a shader of
  the shader bin, under its technique and under each child technique of it. By the rule above,
  both files hold every key: 32,402 and 32,129 variants of a technique, 23,640 and 23,641 of a
  child technique. At either build:
  - without the pass's `shaderMacros`, which 84 passes set, 52 variants of
    `Shaders/StaticMesh/TFT_Skybox` are not held;
  - in 72 variants a static switch sets a define that a feature define sets to another value;
    with the feature define laid last, 26 are not held;
  - in 1,039 variants a child technique's macro sets a static switch to another value; either
    order gives a key the files hold, so Riot's materials do not show which is laid last;
  - keeping every value a name is given, in place of the last, leaves each of those 1,111
    variants without its key in one file or both;
  - 82,709 and 82,628 of the materials' switches hold no `on`, read as on.
- Counted over the 4,909 mods of the datasets in scope by this rule (2026-10-10, against
  16.19.8207193): 9 mods hold a material whose variant the cache lacks, the same 9 in a port and
  in a repair, each a champion skin. Every variant they lack is of a pixel file, and none lacks
  `NUM_BLEND_WEIGHTS=4`:
  - 7 Zed skins, each with one material on `Leona_Texture_Transition` with
    `OFF_VALUE_ON_TEXTURE` off;
  - a Mordekaiser and a Warwick skin, with materials on
    `Shaders/SkinnedMesh/TFT_FixedUVSpace_Bloom` without `SKINNED_MATERIAL_ADDITIVE=1`: the pixel
    file holds the variant with it, and all 288 passes of Riot's on that shader set it in their
    material's `shaderMacros`.
- The pipeline hash (computed 2026-10-10). `93a65ee9c30cfdc3`, the same in both crashes of
  2026-10-10, is the XXH3-64 of `characters/braum/skins/skin0/materials/reforge_variant_test`,
  the test material's entry name lowercased. `261d2c7e23688adf` is not the XXH3-64 of the port's
  material's entry name or its `name` field, as written or lowercased, with or without a
  technique name appended. It is not that of any material name in the game's `Global`, `DATA` and
  two champion WADs (457 materials), or of any of 844,938 CommunityDragon bin entry names
  (computed 2026-10-09).
- LTK Manager's documentation of its game-log diagnostics (`docs/ux/LEAGUE_DIAGNOSTICS.md`, "A
  shader failed", read 2026-10-09) separates this case from #GAME-BEHAVIOR-0022's: a failed
  variant that names its programs "failed for a reason of its own".
