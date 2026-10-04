# GAME-CHANGE-0017: Primitive mesh camera alignment on both axes

- **Build:** 13.8.5036621
- **Affects:** `VfxPrimitiveMesh`, `VfxPrimitiveAttachedMesh` and their base
  `VfxPrimitiveMeshBase`
- **Related:** #GAME-BEHAVIOR-0002

## Before

Up to 13.7 an emitter's mesh primitive, a `VfxPrimitiveMesh` or a `VfxPrimitiveAttachedMesh`,
holds two flags that turn the mesh to the camera: `mAlignYawToCamera: bool` and
`mAlignPitchToCamera: bool`. A game mesh holds either, both or neither. Each one held is `true`.

## After

From 13.8 the primitive holds `AlignYawToCamera: bool` and `AlignPitchToCamera: bool`, declared
on `VfxPrimitiveMeshBase`. The two classes do not declare the old names.

## Riot's conversion

Every mesh that held either old flag holds both new flags, `true`. The conversion is not a
rename of each flag: a mesh aligned on one axis came out aligned on both.

| up to 13.7 | from 13.8 |
|---|---|
| `mAlignPitchToCamera` alone | `AlignYawToCamera` and `AlignPitchToCamera` |
| `mAlignYawToCamera` alone | `AlignYawToCamera` and `AlignPitchToCamera` |
| both | `AlignYawToCamera` and `AlignPitchToCamera` |
| neither | neither |

No game mesh holds one new flag alone at 13.8.5036621. Meshes that hold one alone exist at
later builds, so the client reads the two apart.

Whether a mesh aligned on one axis was drawn the same before and after the build is not
established. Riot's data says only that both flags were set.

## In a mod

A mod made before 13.8 holds the old flags on the primitive. A later build drops them
(#GAME-BEHAVIOR-0002): both new flags keep their default, `false`, and the mesh does not turn to
the camera.

A mod made later holds the old flags too when its author pasted an emitter from an older mod.
The author's own build dropped them.

## Evidence

- Counted in the census history's entry texts by the classes' and the fields' hashes
  (2026-10-04). At 13.7.5031019, of 230,467 mesh primitives, 1,163 hold
  `mAlignPitchToCamera` alone, 517 `mAlignYawToCamera` alone and 632 both. None holds `false`.
  At 13.8.5036621, of 234,318, 2,334 hold both new flags, none holds one alone and none holds an
  old flag.
- The same primitives paired across the two builds: 230,232 are in both. Each of the 2,312 that
  held an old flag holds both new flags, and each of the 227,920 that held none holds none.
- At 13.15.5229820, 406 primitives hold `AlignYawToCamera` alone and 11 `AlignPitchToCamera`
  alone. At 16.19.8207193 it is 1,265 and 569.
- ltk-forge paired 1,669 of the game's meshes across the build on the census and found no
  exception (2026-10-01).
- The meta database declares no types before 13.15, so the layout up to 13.7 is the fields the
  census saw on the game's primitives.
- The meta database (generation 2026-09-28, read 2026-10-04): from 13.15 to 16.19
  `VfxPrimitiveMeshBase` declares `AlignYawToCamera` and `AlignPitchToCamera` as bools, and
  `VfxPrimitiveMesh` and `VfxPrimitiveAttachedMesh` derive from it and declare no
  `mAlignYawToCamera`.
- That the default of the new flags is `false` is inferred from the game's data, where most
  primitives hold neither. The meta database gives no default for them.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 6 of the 5,137 mods
  hold an old flag, on 53 primitives. Each primitive holds a `true`.
