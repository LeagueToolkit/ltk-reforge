# GAME-CHANGE-0006: Emitter shape as SpawnShape and EmitterPosition

- **Build:** 14.1
- **Affects:** `VfxEmitterDefinitionData`, `VfxShape`, `IVfxShape` and the classes derived from
  it: `VfxShapeLegacy`, `VfxShapeBox`, `VfxShapeCylinder`, `VfxShapeSphere`
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
`VfxShapeBox`, `VfxShapeCylinder` and `VfxShapeSphere`, each a volume, or `VfxShapeLegacy`, which
holds `emitOffset`, `emitRotationAngles` and `emitRotationAxes` as `VfxShape` held them.

`VfxShapeLegacy` declares no `birthTranslation`.

## Riot's conversion

- `birthTranslation` became the emitter's `EmitterPosition`, its value unchanged.
- The other three fields became the `SpawnShape`. Riot rewrote many shapes as the volume they
  drew: a random offset as a `VfxShapeBox`, `VfxShapeSphere` or `VfxShapeCylinder` of the
  offset's range, a constant offset under a random rotation as a `VfxShapeSphere` of that
  radius. It kept the others as a `VfxShapeLegacy` with the three fields unchanged.
- No other field of the emitter moved.

Which shapes Riot rewrote as a volume and which it kept is not established as a rule.
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
- No game object from 10.9 to 16.19 holds a `birthTranslation` in a `VfxShapeLegacy`
  (ltk-forge's census lookup, 2026-09-30).
