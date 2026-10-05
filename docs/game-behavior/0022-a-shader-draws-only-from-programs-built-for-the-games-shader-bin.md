# GAME-BEHAVIOR-0022: A shader draws only from programs built for the game's shader bin

- **Status:** Inferred
- **Builds:** 16.19
- **Affects:** `CustomShaderDef`, the passes of `StaticMaterialDef`, `ShaderCache.dx11.wad.client`
- **Related:** #GAME-BEHAVIOR-0006, #GAME-CHANGE-0024

## Behaviour

The client compiles no shader of a mod. It draws a material's pass with the compiled programs
that `ShaderCache.dx11.wad.client` holds for a `CustomShaderDef` of `data/shaders/shaders.bin`,
found by the shader's `objectPath` and the pass's defines. Every `CustomShaderDef` of the game is
in that bin: 350 at 16.19.

A `CustomShaderDef` that a mod defines in another bin, a skin bin for example, takes the place of
the game's object of that name and has no programs. The client logs `Failed to compile shader.`
for each variant of the pass, then `Material Missing Pipeline: <hash>`, and crashes when it first
draws the material.

Not established: what the client does with a pass whose shader link names no `CustomShaderDef`
at all, as a mod's link to a shader Riot removed does (#GAME-CHANGE-0024).

## In a mod

A mod's own definition of a game shader crashes the game where a material that uses it is drawn.
Without the definition, the material's link names the game's shader of that name and draws.

## Evidence

- LTK Manager's documentation of its game-log diagnostics (`docs/ux/LEAGUE_DIAGNOSTICS.md`,
  "A shader failed", read 2026-10-06), and its fixture of a game log of build 16.19.8217343: the
  failed variants with their defines, then the missing pipeline, then the crash handler. Its
  release 1.22.1 names the cause it reports, a mod that defines a `CustomShaderDef` outside
  `data/shaders/shaders.bin`.
- The census history holds 350 `CustomShaderDef` objects in `data/shaders/shaders.bin` at
  16.19.8230722 (#GAME-CHANGE-0024).
- How many mods of the datasets define a `CustomShaderDef` is not counted: the baseline reports
  do not list objects by class.
- What would confirm it here: a mod whose skin bin defines a `CustomShaderDef`, seen in game with
  the object and without it.
