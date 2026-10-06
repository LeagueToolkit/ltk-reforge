# GAME-BEHAVIOR-0024: A simple emitter is drawn by its LegacySimple

- **Status:** Confirmed in game (2026-10-06)
- **Builds:** 16.19
- **Affects:** `VfxSystemDefinitionData.simpleEmitterDefinitionData`,
  `VfxEmitterDefinitionData.LegacySimple`, `VfxEmitterLegacySimple`
- **Related:** #GAME-CHANGE-0004, #GAME-BEHAVIOR-0002

## Behaviour

The client draws the particles of an emitter a system lists as simple
(`simpleEmitterDefinitionData`) at the size its `LegacySimple` gives. A simple emitter with no
`LegacySimple`, and no `birthScale0` or `scale0`, is not seen.

The client reads no `LegacySimple` on an emitter a system lists as complex
(`complexEmitterDefinitionData`): one given a `LegacySimple` is drawn as without it. Checked on one
complex emitter, a camera-facing quad with no primitive; a complex emitter with a primitive is not
checked.

## In a mod

A simple emitter of a mod made before 12.19 holds its size in `birthScale1` and `scale1` on the
emitter. A later build drops those fields (#GAME-BEHAVIOR-0002), the emitter holds no
`LegacySimple`, and its particles are not seen when nothing else sets their size.

A complex emitter of such a mod is drawn the same with its old fields in a `LegacySimple` and
without them.

## Evidence

- Seen in game (2026-10-06, the 16.19.8207193 copy, a replay of Vi at skin 0). Three test mods,
  each Vi's `skin0.bin` with one change to the emitter `glow` of `Vi_Base_Passive_BackPack`: a
  simple emitter, the blue ring on her back while her passive is ready. Its `LegacySimple` holds
  `birthScale` 25 and a `scale` curve from 0 to 1.5, and it holds no `birthScale0` or `scale0`.
  - `kept-control`, unchanged: Vi as without the mod.
  - `kept-scaled`, each value of the `scale` curve four times the game's: the ring very big.
  - `kept-removed`, the `LegacySimple` removed: no ring on her back.
- Seen in game (2026-10-06, the same copy, a replay of Zac at skin 0). Four test mods, each Zac's
  `skin0.bin` with at most one change to an emitter of `Zac_Base_W_cas`, the effect of his W.
  `Shadow` is a simple emitter, a dark swirl projected on the ground, whose `LegacySimple` holds
  `birthScale` 250 and a `scale` curve from 0.35 to 1. `Splat` is a complex emitter, the green goo
  bursting up, alpha-blended at full opacity, with a `birthScale0` of 75 by 200 and no
  `LegacySimple`.
  - `control`, unchanged: Zac as without the mod.
  - `kept-scaled`, each value of the `scale` curve of `Shadow` four times the game's: the swirl
    big.
  - `kept-removed`, the `LegacySimple` of `Shadow` removed: no swirl seen.
  - `simple-scaled`, `Splat` given a `LegacySimple` of `birthScale` 200 and `scale` 4: no
    difference from the control noticed. Drawn by it the way a simple emitter is, the goo would
    be about 800 units tall.
  - `simple-zeroed`, `Splat` given a `LegacySimple` of `birthScale` 0 and `scale` 0: no
    difference from the control seen; the goo is drawn. Drawn by it, the goo would not be seen.
