# GAME-CHANGE-0006: Emitter shape as SpawnShape and EmitterPosition

- **Build:** 14.1
- **Affects:** `VfxEmitterDefinitionData`, `VfxShape`, `IVfxShape` and the classes derived from
  it: `VfxShapeLegacy`, `VfxShapePointDoNotUse`, `VfxShapeBox`, `VfxShapeCylinder`,
  `VfxShapeSphere`
- **Related:** #GAME-BEHAVIOR-0002

## Before

Up to 13.24 an emitter holds where its particles are born as one struct, `shape: embed =
VfxShape`, with four fields:

| field of `VfxShape` | type |
|---|---|
| `birthTranslation` | `embed = ValueVector3` |
| `emitOffset` | `embed = ValueVector3` |
| `emitRotationAngles` | a list of `embed = ValueFloat` |
| `emitRotationAxes` | a list of `vec3` |

## After

From 14.1 an emitter holds two fields in its place, and its class does not declare `shape`:

- `EmitterPosition: embed = ValueVector3`, the emitter's own offset.
- `SpawnShape: pointer = IVfxShape`, the volume the particles are born in. It is null by
  default.

`IVfxShape` is an interface. A `SpawnShape` is one of the classes derived from it:
`VfxShapeBox`, `VfxShapeCylinder` and `VfxShapeSphere`, each a volume; `VfxShapeLegacy`, which
holds `emitOffset`, `emitRotationAngles` and `emitRotationAxes` as `VfxShape` held them; or
`VfxShapePointDoNotUse`, which holds one field, `emitOffset: vec3`. No name table spells the
point's class: a bin shows it as `0xee39916f`.

`VfxShapeLegacy` declares no `birthTranslation`.

## Riot's conversion

- `birthTranslation` became the emitter's `EmitterPosition`, its value unchanged.
- The other three fields became the `SpawnShape`. Riot rewrote many shapes as the volume they
  drew: a random offset as a `VfxShapeBox`, `VfxShapeSphere` or `VfxShapeCylinder` of the
  offset's range, a constant offset under a random rotation as a `VfxShapeSphere` of that
  radius. It rewrote a constant offset as a `VfxShapePointDoNotUse` holding the constant as a
  bare vector. It kept the others as a `VfxShapeLegacy` with the three fields unchanged.
- No other field of the emitter moved.

Which shapes Riot rewrote as a volume or a point and which it kept is not established as a
rule. Some points came from shapes with a curve or a rotation, one at another value.
`VfxShapeLegacy` holds the three fields under their old names and types, so every old shape can
be written as one.

## In a mod

A mod made before 14.1 holds `shape` on the emitter. A later build drops it
(#GAME-BEHAVIOR-0002): `EmitterPosition` and `SpawnShape` keep their defaults, so every particle
is born at the emitter's origin with no offset and no spread.

A mod made later holds `shape` too when its author pasted an emitter from an older mod. The
author's own build dropped it.

## Evidence

- ltk-forge paired 3,000 of the game's entries across 13.24 and 14.1 on the census
  (2026-09-30). `EmitterPosition` equals the old `birthTranslation` in 10,840 of 10,840 paired
  emitters. No other field of an emitter moved.
- That the client draws a `VfxShapeLegacy` as it drew a `VfxShape` of the same three values is
  inferred from Riot keeping shapes so. No check in game compared the two.
- The meta database (generation 2026-09-28, read 2026-10-03):
  `VfxEmitterDefinitionData.shape` is an embed of `VfxShape` up to 13.24 and is removed in
  14.1. `SpawnShape`, a pointer to `IVfxShape` with default null, and `EmitterPosition`, an
  embed of `ValueVector3`, are declared from 14.1. `IVfxShape` and `VfxShapeLegacy` exist from
  14.1. `VfxShapeLegacy` declares `emitOffset`, `emitRotationAngles` and `emitRotationAxes`.
  The class `VfxShape` is removed in 14.5.
- Counted in the census history's object texts by the field's hash (2026-10-07): 754,118 lines
  hold `shape` as an embed of `VfxShape` at 13.24.5461574, and no line holds `shape` at
  14.1.5512844, 14.5.5636323, 16.17.8104348 or 16.19.8207193. The class lives to 14.5 in the
  schema; no emitter holds it after 13.24.
- No game object from 10.9 to 16.19 holds a `birthTranslation` in a `VfxShapeLegacy`
  (ltk-forge's census lookup, 2026-09-30).
- The point: `rito-meta` (generation 2026-10-05, read 2026-10-06) names `0xee39916f`
  `VfxShapePointDoNotUse`, derived from `IVfxShape` from 14.1, declaring `emitOffset: vec3`. In
  the census history, branch `history-v2`, 425 object files of Ahri's WAD hold one at
  14.1.5512844 and none at 13.24.5479214. In the first 40 of them, emitters paired by file and
  `emitterName` give 121 points from a 13.24 shape whose `emitOffset` held a constant alone, and
  89 from shapes with a curve, a rotation or other fields. The samples read keep the constant's
  value, but for one: `{30, 0, 0}` with a curve became `{1, 0, 0}`.
