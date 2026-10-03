# GAME-CHANGE-0002: Emitter filters in Filtering

- **Build:** 12.17
- **Affects:** `VfxEmitterDefinitionData`, `VfxEmitterFiltering`
- **Related:** #GAME-BEHAVIOR-0002

## Before

Up to 12.16 an emitter holds its filters as fields of its own: `keywordsExcluded`,
`keywordsRequired`, `spectatorPolicy` and `censorPolicy`.

## After

From 12.17 an emitter holds `Filtering: pointer = VfxEmitterFiltering`, and the four fields are
fields of that struct. The emitter's class does not declare them.

`VfxEmitterFiltering` also declares `keywordsIncluded`, which no field of the old emitter moved
into.

## Riot's conversion

Each of the four fields moved into `Filtering` under the same name, its value unchanged.

| on the emitter, to 12.16 | in `Filtering`, from 12.17 |
|---|---|
| `keywordsExcluded` | `keywordsExcluded` |
| `keywordsRequired` | `keywordsRequired` |
| `spectatorPolicy` | `spectatorPolicy` |
| `censorPolicy` | `censorPolicy` |

An emitter that held none of the four got no `Filtering`.

## In a mod

A mod made before 12.17 holds the four fields on the emitter. A later build drops them
(#GAME-BEHAVIOR-0002), so the filters keep their defaults and a filter the author set does not
apply.

A mod made later holds them there too when its author pasted an emitter from an older mod. The
author's own build dropped them, so the author saw the emitter unfiltered.

## Evidence

- ltk-forge paired the game's emitters across 12.16 and 12.17 on the census and looked for each
  value that left an emitter in the same emitter at 12.17 (2026-10-01). `keywordsExcluded`:
  9,563 of 9,563 paired values are equal in `Filtering`.
- The meta database declares no types before 13.15, so the layout up to 12.16 is the fields the
  census saw on the game's emitters.
- The meta database (generation 2026-09-28, read 2026-10-03): from 13.15 to 16.19
  `VfxEmitterDefinitionData.Filtering` is a pointer to `VfxEmitterFiltering`, the class declares
  `censorPolicy`, `keywordsExcluded`, `keywordsIncluded`, `keywordsRequired` and
  `spectatorPolicy`, and the emitter declares no `keywordsExcluded`.
