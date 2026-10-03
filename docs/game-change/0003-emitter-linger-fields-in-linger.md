# GAME-CHANGE-0003: Emitter linger fields in Linger

- **Build:** 12.18 for the drag, 12.20 for the other fields
- **Affects:** `VfxEmitterDefinitionData`, `VfxLingerDefinitionData`
- **Related:** #GAME-BEHAVIOR-0002

## Before

Up to 12.17 an emitter holds its linger settings as fields of its own: a drag, a colour, a
velocity, an acceleration, a scale and a rotation, each with a flag beside it.

## After

From 12.20 an emitter holds `Linger: pointer = VfxLingerDefinitionData`, and the twelve fields
are fields of that struct, ten of them under another name. The emitter's class does not declare
the old names.

At 12.18 and 12.19 an emitter holds both: `Linger` with the drag and its flag, and the other
ten fields on the emitter.

## Riot's conversion

Each field moved into `Linger`, its value unchanged.

| on the emitter | in `Linger` | moved at |
|---|---|---|
| `lingerDrag` | `KeyedLingerDrag` | 12.18 |
| `UseLingerDrag` | `UseKeyedLingerDrag` | 12.18 |
| `lingerColor` | `SeparateLingerColor` | 12.20 |
| `useLingerColor` | `UseSeparateLingerColor` | 12.20 |
| `lingerVelocity` | `KeyedLingerVelocity` | 12.20 |
| `UseLingerVelocity` | `UseKeyedLingerVelocity` | 12.20 |
| `lingerAcceleration` | `KeyedLingerAcceleration` | 12.20 |
| `UseLingerAcceleration` | `UseKeyedLingerAcceleration` | 12.20 |
| `LingerScale0` | `LingerScale` | 12.20 |
| `UseLingerScale` | `UseLingerScale` | 12.20 |
| `LingerRotation0` | `LingerRotation` | 12.20 |
| `UseLingerRotation` | `UseLingerRotation` | 12.20 |

At 12.20 the ten fields joined the `Linger` that 12.18 made. No second struct was made.

## In a mod

A mod made before 12.18 holds all twelve fields on the emitter. A mod made at 12.18 or 12.19
holds a `Linger` with the drag, and the other ten on the emitter. A later build drops what is
on the emitter (#GAME-BEHAVIOR-0002): those linger settings keep their defaults, and what the
author set does not apply.

A mod made later holds the old fields too when its author pasted an emitter from an older mod.
The author's own build dropped them.

## Evidence

- ltk-forge paired the game's emitters across 12.17 and 12.18 and across 12.19 and 12.20 on the
  census and looked for each value that left an emitter in the same emitter at the next build
  (2026-10-01). Every pair is equal. `lingerColor` to `SeparateLingerColor`: 5,609 of 5,609
  paired values.
- A flag's value does not tell which field it moved to, since every `true` equals every other.
  The six flags are paired by name: each left the emitter at the build its value's field left,
  and its new name appeared in `Linger` at that build.
- The meta database declares no types before 13.15, so the layouts up to 12.20 are the fields
  the census saw on the game's emitters.
- The meta database (generation 2026-09-28, read 2026-10-03): from 13.15 to 16.19
  `VfxEmitterDefinitionData.Linger` is a pointer to `VfxLingerDefinitionData`, the class declares
  the twelve new names and no other field, and the emitter declares no `lingerColor`.
