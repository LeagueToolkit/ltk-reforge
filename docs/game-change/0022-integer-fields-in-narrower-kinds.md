# GAME-CHANGE-0022: Integer fields in narrower kinds

- **Build:** 9.22.2961221 for the first fields; 9.23.2983143, 11.9.3718146 and 13.4.4929018
  for the others
- **Affects:** integer fields of `VfxEmitterDefinitionData`, `VfxSystemDefinitionData`,
  `VfxTrailDefinitionData`, `VfxAlphaErosionDefinitionData` and `TrackData`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-BEHAVIOR-0027, #GAME-CHANGE-0020, #MOD-ARTIFACT-0021,
  #MOD-ARTIFACT-0022

## Before

The game's bins hold these fields in a wide integer kind:

| field | kind | up to |
|---|---|---|
| `blendMode`, `importance`, `colorLookUpTypeX`, `colorLookUpTypeY`, `texAddressModeBase`, `VfxTrailDefinitionData.mMode` | `u32` | 9.21.2940813 |
| `meshRenderFlags`, `miscRenderFlags`, `colorRenderFlags` | `u16` | 9.21.2940813 |
| `VfxSystemDefinitionData.flags` | `u32` | 9.21.2940813 |
| `numFrames`, `startFrame` | `i32` | 9.22.2974763 |
| `TrackData.mPriority` | `u32` | 11.8.3704668 |
| `VfxAlphaErosionDefinitionData.erosionDriveSource` | `u32` | 13.3.4916222 |

## After

| field | kind | from |
|---|---|---|
| `blendMode`, `importance`, `colorLookUpTypeX`, `colorLookUpTypeY`, `texAddressModeBase`, `mMode` | `u8` | 9.22.2961221 |
| `meshRenderFlags`, `miscRenderFlags`, `colorRenderFlags` | `u8` | 9.22.2961221 |
| `VfxSystemDefinitionData.flags` | `u8` | 9.22.2961221; a `u16` from 13.1.4875491 |
| `numFrames`, `startFrame` | `u16` | 9.23.2983143 |
| `TrackData.mPriority` | `u8` | 11.9.3718146 |
| `erosionDriveSource` | `u8` | 13.4.4929018 |

`StaticMaterialDef.type` went from a `u32` to a `u8` at 16.18 and is a record of its own
(#GAME-CHANGE-0020).

## Riot's conversion

The same number in the narrower kind, as far as the counts show: at the build of each change
the game's bins hold the field in the new kind about as often as they held it in the old kind
one build before, and no longer in the old kind. The values were not paired object by object,
so that each number stayed the same is not established.

A few values stay in the old kind past the build of the change. 16 lines hold `startFrame` as
an `i32` at 12.23.4875509 and none at 13.1.4875491. 3 lines hold the hash of `mPriority` as a
`u32` from 11.9.3718146 to 11.18.3955551. The class of those lines was not read.

## In a mod

A mod made before the build of a change holds the field in the wide kind. A later build drops
the property and the field keeps its default (#GAME-BEHAVIOR-0002): an emitter's blend mode,
its render flags and its frame count are the defaults, not the author's. The client reads no
`meshRenderFlags` (#GAME-BEHAVIOR-0027): that one changes nothing drawn.

The wide kinds are also found in mods made long after the changes (#MOD-ARTIFACT-0021).

LTK Manager's repair rule widens an integer and does not narrow one (#MOD-ARTIFACT-0022), so a
mod installed by it keeps the wide kind.

## Evidence

- Counted in the census history's entry texts by each field's hash, whatever the class
  (2026-10-04), at 18 builds from 8.20.2483196 to 16.19.8207193 and then at each build around
  a change. `blendMode`: 173,817 `u32` and no `u8` at 9.21.2940813, 181,241 `u8` and no `u32`
  at 9.22.2961221. `meshRenderFlags`: 25,605 `u16` at 9.21.2940813, 26,144 `u8` at
  9.22.2961221. `numFrames`: 45,815 `i32` at 9.22.2961221 and none as `u16` at 9.22.2974763,
  46,215 `u16` at 9.23.2983143. `flags`: 48,409 `u8` at 12.23.4875509, 49,691 `u16` at
  13.1.4875491. The numbers of the other fields were not kept.
- `flags` as a `u16` above `0xFF`, the class not read (the same kind of count, 2026-10-07): 11
  lines at 13.1.4875491, 35 at 13.23.5447494, 10,428 at 16.19.8207193. A `u8` holds none, so
  the widening at 13.1 came with values that need it. `rito-meta` (generation 2026-10-05) gives
  `VfxSystemDefinitionData.flags` the default 212 (`0xD4`).
- The meta database (generation 2026-09-28, read 2026-10-04): each field has the narrow kind
  at every build from 13.15 to 16.19. The database starts at 13.15, so the builds of these
  changes come from the census alone.
- No pairing of one object across the build of a change was made. A pairing of the game's
  emitters across 9.21.2940813 and 9.22.2961221, value by value, would establish the
  conversion.
