# MOD-ARTIFACT-0021: Integer fields written too wide

- **Source:** the kinds the game itself held before it narrowed them (#GAME-CHANGE-0022);
  which tool or practice carries them into a later mod is not established
- **Affects:** integer fields of `VfxEmitterDefinitionData`, `VfxSystemDefinitionData`,
  `VfxTrailDefinitionData`, `VfxAlphaErosionDefinitionData` and `TrackData`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-BEHAVIOR-0027, #GAME-CHANGE-0020, #GAME-CHANGE-0022,
  #MOD-ARTIFACT-0022

## What is found

An integer property in a wider kind than the build reads: a `u16` or a `u32` where the build
reads a `u8`, an `i32` or a `u32` where it reads a `u16`. The number itself fits the narrower
kind.

The fields found so in mods:

| field | in the mod | the build reads |
|---|---|---|
| `VfxEmitterDefinitionData.meshRenderFlags`, `miscRenderFlags`, `colorRenderFlags` | `u16` | `u8` |
| `VfxEmitterDefinitionData.blendMode`, `importance`, `colorLookUpTypeY`, `texAddressModeBase` | `u32` | `u8` |
| `VfxEmitterDefinitionData.numFrames`, `startFrame` | `i32` | `u16` |
| `VfxSystemDefinitionData.flags` | `u32` | `u16` |
| `VfxTrailDefinitionData.mMode` | `u32` | `u8` |
| `VfxAlphaErosionDefinitionData.erosionDriveSource` | `u32` | `u8` |
| `TrackData.mPriority` | `u32` | `u8` |

`StaticMaterialDef.type` as a `u32` is not this artifact: the game held it so up to 16.17
(#GAME-CHANGE-0020).

## Why it is there

Each wide kind is the kind the game's own bins held the field in before the game narrowed it,
at builds from 9.22 to 13.4 (#GAME-CHANGE-0022).

A mod made before those builds holds the wide kind as the game did. The mods of the baseline
run that hold one are dated at builds from 13.16 to 16.3, which read the narrow kind. How the
wide kind came into them is not established: an emitter pasted from an older mod, or a tool
that writes the kinds of an older schema.

## What reads it, and how

The client drops the property and the field keeps its default (#GAME-BEHAVIOR-0002): an
emitter's blend mode, its render flags and its frame count are the defaults, not the author's.
The client reads no `meshRenderFlags` (#GAME-BEHAVIOR-0027): that one changes nothing drawn.

LTK Manager's repair rule widens an integer and does not narrow one (#MOD-ARTIFACT-0022), so a
mod installed by it keeps the wide kind.

In a mod made at a build that read the narrow kind, the author's own build dropped the value
too.

## The repair

The same number in the kind the build reads, where the number fits it. A number that does not
fit has no form in the narrower kind.

## Evidence

- The kind the game held each field in, build by build, is the evidence of #GAME-CHANGE-0022.
- The meta database (generation 2026-09-28, read 2026-10-04): each field of the table above has
  the narrow kind at every build from 13.15 to 16.19. The database holds 17 properties whose
  integer kind changed in that span. Three are on a class a skin mod holds:
  `StaticMaterialDef.type` (#GAME-CHANGE-0020) and two fields of `SkinCharacterDataProperties`,
  `0x46558f0e` and `0x2d790b62`, each a `u32` up to 16.7 and a `u8` from 16.8. No game object
  holds either of the two at 16.7.7620675, 16.8.7632820 or 16.19.8207193.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired. 18 of the 5,137
  mods, dated from 13.16 to 16.3, hold 456 such values that no build since the mod's own read:
  `meshRenderFlags` 132 in 13 mods, `blendMode` 65 in 13, `importance` 51 in 13,
  `colorLookUpTypeY` 51 in 11, `numFrames` 32 in 12, `miscRenderFlags` 100 in 2, `mMode` 8 in
  2, `flags` 4 in 3, `mPriority` 4 in 2, `texAddressModeBase` 4 in 2, `startFrame` 2 in 2,
  `erosionDriveSource` 2 in 1, `colorRenderFlags` 1 in 1.
- The same run: in 4 mods a `u16` where 16.19 reads a `u8` was converted, 377 values. 125
  mods hold a `u32` where it reads a `u8`, 157 values, a count that holds
  `StaticMaterialDef.type` as well: the reports name the two kinds and not the field.
- No check in game of a wide value is recorded.
