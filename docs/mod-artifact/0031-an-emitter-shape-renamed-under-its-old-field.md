# MOD-ARTIFACT-0031: An emitter shape renamed under its old field

- **Source:** TopazModFixer, the fixer of the cslol-go manager (its "FixiShape" step, ported from
  a Python script); another tool for one variant
- **Affects:** `VfxEmitterDefinitionData.shape`, `EmitterPosition`, `SpawnShape`; the classes
  `VfxShapeLegacy`, `VfxShapeSphere` and `0xee39916f`
- **Related:** #GAME-CHANGE-0006, #MOD-ARTIFACT-0019, #MOD-ARTIFACT-0029, #GAME-BEHAVIOR-0002

## What is found

An emitter that still holds the field `shape`, which no emitter class declares from 14.1
(#GAME-CHANGE-0006), with a value of a class the old `shape` never held:

- `shape: embed = VfxShapeLegacy { ... }`: the old `VfxShape`'s fields with the class renamed.
  Its `birthTranslation` is an empty `ValueVector3`.
- `shape: embed = 0xee39916f { emitOffset: vec3 }`: a class no build declares, holding the old
  `emitOffset`'s constant value as a bare vector.

The class `0xee39916f` is also found under the new field, from another writer:
`SpawnShape: 0xee39916f { constantValue: vec3 }`, the vector's z set to 0
(`[-155.0, 450.0, 0.0]`).

Beside it the emitter holds `EmitterPosition: embed = ValueVector3 { constantValue }`, the old
birth translation's constant, as its last field, and twice where it held one already.

Where the old offset was constant under a random rotation about `{0, 1, 0}` and `{0, 0, 1}`, the
tool writes `SpawnShape: pointer = VfxShapeSphere { radius, height, flags: 1 }` with the
offset's x as the radius and its y as `height`. `VfxShapeSphere` declares `radius` and `flags`,
not `height`.

It is told from Riot's form by the field: Riot's emitters hold `SpawnShape` and no `shape` from
14.1. It is told from #MOD-ARTIFACT-0019 by the field too: there the misplaced value is a birth
translation inside a `SpawnShape`.

## Why it is there

The tool follows #GAME-CHANGE-0006 in part. It moves the birth translation's constant to
`EmitterPosition`, chooses a class for the shape by the old fields' values, and writes the new
key only for the sphere. In its other branches it changes the value's class and keeps the key
`shape`, and an embedded value stays embedded where `SpawnShape` is a pointer.

## What reads it, and how

The client reads no `shape` on an emitter from 14.1 (#GAME-BEHAVIOR-0002): the particles are
born at the emitter's position, with none of the spread the author's shape gave. The sphere's
`height` is not read. A birth translation the tool moved with only its constant loses its
curve. A `SpawnShape` of class `0xee39916f`, which no build declares, is taken to give no shape
either: not checked in game.

## The repair

The value under `shape` is the author's old shape, converted as #GAME-CHANGE-0006 converts it:

- `VfxShapeLegacy { ... }` is `SpawnShape: pointer = VfxShapeLegacy` with its `emitOffset`,
  `emitRotationAngles` and `emitRotationAxes`. The empty birth translation is dropped, and the
  `EmitterPosition` the tool added stands.
- `0xee39916f { emitOffset: vec3 }` under `shape`, and `0xee39916f { constantValue: vec3 }`
  under `SpawnShape`, are an offset with that constant value. The curve the old offset held, if
  any, is not in the file, nor is the z the second form set to 0.
- A second `EmitterPosition` is the tool's copy of the first.

## Evidence

- The baseline repair run (2026-10-04), over the 5,137 mods of the datasets: 14 mods hold an
  emitter's `shape` of class `VfxShapeLegacy`, and each holds files under TopazModFixer's
  repath prefix (#MOD-ARTIFACT-0029). One of them holds 206 such values and 30 of class
  `0xee39916f`. A second mod holds 101 `SpawnShape: 0xee39916f { constantValue }` values with
  z 0, and no repath prefix of #MOD-ARTIFACT-0029. Which tool wrote it is not established:
  Hematite's source writes `0xee39916f { emitOffset: vec3 }` under `SpawnShape`.
- In those runs each value was removed as a field the target build does not declare, with
  nothing lost: the mods are dated 16.3 and later by the game files the tool copied in
  (#MOD-ARTIFACT-0029), so the mod's build was taken to read no `shape`.
- No sphere with a `height` is in the datasets.
- The branches and the values are read from the source of TopazModFixerCLI (2026-10-05).
  Hematite's source writes the same classes under `SpawnShape` (`0xee39916f` with the offset as
  a vector whose z is set to 0, a `VfxShapeSphere` with a `height`, an empty `VfxShapeLegacy`
  otherwise), which no mod of the datasets shows. Nothing was run.
