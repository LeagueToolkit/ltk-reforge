# GAME-CHANGE-0005: Emitter texture mult in textureMult

- **Build:** 13.1
- **Affects:** `VfxEmitterDefinitionData`, `VfxTextureMultDefinitionData`
- **Related:** #GAME-BEHAVIOR-0002

## Before

Up to 12.23 an emitter holds a second texture as `textureMult: string`, a path. The fields that
place and scroll that texture are fields of the emitter beside it, each with `Mult` in its
name.

## After

From 13.1 the emitter's `textureMult` is a `pointer = VfxTextureMultDefinitionData`. The path is
that struct's own `textureMult: string`, and the placing and scrolling fields are fields of the
struct, four of them under another name. The emitter's class does not declare them.

The field keeps its name and changes its type: `textureMult` on an emitter is a string up to
12.23 and a struct from 13.1.

## Riot's conversion

The path and each field moved into the struct, its value unchanged.

| on the emitter, to 12.23 | in `textureMult`, from 13.1 |
|---|---|
| `textureMult` | `textureMult` |
| `birthUvScrollRateMult` | `birthUvScrollRateMult` |
| `uvScaleMult` | `uvScaleMult` |
| `birthUVOffsetMult` | `birthUVOffsetMult` |
| `particleUVScrollRateMult` | `ParticleIntegratedUvScrollMult` |
| `particleUVRotateRateMult` | `ParticleIntegratedUvRotateMult` |
| `texAddressModeMult` | `texAddressModeMult` |
| `texDivMult` | `texDivMult` |
| `birthUvRotateRateMult` | `birthUvRotateRateMult` |
| `uvScrollClampMult` | `uvScrollClampMult` |
| `UvRotationMult` | `UvRotationMult` |
| `uvScrollAlphaMult` | `uvScrollAlphaMult` |
| `uvTransformCenterMult` | `uvTransformCenterMult` |
| `emitterUvScrollRateMult` | `emitterUvScrollRateMult` |
| `flexBirthUVScrollRateMult` | `flexBirthUVScrollRateMult` |
| `isRandomStartFrameMult` | `isRandomStartFrameMult` |
| `TextureMultFlipU` | `TextureMultFilpU` |
| `TextureMultFlipV` | `TextureMultFilpV` |

`Filp` is the game's own spelling of the two new names.

## In a mod

A mod made before 13.1 holds a string in the emitter's `textureMult` and the `Mult` fields on
the emitter. A later build reads a struct there and drops the string, and it drops the `Mult`
fields the emitter does not declare (#GAME-BEHAVIOR-0002). The second texture is not drawn.

A mod made later holds the old layout too when its author pasted an emitter from an older mod.
The author's own build dropped it, so the author saw the emitter without its second texture.

## Evidence

- ltk-forge paired 15,669 of the game's emitters across 12.23 and 13.1 on the census and looked
  for each value that left an emitter in the same emitter at 13.1 (2026-09-30). The renames of
  the scroll rate and the rotate rate are read off equal values under the two names.
- The two renamed rates keep their value's class (counted in the census history's object texts,
  2026-10-07): `particleUVScrollRateMult` is an embed of `IntegratedValueVector2` and
  `particleUVRotateRateMult` of `IntegratedValueFloat` at 12.23.4875509, and so are
  `ParticleIntegratedUvScrollMult` and `ParticleIntegratedUvRotateMult` at 13.1.4875491.
- A flag's value does not tell which field it moved to, since every `true` equals every other.
  The flags are paired by name: each left the emitter at 13.1, and its name, or for the two
  flips the name with `Filp`, appeared in the struct at 13.1.
- One mod of the datasets made at 15.8 or later holds 17 emitters with a string in `textureMult`
  (ltk-forge's notes, no date recorded).
- The meta database declares no types before 13.15, so the layout up to 12.23 is the fields the
  census saw on the game's emitters.
- The meta database (generation 2026-09-28, read 2026-10-03): from 13.15 to 16.19
  `VfxEmitterDefinitionData.textureMult` is a pointer to `VfxTextureMultDefinitionData`,
  `VfxTextureMultDefinitionData.textureMult` is a string, and the class declares the eighteen
  names of the right column and no other field.
