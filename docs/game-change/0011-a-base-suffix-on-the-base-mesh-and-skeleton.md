# GAME-CHANGE-0011: A `_base` suffix on the base mesh and skeleton

- **Build:** one build per character, 24 characters from 9.12 to 16.6 (below): the build of the
  character's rework, or of a new export of its model
- **Affects:** `assets/characters/<c>/skins/base/<c>.skn` and `<c>.skl`,
  `SkinMeshDataProperties.simpleSkin` and `skeleton`
- **Related:** #GAME-CHANGE-0001, #GAME-CHANGE-0009

## Before

A character's base mesh and skeleton are named for the character alone:
`assets/characters/<c>/skins/base/<c>.skn` and `<c>.skl`. The base skin's bin names the two
paths in `simpleSkin` and `skeleton`.

## After

The two files are `<c>_base.skn` and `<c>_base.skl` in the same folder, and the bin names
those.

Riot does not rename a file that stays as it is. A character gets the new names at the build
its model is replaced or exported again:

| character | build | the mesh and skeleton at the new names |
|---|---|---|
| `mordekaiser` | 9.12 | another model |
| `pantheon` | 9.16 | another model |
| `fiddlesticks` | 10.7 | another model |
| `volibear` | 10.11 | another model |
| `drmundo` | 11.12 | another model |
| `caitlyn`, `caitlyntrap` | 11.23 | another model |
| `udyr` | 12.16 | another model |
| `ahri` | 13.3 | another model |
| `jax` | 13.20 | another model |
| `sru_riftherald` | 14.1 | the same joints, another mesh |
| `leesin` | 14.9 | another model |
| `teemo` | 14.20 | another model |
| `viktor` | 14.24 | another model |
| `leblanc` | 15.7 | another model |
| `vi` | 15.14 | the same model, exported again |
| `turret` | 15.17 | another mesh, the old joints among more |
| `zac` | 15.22 | the same model, exported again |
| `sru_baron`, `zacrebirthbloblet` | 15.23 | the same model, exported again |
| `tahmkench` | 15.24 | the same model, exported again |
| `quinn` | 16.4 | the same model, exported again |
| `shyvana`, `shyvanadragon` | 16.6 | another model |

Most characters are not renamed. At 16.19.8207193, 807 characters hold a `<c>.skn` in their
base folder and 184 hold a `<c>_base.skn`. A character new to the game ships under either name.

A rework may take another stem altogether: Skarner's model is `skarner_rework_base.skn` from
14.7 and Xin Zhao's is `xinzhaorework_base.skn` from 15.17.

## Riot's conversion

- **The path.** `_base` is added to the stem. For 20 of the 24 characters the new name first
  ships with a variant tag, such as `leblanc_base.leblanc_rework.skn`, which is dropped at a
  later build (#GAME-CHANGE-0009).
- **The file.** Another file in every case. No mesh at a new name holds the bytes of the mesh
  at the old name.
- **The same model, exported again** (6 characters). The skeleton keeps its joint names, all of
  them or all but three, and gains at most one joint. The mesh keeps its index count, or is
  within 30 of it. Its vertex count changes for four of the six, and its submeshes are renamed
  for five: Vi's one submesh `Enforcer_Proxy_Enforcer_Material` is `Body` and `Teacup` from
  15.14.
- **Another model** (16 characters). At most 41 joint names are shared and the mesh is of
  another size. For the 11 champions among them that #GAME-CHANGE-0001 lists, the build is the
  rework's.
- **Another mesh on the old joints** (2 characters). The Rift Herald keeps its 76 joints and
  gets a mesh of another size. The turret keeps its 16 joints among 58.
- **The names in bins.** The base skin's bin holds the old paths in `simpleSkin` and `skeleton`
  at the last build of the old names and the new paths at the next.

## In a mod

A mod made before its character's build ships `skins/base/<c>.skn` and `<c>.skl`, or names
them in its skin bin.

- **A file at the old name.** No game object names it from the build on. A mod with no skin bin
  that ships only such files replaces nothing: the game reads the mesh from `<c>_base.skn`.
- **A mod's bin that names the game's `<c>.skn`, which the mod does not ship.** The path is one
  the game does not hold.
- **A mod's bin that names a `<c>.skn` the mod ships.** The client reads the mod's files as
  before.
- **Where the build is a rework**, the mod's mesh is made for a model the game dropped, and the
  file at the new name is no successor to it (#GAME-CHANGE-0001).
- **Where the model was exported again**, the mod's mesh fits the new skeleton's joints by their
  names, and holds the submesh names of the old export.

## Evidence

- The history index of 755 builds, 8.20.2483196 to 16.19.8207193 (read 2026-10-03), with path
  names from the name tables. A rename is counted where `<c>.skn` ends at one build and
  `<c>_base.skn`, with or without a tag, begins at the next. The `.skl` paths move at the same
  builds.
- One more character, `slime_pushcart`, holds `<c>.skn` up to 9.2 and `<c>_base.skn` from
  10.14, with builds between that hold neither.
- The models: the index's mesh and skeleton records at the old name's last build and the new
  name's first, for each of the 24. The content keys of the two meshes differ in 24 of 24.
- The names in bins: the text of the base skin's entry in Vi's and Tahm Kench's `skin0.bin`,
  read from the census history at the build before the rename and at its build (2026-10-03).
  Each is named by one game object, the old path up to the build before and the new path from
  the build.
- The meta database (generation 2026-09-28, read 2026-10-03): `SkinMeshDataProperties.simpleSkin`
  and `SkinMeshDataProperties.skeleton` are strings from 13.15 to 16.19.
- The baseline run (2026-10-03): 46 of the 5,137 mods ship a mesh or a skeleton under the old
  name of one of the 24 characters, 100 files: Lee Sin's in 12 mods, Tahm Kench's in 11, Vi's,
  Zac's, Quinn's and Shyvana's in 5 each. ltk-forge copied 74 of the files, in 34 mods, to the
  `_base` name.
- ltk-forge's rule adds `_base` to any file directly in a `skins/base/` folder whose stem has
  no dot and no `_base`, where the game holds the result. The history shows the rename for a
  mesh and a skeleton only.
