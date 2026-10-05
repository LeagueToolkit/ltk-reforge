# GAME-BEHAVIOR-0023: A material linking no shader crashes the client at load

- **Status:** Confirmed in game (2026-10-06)
- **Builds:** 16.19
- **Affects:** the link `StaticMaterialDef.techniques[].passes[].shader`
- **Related:** #GAME-BEHAVIOR-0006, #GAME-BEHAVIOR-0022, #GAME-CHANGE-0024

## Behaviour

A material's pass whose shader link names no `CustomShaderDef` the client holds crashes the
client during the load, before the loading screen closes. A shader Riot removed and
a name no shader ever had give the same crash: what the shader cache may still hold under a
removed shader's name does not stand in for it.

The game log says nothing of it. It ends before any line about shaders, cut short by the crash.

It is a bound on #GAME-BEHAVIOR-0006: a link to an object nothing loads is skipped there, and a
pass's shader link is not.

## In a mod

A mod made before a build at which Riot removed a shader its material names crashes the game at
load from that build (#GAME-CHANGE-0024). The game's log does not point at the mod.

## Evidence

- Seen in game (2026-10-06, the 16.19.8207193 copy, a replay of Braum at skin 0). Two mods, each
  Braum's `skin0.bin` with one material on the whole model, the control of #GAME-BEHAVIOR-0022
  with its pass's shader link changed:
  - to `Shaders/SkinnedMesh/Cloth_Base`, which left the shader bin at 16.10: the game crashed at
    once;
  - to `Shaders/SkinnedMesh/Reforge_Test_No_Such_Shader`, a name no build held: the same.
- Both game logs end at 0.46 to 0.48 s, after the replay's session starts and before the load,
  with no line about shaders and no line of the crash handler. The control's log of the same
  replay goes on through the load and the game.
- The user saw both crash at once, faster than the loading screen could be told apart.
