# GAME-BEHAVIOR-0027: Some declared VFX fields are loaded and not read

- **Status:** Inferred
- **Builds:** 16.17
- **Affects:** `VfxTextureMultDefinitionData.uvScrollAlphaMult`,
  `VfxTextureMultDefinitionData.isRandomStartFrameMult`, `VfxEmitterDefinitionData.meshRenderFlags`
- **Related:** #GAME-BEHAVIOR-0021, #GAME-CHANGE-0005, #GAME-CHANGE-0022, #MOD-ARTIFACT-0021

## Behaviour

The client declares these fields and loads them into an emitter's definition, and no code of the
client reads them after:

| class | field | what the client does in its place |
|---|---|---|
| `VfxTextureMultDefinitionData` | `uvScrollAlphaMult` | multiplies all four channels of the second texture |
| `VfxTextureMultDefinitionData` | `isRandomStartFrameMult` | takes the second texture's frame from the emitter's one frame counter |
| `VfxEmitterDefinitionData` | `meshRenderFlags` | nothing |

`miscRenderFlags`, beside `meshRenderFlags`, is read: `0x1` turns the depth test off, `0x2` makes
the emitter unbatchable, `0x4` adds the fog-of-war define.

The reading is of 16.17. No other build is read.

## In a mod

A value of one of these fields draws nothing. Kept, moved into its struct or dropped, the emitter
draws the same.

A `meshRenderFlags` in a wider kind than the build reads (#MOD-ARTIFACT-0021) is dropped, and
nothing drawn changes.

## Evidence

- LTK Manager's VFX renderer plan (read 2026-10-07), a reading of the 16.17 client. Its section
  2.11: "`uvScrollAlphaMult`: Dead. Written by the loader, read by nothing, and the shipped
  shaders multiply all four channels", and "One frame counter serves both layers, and
  `isRandomStartFrameMult` is dead too." Its section 3.2 names the two as the bits `0x1` and
  `0x10` of `TextureMultControlBitFlags`, the word the loader writes the struct's flags into.
- Its section 2.14: "`meshRenderFlags`: No reader in 16.17, against a positive control on
  `miscRenderFlags`." The control is `miscRenderFlags`, whose readers the same search found.
- Its section 2.11 on `miscRenderFlags`: "Three bits. `0x1` turns the depth test off, `0x2`
  makes the emitter unbatchable, `0x4` emits a fog-of-war define. Nothing above `0x4` is authored
  anywhere."
- Riot's data holds all three (counted in the census history's object texts, 2026-10-07).
  `uvScrollAlphaMult` is held as `false`, its default being `true`: 6,549 lines at
  16.19.8207193. `isRandomStartFrameMult` is held as `true` on 1,315 lines at 13.1.4875491, on
  none at 13.23.5447494 and on 8 from 14.1.5512844 to 16.19.8207193. `meshRenderFlags` is held
  as 0, its default being 1: 195,860 lines at 16.19.8207193.
- No check in game. A mod that sets one of the three fields against the game's emitter, seen
  beside the game's, would confirm it.
