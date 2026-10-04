# MOD-ARTIFACT-0021: Integer fields written too wide

- **Source:** the kinds the game itself held up to 9.21, 11.8, 12.23 and 13.3; which tool or
  practice carries them into a later mod is not established
- **Affects:** integer fields of `VfxEmitterDefinitionData`, `VfxSystemDefinitionData`,
  `VfxTrailDefinitionData`, `VfxAlphaErosionDefinitionData` and `TrackData`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-CHANGE-0020, #MOD-ARTIFACT-0022

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

Each wide kind is the kind the game's own bins held the field in at earlier builds:

| field | the game's kind | up to | then |
|---|---|---|---|
| `blendMode`, `importance`, `colorLookUpTypeX`, `colorLookUpTypeY`, `texAddressModeBase`, `mMode` | `u32` | 9.21.2940813 | `u8` from 9.22.2961221 |
| `meshRenderFlags`, `miscRenderFlags`, `colorRenderFlags` | `u16` | 9.21.2940813 | `u8` from 9.22.2961221 |
| `VfxSystemDefinitionData.flags` | `u32` | 9.21.2940813 | `u8` from 9.22.2961221, `u16` from 13.1.4875491 |
| `numFrames`, `startFrame` | `i32` | 9.22.2974763 | `u16` from 9.23.2983143 |
| `TrackData.mPriority` | `u32` | 11.8.3704668 | `u8` from 11.9.3718146 |
| `erosionDriveSource` | `u32` | 13.3.4916222 | `u8` from 13.4.4929018 |

A mod made before those builds holds the wide kind as the game did. The mods of the baseline
run that hold one are dated at builds from 13.16 to 16.3, which read the narrow kind. How the
wide kind came into them is not established: an emitter pasted from an older mod, or a tool
that writes the kinds of an older schema.

## What reads it, and how

The client drops the property and the field keeps its default (#GAME-BEHAVIOR-0002): an
emitter's blend mode, its render flags and its frame count are the defaults, not the author's.

LTK Manager's repair rule widens an integer and does not narrow one (#MOD-ARTIFACT-0022), so a
mod installed by it keeps the wide kind.

In a mod made at a build that read the narrow kind, the author's own build dropped the value
too.

## The repair

The same number in the kind the build reads, where the number fits it. A number that does not
fit has no form in the narrower kind.

## Evidence

- Counted in the census history's entry texts by each field's hash, whatever the class
  (2026-10-04), at 18 builds from 8.20.2483196 to 16.19.8207193 and then at each build around
  a change. `blendMode`: 173,817 `u32` and no `u8` at 9.21.2940813, 181,241 `u8` and no `u32`
  at 9.22.2961221. `meshRenderFlags`: 25,605 `u16` at 9.21.2940813, 26,144 `u8` at
  9.22.2961221. `numFrames`: 45,815 `i32` at 9.22.2961221 and none as `u16` at 9.22.2974763,
  46,215 `u16` at 9.23.2983143. `flags`: 48,409 `u8` at 12.23.4875509, 49,691 `u16` at
  13.1.4875491.
- The same count: 16 lines hold `startFrame` as an `i32` at 12.23.4875509 and none at
  13.1.4875491. 3 lines hold the hash of `mPriority` as a `u32` from 11.9.3718146 to
  11.18.3955551. The class of those lines was not read.
- The meta database (generation 2026-09-28, read 2026-10-04): each field of the first table has
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
