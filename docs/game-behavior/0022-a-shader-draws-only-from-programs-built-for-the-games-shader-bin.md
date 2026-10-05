# GAME-BEHAVIOR-0022: A shader draws only from programs built for the game's shader bin

- **Status:** Confirmed in game (2026-10-06)
- **Builds:** 16.19
- **Affects:** `CustomShaderDef`, the passes of `StaticMaterialDef`, `ShaderCache.dx11.wad.client`
- **Related:** #GAME-BEHAVIOR-0006, #GAME-BEHAVIOR-0023, #GAME-CHANGE-0024

## Behaviour

The client compiles no shader of a mod. It draws a material's pass with the compiled programs
that `ShaderCache.dx11.wad.client` holds for a `CustomShaderDef` of `data/shaders/shaders.bin`.
Every `CustomShaderDef` of the game is in that bin: 350 at 16.19.

A `CustomShaderDef` that a mod defines in another bin, a skin bin for example, has no programs:

- under the name of a game shader, it takes the place of the game's object of that name;
- under a name of its own, it is a shader no program was built for.

Either way the client logs `Failed to compile shader.` for each variant of the pass while it
loads, the shader names of the record empty. The loading screen closes. At the first draw of the
material the client logs `Material Missing Pipeline: <hash>` and crashes.

The hash is the XXH3-64 of the material's entry name, lowercased, written in hex with no leading
zeros: the log names the material that crashed.

## In a mod

A mod's own definition of a shader crashes the game where a material that uses it is drawn.
Without the definition, a material's link to a game shader of that name draws.

## Evidence

- Seen in game (2026-10-06, the 16.19.8207193 copy, a replay of Braum at skin 0). Three mods,
  each Braum's `skin0.bin` with one material on the whole model: Vex's skin 33 shadow material as
  the game holds it, drawing Braum's own texture.
  - The material linking the game's `Shaders/SkinnedMesh/Diffuse_Alpha_mult`: Braum drawn, no
    shader line in the log.
  - The same with the game's definition of `Diffuse_Alpha_mult` copied into the skin bin: two
    variants failed to compile during the load, with the pass defines
    `NUM_BLEND_WEIGHTS=4USE_ALBEDO_REMAP=1USE_RIM=1USE_THIN_FILM=0`, and with `TRANSITION=1` among
    them, again for the global defines `COLORPALETTE_DEFAULT=1MRT_SUPPORTED=1`; the loading screen
    closed at 1.67 s; `Material Missing Pipeline: 65037001b3eb385e` at 1.72 s; the game crashed.
  - The same definition copied under the name `Shaders/SkinnedMesh/Reforge_Test_Own_Shader`, the
    material linking it: the same lines and the same hash, and the crash.
- `65037001b3eb385e` is the XXH3-64 of
  `characters/braum/skins/skin0/materials/reforge_shader_test`, the test material's name
  lowercased (computed 2026-10-06). It did not change with the shader's name.
- LTK Manager's documentation of its game-log diagnostics (`docs/ux/LEAGUE_DIAGNOSTICS.md`,
  "A shader failed", read 2026-10-06) states the cause and the lines, and its fixture of a game
  log of 16.19.8217343 holds them, the hash printed as `822941f5adffbcc`.
- The census history holds 350 `CustomShaderDef` objects in `data/shaders/shaders.bin` at
  16.19.8230722 (#GAME-CHANGE-0024).
- How many mods of the datasets define a `CustomShaderDef` is not counted: the baseline reports
  do not list objects by class.
