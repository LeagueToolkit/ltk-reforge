# GAME-CHANGE-0004: Emitter simple-particle fields in LegacySimple

- **Build:** 12.19; at 12.20 most of the game's emitters lost the struct
- **Affects:** `VfxEmitterDefinitionData`, `VfxEmitterLegacySimple`
- **Related:** #GAME-BEHAVIOR-0002

## Before

Up to 12.18 an emitter holds the scale, rotation and scroll of a simple particle as fields of
its own, six of them with a `1` at the end of the name: `birthScale1`, `scale1`,
`birthRotation1`, `rotation1`, `birthRotationalVelocity1`, `uvScrollRate1`. Beside them it
holds `particleBind`, `orientation`, `scaleBias`, `hasFixedOrbit` and `scaleUpFromOrigin`.

## After

From 12.19 an emitter holds `LegacySimple: pointer = VfxEmitterLegacySimple`, and the eleven
fields are fields of that struct, the six without their `1`. The emitter's class does not
declare them.

`VfxEmitterLegacySimple` also declares `fixedOrbitType` and `lockedToEmitter`, which no field
of the old emitter moved into.

From 12.20 about one game emitter in fifty still holds a `LegacySimple`. The others hold none, and
nothing holds their old values.

A system lists each emitter as simple (`simpleEmitterDefinitionData`) or complex
(`complexEmitterDefinitionData`). At 16.19 nearly every simple emitter holds a `LegacySimple`, and
nearly no complex one does.

## Riot's conversion

Each field moved into `LegacySimple`, its value unchanged.

| on the emitter, to 12.18 | in `LegacySimple`, from 12.19 |
|---|---|
| `birthScale1` | `birthScale` |
| `scale1` | `scale` |
| `birthRotation1` | `birthRotation` |
| `rotation1` | `rotation` |
| `birthRotationalVelocity1` | `birthRotationalVelocity` |
| `uvScrollRate1` | `uvScrollRate` |
| `particleBind` | `particleBind` |
| `orientation` | `orientation` |
| `scaleBias` | `scaleBias` |
| `hasFixedOrbit` | `hasFixedOrbit` |
| `scaleUpFromOrigin` | `scaleUpFromOrigin` |

At 12.20 Riot removed the `LegacySimple` from most emitters that had one, with every value in
it, and put nothing in its place. At 16.19 the emitters that hold one are the simple emitters,
with few exceptions (After). That this was the rule of the removal is inferred from 16.19; the
emitters of 12.20 are not counted by the list that holds them.

## In a mod

A mod made before 12.19 holds the eleven fields on the emitter. A later build drops them
(#GAME-BEHAVIOR-0002): the particle is drawn with the default of each, so the scale, rotation
and scroll rate the author set do not apply. A simple emitter's particles are not seen when
nothing else sets their size, and a complex emitter is drawn the same with or without the fields
in a `LegacySimple` (#GAME-BEHAVIOR-0024).

A mod made later holds them there too when its author pasted an emitter from an older mod. The
author's own build dropped them.

## Evidence

- ltk-forge paired 15,767 of the game's emitters across 12.18 and 12.19 on the census and
  looked for each value that left an emitter in the same emitter at 12.19 (2026-09-30). Every
  paired value is equal. `birthScale1` to `birthScale`: 2,491 of 2,491.
- The meta database declares no types before 13.15, so the layout up to 12.18 is the fields the
  census saw on the game's emitters.
- The meta database (generation 2026-09-28, read 2026-10-03): from 13.15 to 16.19
  `VfxEmitterDefinitionData.LegacySimple` is a pointer to `VfxEmitterLegacySimple`, the class
  declares the eleven new names with `fixedOrbitType` and `lockedToEmitter`, and the emitter
  declares no `birthScale1`.
- Counted in the census history's entry texts by the field's hash (2026-10-03): 57,673 emitters
  in 25,195 entries hold a `LegacySimple` at 12.19.4723608, 1,056 in 558 at 12.20.4732751, 1,077
  in 576 at 13.15.5229820 and 1,291 in 833 at 16.19.8207193. None holds one at 12.18.4683251.
- One emitter of one entry compared across 12.19.4723608 and 12.20.4732751: its `LegacySimple`
  is removed whole and the entry gains no line.
- `scaleUpFromOrigin` is held by one game emitter at 12.18.4683251 and at 12.19.4723608, there
  inside its `LegacySimple`, and by none at 12.20.4732751, 13.15.5229820 or 16.19.8207193
  (the same count, 2026-10-03). The class declares it, as a bool, at every build from 13.15 to
  16.19. The field did not leave the class: the one emitter that held it lost its
  `LegacySimple` at 12.20.
- Counted in the 16.19.8207193 install copy (2026-10-06), over the skin bins
  `data/characters/<name>/skins/skin<n>.bin`, n from 0 to 99, of the 174 WADs of
  `DATA/FINAL/Champions` that are no locale's: 369 of the 382 emitters in a
  `simpleEmitterDefinitionData` hold a `LegacySimple`, and 37 of the 794,451 in a
  `complexEmitterDefinitionData`, each of the 37 beside a `birthScale0`. The bins a skin bin links
  are not counted.
- Counted in the reports of both baselines (2026-10-04, counted 2026-10-06): the mods hold the
  old fields on 847 complex emitters in 16 mods, 5,811 values, and on 46 simple emitters in 7
  mods, 132 values.
- How the client draws an emitter by its `LegacySimple`, and what is not established of it:
  #GAME-BEHAVIOR-0024.
