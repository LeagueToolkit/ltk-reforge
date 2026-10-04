# MOD-ARTIFACT-0019: A birth translation inside VfxShapeLegacy

- **Source:** a tool that converts an emitter to the layout of 14.1; which is not established
- **Affects:** `VfxEmitterDefinitionData.SpawnShape`, `VfxShapeLegacy`,
  `VfxEmitterDefinitionData.EmitterPosition`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-CHANGE-0006

## What is found

An emitter whose `SpawnShape` is a `VfxShapeLegacy` that holds a `birthTranslation`:

```
SpawnShape: pointer = VfxShapeLegacy {
    birthTranslation: embed = ValueVector3 { ... }
    emitOffset: embed = ValueVector3 { ... }
}
```

It is told from deliberate work by the class. `VfxShapeLegacy` declares `emitOffset`,
`emitRotationAngles` and `emitRotationAxes` and no `birthTranslation`, and no shape of the game
holds one.

## Why it is there

Up to 13.24 an emitter's `shape`, a `VfxShape`, held four fields, `birthTranslation` among them.
At 14.1 Riot moved `birthTranslation` to the emitter's `EmitterPosition` and the other three to
the `SpawnShape` (#GAME-CHANGE-0006).

The shape is that of a tool that turned the old `shape` into a `VfxShapeLegacy` with all four
fields. That is inferred from the shape. Which tool did it is not established.

## What reads it, and how

The client drops a field its class does not declare (#GAME-BEHAVIOR-0002). The emitter holds no
`EmitterPosition`, so its position is the default, the origin, and the effect is drawn without
the offset the old emitter had.

The author's own build dropped it too, where the mod was made at 14.1 or later.

## The repair

The value as the emitter's `EmitterPosition`, and no `birthTranslation` in the shape: what
Riot's conversion made of a `birthTranslation` (#GAME-CHANGE-0006).

## Evidence

- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 45 of the 5,137 mods
  hold it, 14,899 values.
- ltk-forge's notes give 42 mods and 15,394 values over the datasets, with no date recorded.
- The meta database (generation 2026-09-28, read 2026-10-04): `VfxShapeLegacy` exists from
  14.1 and declares `emitOffset`, `emitRotationAngles` and `emitRotationAxes` at every build to
  16.19. `VfxEmitterDefinitionData.EmitterPosition` is an embed of `ValueVector3` from 14.1,
  with a default of zero on each axis.
- Counted in the census history's entry texts by the field's hash (2026-10-04): 404,769 lines
  hold a `birthTranslation` at 13.15.5229820, each at the depth of a `shape`'s fields. No line
  holds one at 14.1.5516065 or at 16.19.8207193, in any class.
- No game object from 10.9 to 16.19 holds a `birthTranslation` in a `VfxShapeLegacy`
  (ltk-forge's census lookup, 2026-09-30).
- That the client draws the emitter at the origin is inferred from #GAME-BEHAVIOR-0002 and the
  default. No check in game is recorded.
