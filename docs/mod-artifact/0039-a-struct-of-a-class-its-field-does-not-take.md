# MOD-ARTIFACT-0039: A struct of a class its field does not take

- **Source:** authors' edits and the tools that wrote them; which tool, if any, is not
  established
- **Affects:** fields that hold a struct, a `pointer` or an `embed`, alone or in a list or a
  map: in the mods found, `AnimationGraphData.mSyncGroupDataMap`,
  `VfxEmitterDefinitionData.rotation0`, `VfxSystemDefinitionData.assetRemappingTable`,
  `VfxEmitterDefinitionData.SpawnShape`, `PersistentEffectConditionData.OwnerCondition` and
  `FloatComparisonMaterialDriver.mValueA`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-BEHAVIOR-0028, #MOD-ARTIFACT-0001

## What is found

A field declares the class of the struct it holds, and takes that class or one that derives
from it. Here the field holds a struct of an unrelated class, or of a class no build's data
holds. No build declared the field with that class: each field below has held its class
unchanged from 13.15 to 16.20.

| field | takes | holds |
|---|---|---|
| `AnimationGraphData.mSyncGroupDataMap` | `SyncGroupData` | an empty `TrackData`, as one entry of the map |
| `VfxEmitterDefinitionData.rotation0` | `IntegratedValueVector3` | a `ValueVector3` with three fields neither class declares |
| `VfxSystemDefinitionData.assetRemappingTable` | `VfxAssetRemap` | a whole `VfxEmitterDefinitionData`, as one item of the list |
| `VfxEmitterDefinitionData.SpawnShape` | `IVfxShape` | `0xee73789f` or `0xee73787f`, each holding only `emitOffset` |
| `PersistentEffectConditionData.OwnerCondition` | `ILogicBoolDriver` | `0xfd47f6e6`, holding a list `mDrivers` |
| `FloatComparisonMaterialDriver.mValueA` | `ILogicFloatDriver` | `0x538189bf`, holding `mSpellSlot` |

- **The sync group.** Each graph holds nine sync groups. Eight are `SyncGroupData` and one,
  under the key `0x477b27ca`, is a `TrackData` with no field. The game's graph of the champion
  holds eight sync groups and no `0x477b27ca` among its sync groups or its tracks.
- **The rotation.** The struct holds `0x00edc2b5`, an integer `1`, and two
  `VfxAnimatedVector3fVariableData` under `0x938122f7` and `0x5bf2f681`. No name table spells
  the three fields, and the meta database declares none of them.
- **The remap.** The item is a complete emitter, with its name, its shape, its primitive and
  its texture, where the list holds pairs of names to remap.
- **The unknown classes.** Four class hashes that no name table spells, that no build's data
  holds, and that no meta database declares. Each struct holds the fields of a known class:
  the shapes only `emitOffset`, as `VfxShapeLegacy` does; the condition a list of drivers, as
  `AllTrueMaterialDriver` does; the float driver a spell slot.

## Why it is there

An author's edit: an entry added to the wrong map, an emitter pasted into the wrong list. How
the rotation's fields came to be is not established; they are a tool's or an older schema's.

The unknown classes came with an author's own additions. The condition and the float driver
are in a rework that adds more than 30 persistent-effect conditions to a skin entry; the two earlier
uploads of the same mod hold no condition and neither class. The shapes are in a mod made again
with a repath tool, whose first form is not among the datasets; the other repathed mods hold
none of the four, so the tool does not write them.

## What reads it, and how

Not established. No such struct was loaded in game. A clip event of a class the client does not
declare crashes the client at load (#GAME-BEHAVIOR-0028). The first three classes are declared,
and held where their field does not take them; the four unknown ones are declared nowhere, as
the event's class was not.

## The repair

None is made: the fixups bring a value to the type its field has at the target build, and these
are no older form of that type. Which known class an unknown one stands for is the author's
intent, which the bins do not say.

## Evidence

- The mods of the datasets in scope (4,909, read 2026-10-09), each struct's class checked
  against the class its holding field takes at 16.19.8207193 in the meta database (generation
  2026-10-05), with the classes that derive from it:
  - `TrackData` in `mSyncGroupDataMap`: 2 mods, 201 structs. Each mod is one champion's skin
    whose graph is copied into about 100 bins (#MOD-ARTIFACT-0001); each copy holds the one
    misplaced entry.
  - `ValueVector3` in `rotation0`: 1 mod, 100 structs, in one emitter of a particle system
    repeated across its bins.
  - `VfxEmitterDefinitionData` in `assetRemappingTable`: 1 mod, 3 structs.
  - `0xee73789f` and `0xee73787f` in `SpawnShape`: 1 mod, 48 structs, the same in its second
    upload. `0xfd47f6e6` in `OwnerCondition` and `0x538189bf` in `mValueA`: 1 mod, 3 structs.
- The meta database declares each of the six fields with the class in the table at every
  build from 13.15 to 16.20.
- The four unknown classes, looked for on 2026-10-09: not in the census history's data at any
  build from 8.20 to 16.20 (the index's table of the classes and fields the data holds), not in
  the meta dumps of 7.16 to 12.12 (10.17 missing) nor the meta database of 13.15 to 16.20, and
  not the hash of any name the tables spell, in any letter case, nor of a name one edit from a
  class name. No schema covers 12.13 to 13.14; the two mods are dated at 16.1 and 16.6.
- The mod with the condition and the float driver: its two earlier uploads in the datasets,
  1.0.7 and 1.0.8, hold neither hash and no persistent-effect condition. The mod with the shapes:
  146 uploads of the datasets are named as repathed, and only its two hold the hashes.
- The game's graph of the champion of the two sync-group mods, at 16.19.8207193: eight sync
  groups and five tracks, none under `0x477b27ca`.
