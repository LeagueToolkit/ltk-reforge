# GAME-CHANGE-0013: File references as chunk hashes

- **Build:** 16.17.8104348 for 393 properties; 16.18.8159717 for eight more
- **Affects:** every bin property that names a file: `SkinMeshDataProperties.texture`,
  `StaticMaterialShaderSamplerDef.texturePath`, `VfxAssetRemap.oldAsset`,
  `AnimationResourceData.mAnimationFilePath`, the icons of `SkinCharacterDataProperties`,
  `AtlasDataBase.mTextureName` and the others of 152 classes
- **Related:** #GAME-BEHAVIOR-0002, #GAME-CHANGE-0007, #GAME-CHANGE-0014, #MOD-ARTIFACT-0022

## Before

A bin property names a file by its path, as a `string`: `texture: string =
"ASSETS/Characters/Aatrox/Skins/Skin26/Aatrox_Skin26_TX_CM.tex"`.

Seven properties name a file by a 32-bit `hash`, the FNV-1a hash of the lowercased path.
`VfxAssetRemap.oldAsset` is one.

The type `file` exists before the change: 3,316 values at 16.16.8049184, a view controller's
`PathHashToSelf` among them.

## After

The property is a `file`: the 64-bit xxHash of the lowercased path, which is the hash a WAD
holds the file's chunk under. The bin holds no path.

The properties whose type changed, by the meta database:

| before | after | at 16.17 | at 16.18 |
|---|---|---:|---:|
| `string` | `file` | 350 | 6 |
| `hash` | `file` | 7 | |
| a list of `string` | a list of `file` | 10 | 1 |
| an option of `string` | an option of `file` | 5 | |
| a map with `string` values | a map with `file` values | 20 | 1 |
| a map with `hash` keys | a map with `file` keys | 1 | |

The map keyed by files is `UiElementParticleSystemData.TextureOverrides`: its keys change at
16.17 and its values at 16.18.

Not every property that names a file changed. `SkinMeshDataProperties.simpleSkin` and
`skeleton`, `VfxEmitterDefinitionData.texture` and `VfxAssetRemap.newAsset` are strings at
16.19.

## Riot's conversion

- **A string** became the xxHash of its lowercased path. Every one of 23,300 paired
  `SkinMeshDataProperties.texture` values is so.
- **A 32-bit hash** became the xxHash of the path the old hash was of. The old hash does not
  give the new one: the conversion goes through the path.
- **A list, an option or a map** kept its shape, each string in it converted the same way.
- **An empty string.** None of the game's values of these fields is empty at 16.16.8049184, so
  no game value shows what an empty string became. The schema's default for a `file` is the
  zero hash.

## In a mod

A mod made before 16.17 holds strings, and 32-bit hashes in the seven properties. A later build
reads a `file` there and drops the value (#GAME-BEHAVIOR-0002): the skin's texture is not drawn,
a material's sampler binds nothing, an icon is missing, a remap does not apply.

A mod installed by LTK Manager had these values converted as it was installed
(#MOD-ARTIFACT-0022).

A 32-bit hash converts only when its path is known. A mod's own file can be named by a hash no
name table spells.

## Evidence

- The meta database (generation 2026-09-28, read 2026-10-04): between 16.16 and 16.17 the type
  of 395 properties changed, 393 of them to a type holding a `file`, in 152 classes. Between
  16.17 and 16.18 eight more: `AtlasDataBase.mTextureName`, `TextureResource.texturePath`,
  `EvolutionDescription.mIconNames`, one path each of `MapBakeProperties`, `MapClouds`,
  `MapCubemapProbe` and `MapTerrainPaint`, and the values of
  `UiElementParticleSystemData.TextureOverrides`. None between 16.18 and 16.19.
- The same database: `SkinMeshDataProperties.texture` is a string from 13.15 to 16.16 and a
  `file` from 16.17. `VfxAssetRemap.oldAsset` is a `hash` from 13.15 to 16.16 and a `file` from
  16.17. `SkinMeshDataProperties.simpleSkin` and `skeleton`, `VfxEmitterDefinitionData.texture`
  and `VfxAssetRemap.newAsset` are strings from 13.15 to 16.19.
- Counted in the census history's entry texts (2026-10-04). Lines of a `file` type, in a leaf
  or a container: 3,316 at 16.16.8049184, 473,746 at 16.17.8104348, 484,907 at 16.18.8159717
  and 490,083 at 16.19.8207193.
- `SkinMeshDataProperties.texture` (the same count): 25,048 strings and no `file` at
  16.16.8049184, 23,374 `file` values and no string at 16.17.8104348. 23,300 skin meshes are in
  both builds, and in each the new value is the xxHash of the old path, lowercased.
- `VfxAssetRemap.oldAsset` (the same count): 10,963 `hash` values at 16.16.8049184, 8,522
  `file` values at 16.17.8104348, 8,472 remaps in both. For 5,042 of them the old hash is the
  FNV-1a hash of a path string the game's bins hold at 16.16.8049184, and in each the new value
  is the xxHash of that path. No old hash maps to two new values. The other 3,430 old hashes
  are of paths no bin of that build spells, and are not checked.
- By the hashes of the 291 field names the schema retypes from string to `file` at 16.17,
  whatever the class: one `file` value and no empty string at 16.16.8049184, 422,909 `file`
  values at 16.17.8104348.
- By the hash of `mTextureName`, whatever the class: 8,876 strings and 413 `file` values at
  16.17.8104348, 108 and 9,155 at 16.18.8159717.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 3,918 of the 5,137
  mods hold a string where 16.19 reads a `file`, 276,351 values. 340 mods hold a `hash` there,
  1,704 values. One value in one mod is a hash of a path nothing spells.
