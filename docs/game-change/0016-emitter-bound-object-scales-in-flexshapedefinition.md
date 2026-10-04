# GAME-CHANGE-0016: Emitter bound-object scales in FlexShapeDefinition

- **Build:** 12.6.4302692 for the emit-offset scales and the two flex values, 12.21.4760803 for
  the birth-scale scales
- **Affects:** `VfxEmitterDefinitionData`, `VfxFlexShapeDefinitionData`
- **Related:** #GAME-BEHAVIOR-0002

## Before

Up to 12.5 an emitter holds, as fields of its own, how its shape follows the size of the object
it is bound to:

| field of the emitter | type |
|---|---|
| `scaleEmitOffsetByBoundObjectSize` | `f32` |
| `scaleEmitOffsetByBoundObjectHeight` | `f32` |
| `scaleEmitOffsetByBoundObjectRadius` | `f32` |
| `flexScaleEmitOffset` | a pointer |
| `flexBirthTranslation` | a pointer |
| `scaleBirthScaleByBoundObjectSize` | `f32` |
| `scaleBirthScaleByBoundObjectHeight` | `f32` |
| `scaleBirthScaleByBoundObjectRadius` | `f32` |

## After

From 12.21 an emitter holds `FlexShapeDefinition: pointer = VfxFlexShapeDefinitionData`, and the
eight fields are fields of that struct under the same names. The emitter's class does not
declare them.

From 12.6 to 12.20 an emitter holds both: `FlexShapeDefinition` with the first five fields, and
the three `scaleBirthScale...` fields on the emitter.

`VfxFlexShapeDefinitionData` also declares `ScaleBirthTranslationByBoundObjectSize`, which no
field of the old emitter moved into. Six game emitters hold it at 12.6.4302692.

## Riot's conversion

Each field moved into `FlexShapeDefinition` under the same name, its value unchanged.

| field | moved at |
|---|---|
| `scaleEmitOffsetByBoundObjectSize` | 12.6 |
| `scaleEmitOffsetByBoundObjectHeight` | 12.6 |
| `scaleEmitOffsetByBoundObjectRadius` | 12.6 |
| `flexScaleEmitOffset` | 12.6 |
| `flexBirthTranslation` | 12.6 |
| `scaleBirthScaleByBoundObjectSize` | 12.21 |
| `scaleBirthScaleByBoundObjectHeight` | 12.21 |
| `scaleBirthScaleByBoundObjectRadius` | 12.21 |

At 12.21 the three fields joined the `FlexShapeDefinition` that 12.6 made, where the emitter
had one. An emitter that had none got one.

## In a mod

A mod made before 12.6 holds the eight fields on the emitter. A mod made from 12.6 to 12.20
holds a `FlexShapeDefinition` with five of them, and the three birth-scale fields on the
emitter. A later build drops what is on the emitter (#GAME-BEHAVIOR-0002): those scales keep
their defaults, and the emitter does not follow the size of its bound object as the author set
it.

A mod made later holds the old fields too when its author pasted an emitter from an older mod.
The author's own build dropped them.

## Evidence

- Counted in the census history's entry texts by the class's and the fields' hashes
  (2026-10-04), on the emitter and in its `FlexShapeDefinition`:

  | field | on the emitter, 12.5.4259171 | in the struct, 12.6.4302692 |
  |---|---:|---:|
  | `scaleEmitOffsetByBoundObjectSize` | 43,763 | 43,353 |
  | `scaleEmitOffsetByBoundObjectHeight` | 1,211 | 1,181 |
  | `scaleEmitOffsetByBoundObjectRadius` | 151 | 133 |
  | `flexScaleEmitOffset` | 27 | 27 |
  | `flexBirthTranslation` | 5 | 5 |

  | field | on the emitter, 12.20.4748882 | in the struct, 12.21.4760803 |
  |---|---:|---:|
  | `scaleBirthScaleByBoundObjectSize` | 72,533 | 73,337 |
  | `scaleBirthScaleByBoundObjectHeight` | 495 | 508 |
  | `scaleBirthScaleByBoundObjectRadius` | 51 | 51 |

  No emitter holds one of the first five on itself at 12.6.4302692, or one of the last three at
  12.21.4760803. No emitter holds a `FlexShapeDefinition` at 12.5.4259171.
- The same emitters paired across 12.5.4259171 and 12.6.4302692: 80,531 emitters that hold one
  of the fields are in both builds. Each value that left the emitter is equal in the struct
  under the same name: 42,187, 1,180, 131, 27 and 5 values in the table's order. The three
  birth-scale fields stay on the emitter, equal.
- Paired across 12.20.4748882 and 12.21.4760803: 102,441 emitters are in both.
  `scaleBirthScaleByBoundObjectSize` is equal in the struct in 72,518 of 72,523, `...Height` in
  495 of 495 and `...Radius` in 51 of 51. The other 5 are in entries whose lists of emitters
  changed at that build.
- ltk-forge paired 7,756 of the game's emitters across 12.20 and 12.21 on the census
  (2026-10-01): `scaleBirthScaleByBoundObjectSize`, 809 of 809 paired values are equal in
  `FlexShapeDefinition`.
- The meta database declares no types before 13.15, so the layouts up to 12.20 are the fields
  the census saw on the game's emitters.
- The meta database (generation 2026-09-28, read 2026-10-04): from 13.15 to 16.19
  `VfxEmitterDefinitionData.FlexShapeDefinition` is a pointer to `VfxFlexShapeDefinitionData`,
  the class declares the eight names and `ScaleBirthTranslationByBoundObjectSize` and no other
  field, and the emitter declares no `scaleEmitOffsetByBoundObjectSize` and no
  `scaleBirthScaleByBoundObjectSize`.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 14 of the 5,137 mods
  hold one of the fields on an emitter. `scaleEmitOffsetByBoundObjectSize`: 169 values in 12
  mods. `scaleBirthScaleByBoundObjectSize`: 262 in 11. `scaleBirthScaleByBoundObjectHeight`: 17
  in 1. ltk-forge moved those three and `scaleBirthScaleByBoundObjectRadius`, and not the other
  four.
