# MOD-ARTIFACT-0020: Fields no game build declared

- **Source:** tools that write bins; which is not established
- **Affects:** `ValueColor`, `VfxEmitterDefinitionData`, `VfxSystemDefinitionData`,
  `StaticMaterialDef`
- **Related:** #GAME-BEHAVIOR-0002, #GAME-BEHAVIOR-0026

## What is found

A field on a class of the game, under a hash that no build declared on that class and no game
object holds there. Seven are known:

| class | field hash | holds, in the mods |
|---|---|---|
| `ValueColor` | `0xecbec3b3` | a struct of class `0x44d038f0` with two empty lists, of `f32` and of `vec4`: an empty colour curve |
| `ValueColor` | `0x20e61d8a` | a colour, beside `constantValue` or in its place |
| `VfxEmitterDefinitionData` | `0x086379e1` | a flag |
| `VfxEmitterDefinitionData` | `0x82a515a5` | a flag |
| `VfxEmitterDefinitionData` | `0x3040923e` | `false` |
| `VfxSystemDefinitionData` | `0xc750ee97` | a list of emitters |
| `StaticMaterialDef` | `0xf3d3de85` | a list of samplers |

`0x3040923e` is the hash of `isLocalSpace`, which the game declares on two field classes,
`VfxFieldAccelerationDefinitionData` and `VfxFieldOrbitalDefinitionData`, and not on an
emitter. The names the other six hashes were made from are not known.

It is told from a field Riot renamed or removed by its history: a field of an older layout is
on the game's objects at some build. These are on none.

That no build declared them rests on the meta schema from 13.15. Before 13.15 it rests on the
census alone, which does not show a field that a build declared and every game object left at
its default (#GAME-BEHAVIOR-0026).

## Why it is there

Not established. The empty colour curve is in 210 mods, so it is a tool's and not one author's.
Which tools write the seven, and what name each hash was meant to be, is not known.

## What reads it, and how

The client drops a field its class does not declare (#GAME-BEHAVIOR-0002). No build from 13.15
declares these, so no author of a mod made since saw what one holds: the colour, the flags, the
list of emitters and the list of samplers never showed. For a mod made before 13.15 that is
inferred (What is found).

## The repair

None. The field has no form at any build, and what the author meant by it is not in the file.
Removing it changes nothing a build from 13.15 drew (`fixup.removed`). For an earlier build that
is inferred.

Only the class of the table loses the field. The same hash on another class is another field.

## Evidence

- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 281 of the 5,137
  mods hold one of the seven.

  | class and hash | mods | values |
  |---|---:|---:|
  | `ValueColor.0xecbec3b3` | 210 | 6,615 |
  | `ValueColor.0x20e61d8a` | 81 | 2,044 |
  | `VfxEmitterDefinitionData.0x3040923e` | 40 | 197 |
  | `VfxEmitterDefinitionData.0x82a515a5` | 28 | 94 |
  | `VfxEmitterDefinitionData.0x086379e1` | 27 | 93 |
  | `VfxSystemDefinitionData.0xc750ee97` | 11 | 11 |
  | `StaticMaterialDef.0xf3d3de85` | 1 | 1 |

- The meta database (generation 2026-09-28, read 2026-10-04): no class declares a property
  under any of the six hashes other than `0x3040923e`, at any build from 13.15 to 16.19.
  `0x3040923e` is `isLocalSpace`, a bool with default `true`, on
  `VfxFieldAccelerationDefinitionData` and `VfxFieldOrbitalDefinitionData` alone. `ValueColor`
  declares `constantValue` and `dynamics` and no other field.
- Counted in the census history's entry texts by the field's hash (2026-10-04), at 18 builds
  from 8.20.2483196 to 16.19.8207193: no line holds one of the six hashes, in any class.
- `0x3040923e` by the line's depth (the same count), at 13.1.4875491, 13.15.5229820,
  14.1.5516065 and 16.19.8207193: every line is at the depth of a field force's fields, and
  none is at an emitter's. At 16.19.8207193 it is 10,183 lines.
- LTK Manager's VFX renderer plan (read 2026-10-07), over the 174 champion WADs at 16.17 (its
  section 2.36): the game's acceleration and orbital fields hold `isLocalSpace` only as `false`,
  6,427 and 1,956 times. `false` is what the mods hold under `0x3040923e` on the emitter.
- ltk-forge's notes state that no name table spells the six hashes and that the install's bins
  do not hold them (2026-09-30). Not checked again here.
- What the mods hold under each hash is ltk-forge's description of them. The reports of the
  baseline run give it and not the values.
- TopazModFixer, the fixer of the cslol-go manager, reads `StaticMaterialDef.0xf3d3de85` as the
  material's list of samplers where `samplerValues` is absent, renames the fields of its
  samplers and leaves the list under `0xf3d3de85` (source of TopazModFixerCLI, read
  2026-10-05). A tool's author took the field for a list of samplers too. Which tool writes it
  is not established.
- That removing the fields changes nothing drawn is inferred from #GAME-BEHAVIOR-0002. No check
  in game is recorded.
