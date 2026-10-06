# MOD-ARTIFACT-0031: An emitter shape renamed under its old field

- **Source:** TopazModFixer, the fixer of the cslol-go manager (its "FixiShape" step, ported from
  a Python script); LtMAO's "Fix VFX Shape" writes the same classes under `SpawnShape`
- **Affects:** `VfxEmitterDefinitionData.shape`, `EmitterPosition`, `SpawnShape`; the classes
  `VfxShapeLegacy`, `VfxShapePointDoNotUse` (`0xee39916f`) and `VfxShapeSphere`
- **Related:** #GAME-CHANGE-0006, #MOD-ARTIFACT-0019, #MOD-ARTIFACT-0029, #GAME-BEHAVIOR-0002

## What is found

An emitter that still holds the field `shape`, which no emitter class declares from 14.1
(#GAME-CHANGE-0006), with a value of a class of 14.1 that the old `shape` never held:

- `shape: embed = VfxShapeLegacy { ... }`: the old `VfxShape`'s fields with the class renamed.
  Its `birthTranslation` is an empty `ValueVector3` where the tool gave its constant to
  `EmitterPosition`, and the author's own where the emitter holds no `EmitterPosition`.
- `shape: embed = VfxShapePointDoNotUse { emitOffset: vec3 }`: the old `emitOffset`'s constant
  value as a bare vector, Riot's form of a constant offset (#GAME-CHANGE-0006). No name table
  spells the class, and the bins show it as `0xee39916f`.

The point is also found under the new field with its offset under a field the class does not
declare: `SpawnShape: VfxShapePointDoNotUse { constantValue: vec3 }`, the vector's z set to 0
(`[-155.0, 450.0, 0.0]`). LtMAO's converter, whose branches TopazModFixer's follow, writes the key
`SpawnShape` in every branch, as a pointer: `VfxShapeLegacy` with the old fields and an emptied
birth translation (#MOD-ARTIFACT-0019), `VfxShapePointDoNotUse { emitOffset: vec3 }`, which is
Riot's form, or `VfxShapeSphere { radius, radius, flags: 1 }`, where a slip writes `radius` a
second time in place of the offset's y.

Beside it the emitter holds `EmitterPosition: embed = ValueVector3 { constantValue }`, the old
birth translation's constant, as its last field, and twice where it held one already.

Where the old offset was constant under a random rotation about `{0, 1, 0}` and `{0, 0, 1}`, the
tool writes `SpawnShape: pointer = VfxShapeSphere { radius, height, flags: 1 }` with the
offset's x as the radius and its y as `height`. `VfxShapeSphere` declares `radius` and `flags`,
not `height`.

It is told from Riot's form by the field: Riot's emitters hold `SpawnShape` and no `shape` from
14.1, and Riot's points hold `emitOffset`. It is told from #MOD-ARTIFACT-0019 by the field too:
there the misplaced value is a birth translation inside a `SpawnShape`.

## Why it is there

The tool follows #GAME-CHANGE-0006 in part. It moves the birth translation's constant to
`EmitterPosition`, chooses a class for the shape by the old fields' values, and writes the new
key only for the sphere. In its other branches it changes the value's class and keeps the key
`shape`, and an embedded value stays embedded where `SpawnShape` is a pointer.

## What reads it, and how

The client reads no `shape` on an emitter from 14.1 (#GAME-BEHAVIOR-0002): the particles are
born at the emitter's position, with none of the spread the author's shape gave. The sphere's
`height` is not read. A birth translation the tool moved with only its constant loses its
curve. A point with no `emitOffset` has the default offset, `[0, 0, 0]`: taken from the schema,
not checked in game.

## The repair

The value under `shape` is the author's old shape, converted as #GAME-CHANGE-0006 converts it.
It is `SpawnShape: pointer` of the same class and fields: a `VfxShapeLegacy` with its
`emitOffset`, `emitRotationAngles` and `emitRotationAxes`, or the point with its `emitOffset`.
The empty birth translation is dropped, and the `EmitterPosition` the tool added stands. A birth
translation the tool left whole is the emitter's `EmitterPosition`.

A point's `constantValue` is its `emitOffset`. The curve the old offset held, if any, is not in
the file, nor is the z the tool set to 0.

A second `EmitterPosition` is the tool's copy of the first.

## Evidence

- The baseline repair run (2026-10-04), over the 5,137 mods of the datasets: 14 mods hold an
  emitter's `shape` of class `VfxShapeLegacy`, and each holds files under TopazModFixer's
  repath prefix (#MOD-ARTIFACT-0029). One of them holds 206 such values and 30 points. A second
  mod holds 101 `SpawnShape: VfxShapePointDoNotUse { constantValue }` values with z 0, and no
  repath prefix of #MOD-ARTIFACT-0029. Which tool wrote it is not established: LtMAO's and
  Hematite's sources write the point's offset under `emitOffset`.
- In those runs each value under `shape` was removed as a field the target build does not
  declare, with nothing lost: the mods are dated 16.3 and later by the game files the tool
  copied in (#MOD-ARTIFACT-0029), so the mod's build was taken to read no `shape`. Each
  `constantValue` of a point was removed so too.
- Repair and port runs of the 15 mods and the reduced corpus with the repair of this record
  (2026-10-06): of the 262 values under `shape`, 148 `VfxShapeLegacy` and 30 points became
  `SpawnShape` pointers, and 84 held a birth translation alone. Of the 225 birth translations,
  116 were empty and dropped, and 109 held a value and became the emitter's `EmitterPosition`,
  the emitter holding none. The 101 points' `constantValue` became their `emitOffset`. No other
  mod changed.
- No sphere with a `height` is in the datasets.
- The sphere's `flags: 1` is the game's own common form. Counted in the census history's object
  texts at 16.19.8207193, every WAD (2026-10-07): 45,834 of the 64,996 `VfxShapeSphere` hold
  `flags: u8 = 1` and the others no `flags`. 103,513 of 107,640 boxes and 57,198 of 110,821
  cylinders hold it too. LTK Manager's VFX renderer plan reads the bit as interior against
  boundary (its section 2.12); its count of no sphere holding it is not reproduced here.
- The class: `rito-meta` names `0xee39916f` `VfxShapePointDoNotUse`, of 14.1, declaring
  `emitOffset: vec3` (read 2026-10-06).
- The branches and the values are read from the source of TopazModFixerCLI (2026-10-05).
  Hematite's source writes the same classes under `SpawnShape` (the point with its offset's z
  set to 0, a `VfxShapeSphere` with a `height`, an empty `VfxShapeLegacy` otherwise), which no
  mod of the datasets shows. LtMAO's is its hapiBin "Fix VFX Shape Property + BirthTranslation"
  (source of version 28032026.0, read 2026-10-06), whose comments call the sphere's rule a
  theory. Nothing was run.
