# GAME-CHANGE-0018: Emission surface in EmissionSurface

- **Build:** 15.18.7095052 for the avatar-pose flag, 15.22.7229166 for the surface's fields
- **Affects:** `VfxEmissionSurfaceData`, `IVfxEmissionSurface` and the classes derived from it:
  `VfxEmissionMeshData`, `VfxEmissionSkeletonData`
- **Related:** #GAME-BEHAVIOR-0002

## Before

An emitter holds the surface its particles are born on as `emissionSurfaceDefinition: pointer =
VfxEmissionSurfaceData`. Up to 15.17 that struct holds the surface itself:

| field of `VfxEmissionSurfaceData` | type |
|---|---|
| `meshName` | `string` |
| `skeletonName` | `string` |
| `AnimationName` | `string` |
| `Submeshes` | a list of `hash` |
| `meshScale` | `f32` |
| `maxJointWeights` | `u16` |
| `useSurfaceNormalForBirthPhysics` | `bool` |
| `useAvatarPose` | `bool` |

From 15.18 to 15.21 it holds `EmissionSource: u8` in place of `useAvatarPose`. From 15.19 to
15.21 the class also declares `JointMask`, a list of `hash`. No game surface holds a
`JointMask` at those builds.

## After

From 15.22 a `VfxEmissionSurfaceData` holds `EmissionSurface: pointer = IVfxEmissionSurface`,
and its class declares none of the fields above.

`IVfxEmissionSurface` is an interface. An `EmissionSurface` is one of the classes derived from
it:

- `VfxEmissionMeshData`, which declares `meshName`, `skeletonName`, `AnimationName`,
  `Submeshes`, `meshScale`, `maxJointWeights` and `useSurfaceNormalForBirthPhysics`.
- `VfxEmissionSkeletonData`, which declares `skeletonName` and `JointMask`.
- From 16.1, a class with no field, `0x526478f0`, whose name no table spells.

No class declares `EmissionSource` or `useAvatarPose`.

## Riot's conversion

- **15.18.7095052.** Each `useAvatarPose: true` became `EmissionSource: 1`. No other field of
  a surface changed.
- **15.22.7229166.** The seven fields of a surface moved into an `EmissionSurface` of class
  `VfxEmissionMeshData`, each under the same name, its value unchanged. `EmissionSource` was
  dropped, and nothing took its place.
- A surface that held `EmissionSource: 1` and no other field became a surface with no field and
  no `EmissionSurface`.
- No game surface became a `VfxEmissionSkeletonData` at that build. The first are at later
  builds.

What the client emits from for a surface that held `EmissionSource: 1`, after 15.22 dropped the
value, is not established.

## In a mod

A mod made before 15.22 holds the surface's fields on the `VfxEmissionSurfaceData`. A later
build drops them (#GAME-BEHAVIOR-0002): the surface names no mesh and no skeleton, and the
particles are not born on the mesh the author named.

A mod made before 15.18 holds `useAvatarPose`. A build from 15.18 drops it. From 15.22 no field
holds what it chose.

A mod made later holds the old fields too when its author pasted an emitter from an older mod.

## Evidence

- Counted in the census history's entry texts by the class's and the fields' hashes
  (2026-10-04), over the surfaces that hold one of the fields above or an `EmissionSurface`:

  | build | surfaces | `meshName` | `useAvatarPose` | `EmissionSource` | `EmissionSurface` |
  |---|---:|---:|---:|---:|---:|
  | 15.17.7085788 | 51 | 45 | 35 | 0 | 0 |
  | 15.18.7095052 | 51 | 45 | 0 | 35 | 0 |
  | 15.21.7218442 | 51 | 45 | 0 | 35 | 0 |
  | 15.22.7229166 | 45 | 0 | 0 | 0 | 45 |
  | 16.19.8207193 | 524 | 0 | 0 | 0 | 524 |

- The same surfaces paired across 15.17.7085788 and 15.18.7095052: all 51 are in both. Each of
  the 35 that held `useAvatarPose: true` holds `EmissionSource: 1`, the other 16 hold neither,
  and every other field is equal.
- Paired across 15.18.7102811 and 15.19.7131928: all 51 are equal, and none holds a
  `JointMask`.
- Paired across 15.21.7218442 and 15.22.7229166. 44 of the 45 surfaces that held a `meshName`
  hold an `EmissionSurface` of class `VfxEmissionMeshData` with the same fields and values, and
  no field of their own. 28 of them held `EmissionSource: 1`, which no field holds after. The
  one other, which held a mesh, a skeleton and an animation, holds no field at 15.22.7229166.
  Each of the 6 surfaces that held only `EmissionSource: 1` holds no field.
- At 16.19.8207193, of the 524 surfaces, 85 hold a `VfxEmissionMeshData`, 10 a
  `VfxEmissionSkeletonData` with a `JointMask`, and 429 the class `0x526478f0`.
- ltk-forge paired the game's surfaces across the same builds on the census and found the same
  conversion: `useAvatarPose` to `EmissionSource`, 35 of 35 (2026-10-01).
- The meta database (generation 2026-09-28, read 2026-10-04):
  `VfxEmissionSurfaceData.useAvatarPose`, a bool with default `false`, is removed in 15.18.
  `EmissionSource`, a `u8` with default 0, is declared from 15.18 to 15.21. `JointMask` is
  declared from 15.19 to 15.21. `meshName` and the six other fields are removed in 15.22.
  `EmissionSurface`, a pointer to `IVfxEmissionSurface` with default null, is declared from
  15.22. `IVfxEmissionSurface`, `VfxEmissionMeshData` and `VfxEmissionSkeletonData` exist from
  15.22 and `0x526478f0` from 16.1.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 226 of the 5,137
  mods hold a surface in an old layout. 405 surfaces in 159 mods hold a mesh's fields.
  `useAvatarPose` is held 554 times in 223 mods, and in 67 of them it is all the surfaces hold.
