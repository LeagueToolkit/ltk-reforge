# GAME-BEHAVIOR-0024: A simple emitter is drawn by its LegacySimple

- **Status:** Confirmed in game (2026-10-06)
- **Builds:** 16.19 in game; 16.17 for LTK Manager's reading of the client
- **Affects:** `VfxSystemDefinitionData.simpleEmitterDefinitionData`,
  `VfxEmitterDefinitionData.LegacySimple`, `VfxEmitterLegacySimple`
- **Related:** #GAME-CHANGE-0004, #GAME-BEHAVIOR-0002

## Behaviour

The client draws the particles of an emitter a system lists as simple
(`simpleEmitterDefinitionData`) at the size its `LegacySimple` gives: `birthScale` times
`scale`, times `scaleBias` on each axis, from the particle's centre to each edge. A simple emitter
with no `LegacySimple`, and no `birthScale0` or `scale0`, is not seen: the struct's defaults give
one unit from the centre to each edge.

The client reads no `LegacySimple` on an emitter a system lists as complex
(`complexEmitterDefinitionData`): one given a `LegacySimple` is drawn as without it. Checked in
game on one complex emitter, a camera-facing quad with no primitive. LTK Manager's reading of the
client makes no exception for a primitive; a complex emitter with a primitive is not checked in
game.

LTK Manager's reading also has a simple emitter read no `Linger`, no `particleLingerType`, no
motion over its particles' life and no stencil mode. That part is not checked in game.

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
    reach 800 units from its centre to each edge, about 1,600 units across.
  - `simple-zeroed`, `Splat` given a `LegacySimple` of `birthScale` 0 and `scale` 0: no
    difference from the control seen; the goo is drawn. Drawn by it, the goo would not be seen.
- LTK Manager's VFX renderer plan (read 2026-10-07), a reading of the 16.17 client. Its section
  2.13: the simple quad is `P ± A ± B` with `A = U * scale * scaleBias.y` and
  `B = V * scale * scaleBias.x`, "so the size is a half-extent". Its tier T8: "one `birthScale`
  half-extent times `scaleBias` covers both extents".
- The same plan's section 2.52, a static read it states was not checked in game: "The list an
  emitter is in makes it simple, so `LegacySimple` defaults where it is unwritten and is ignored
  on a complex emitter. A simple emitter reads none of the over-life motion, the emitter's own
  frame, `Linger` or `particleLingerType`". Its section 2.53: the client reads the stencil mode
  on complex emitters only.
- `rito-meta` (generation 2026-10-05, read 2026-10-07): `VfxEmitterLegacySimple.birthScale` and
  `scale` default to a constant 1, and `scaleBias` to `[1, 1]`.
- Counted in the census history's object texts at 16.19.8207193, every WAD (2026-10-07): 1,231
  of the 1,258 emitters in a `simpleEmitterDefinitionData` hold a `LegacySimple`. None of the
  other 27 holds a `birthScale0` or a `scale0`.
