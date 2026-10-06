# GAME-CHANGE-0025: A flag on camera-aligned meshes

- **Build:** 13.24.5461574
- **Affects:** `VfxPrimitiveMeshBase`, `VfxPrimitiveMesh`, `VfxPrimitiveAttachedMesh`; the field
  `0x6aec9e7a`
- **Related:** #GAME-CHANGE-0017, #GAME-BEHAVIOR-0026

## Before

Up to 13.23 a mesh primitive holds `AlignYawToCamera` and `AlignPitchToCamera`
(#GAME-CHANGE-0017) and no other field of its alignment.

## After

From 13.24 `VfxPrimitiveMeshBase` declares `0x6aec9e7a: bool`. No name table spells the field.
The game's meshes hold it only as `true`.

By LTK Manager's reading of the 16.17 client, the field decides whether a camera-aligned
particle's own orientation is discarded or composed under the camera-facing rotation, and a
mesh without it composes.

## Riot's conversion

Every `VfxPrimitiveMesh` aligned to the camera on either axis got `0x6aec9e7a: true`: aligned on
yaw alone, on pitch alone or on both, and whether or not its emitter holds an orientation of its
own. No `VfxPrimitiveAttachedMesh` got it, and no mesh that is not aligned.

That the flag keeps the look the aligned meshes had before 13.24 is inferred from Riot setting it
on every one of them. What the client drew for an aligned mesh up to 13.23 is not read.

## In a mod

A mod made before 13.24 holds no `0x6aec9e7a` on its aligned meshes. At a later build such a
mesh lacks what Riot's conversion gave the game's, and by LTK Manager's reading it composes its
own orientation under the camera-facing rotation.

A mesh aligned by the conversion of #GAME-CHANGE-0017 is in a mod made before 13.24 too.

## Evidence

- Counted in the census history's object texts (2026-10-07), branch `history-v2`: no line holds
  `0x6aec9e7a` at 13.23.5447494, and 3,047 lines hold `0x6aec9e7a: bool = true` at
  13.24.5461574, the next build of the history. No line holds it as `false`.
- The same primitives paired across the two builds by file, emitter name and place among the
  emitter's primitives (2026-10-07). Of the meshes aligned at both builds, 2,977
  `VfxPrimitiveMesh` hold the flag at 13.24.5461574 and none lacks it: 489 aligned on yaw alone,
  21 on pitch alone, 2,467 on both. 1,048 of their emitters hold a `birthRotation0`, 1,929 do
  not. None of the 310 `VfxPrimitiveAttachedMesh` aligned at both builds holds it. None of the
  4,869 meshes aligned at neither build holds it. 76 `VfxPrimitiveMesh` of four WADs lost their
  alignment at 13.24.5461574 and hold no flag. Of the meshes new at that build, 70 hold it, all
  aligned, and 3 aligned ones do not.
- `rito-meta` (generation 2026-10-05, read 2026-10-07): `VfxPrimitiveMeshBase.0x6aec9e7a` is a
  `Bool` with no default from 13.24 to 16.19, and `VfxPrimitiveMesh` and
  `VfxPrimitiveAttachedMesh` inherit it. The schema starts at 13.15, so "from 13.24" is read and
  not a bound of the schema.
- LTK Manager's VFX renderer plan (read 2026-10-07), a reading of the 16.17 client. Its section
  2.18: "It reaches the particle pool, whose per-particle orientation is a 4x4, identity at
  spawn. The flag picks whether that orientation is discarded or composed under the
  camera-facing rotation", and the renderer composes for a mesh without it. Its section 2.14
  counts 4,882 objects that hold the flag against 8,435 that hold `AlignYawToCamera` at 16.17.
  The same section 2.18 records that two sites of the client read the flag and do not agree,
  and that a third, reached from the simple path, does not read it.
- No check in game compared an aligned mesh with the flag and without it.
