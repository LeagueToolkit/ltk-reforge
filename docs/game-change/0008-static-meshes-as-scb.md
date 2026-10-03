# GAME-CHANGE-0008: Static meshes as `.scb`

- **Build:** 16.3.7422025
- **Affects:** `.sco`, `.scb`, `VfxMeshDefinitionData.mSimpleMeshName`
- **Related:** #GAME-CHANGE-0009

## Before

The game holds a static mesh in one of two formats: the binary `.scb` or the text `.sco`. A
bin value names the mesh by its path, extension included.

`.scb` is the common one at every build of the history. At 8.20.2483196 the game holds 6,681
`.scb` paths and 1,981 `.sco` paths. At 16.2.7413171 it holds 58,089 `.scb` paths and 3,759
`.sco` paths, in the folders of 435 characters and under `assets/maps/`, `assets/shared/` and
`data/particles/`.

## After

From 16.3.7422025 the game holds no `.sco` path. A mesh that was `.sco` is at the same stem
with the extension `.scb`.

## Riot's conversion

- **The path.** Every one of the 3,759 `.sco` paths ends at 16.2.7413171. For 3,735 of them a
  `.scb` path of the same stem begins at 16.3.7422025. The other 24 have no `.scb` beginning at
  that build.
- **The names in bins.** The same field of the same game object holds the `.sco` path at
  16.2.7413171 and the `.scb` path at 16.3.7422025: `mSimpleMeshName`, a string.
- **The mesh.** Not established. The history holds no file bytes and records nothing of a
  static mesh, so whether the `.scb` holds the vertices and faces of the `.sco` is not shown.

## In a mod

A mod made before 16.3 ships `.sco` files at the game's `.sco` paths, and its bins name `.sco`
paths.

- **A file at a path the game dropped.** No game object names it from 16.3. A mod with no skin
  bin that ships only such files replaces nothing: the game reads the mesh from the `.scb` path.
- **A mod's bin that names a `.sco` of the game's, which the mod does not ship.** The path is one
  the game does not hold.
- **A mod's bin that names a `.sco` the mod ships.** Whether a client from 16.3 reads a `.sco`
  file is not established.

A `.sco` file's bytes are not a `.scb` file's: the mesh has to be read as text and written in
the binary format.

## Evidence

- The history index of 755 builds, 8.20.2483196 to 16.19.8207193 (read 2026-10-03), with path
  names from the name tables. It holds 7,314 `.sco` paths and 146,834 `.scb` paths over all
  builds. By its magic the first file at each is of the format its extension says.
- The last `.sco` path begins at 16.2.7385403. Of the `.sco` paths that end before 16.3, 1,955
  lose a variant tag and stay `.sco` (#GAME-CHANGE-0009), and 1,557 end with no `.scb` of their
  stem at any build.
- The names in bins: of 500 sampled moves, 484 `.sco` paths were named by a game object up to
  16.2.7413171 and their `.scb` paths from 16.3.7422025. No object names one of the `.sco` paths
  at a later build. The other 16 are named by no game object.
- The field: the texts of 60 game objects that named a moved `.sco`, read at 16.2.7413171 and
  at 16.3.7422025 from the census history (2026-10-03). The field `0xd467e8c0` holds 102 `.sco`
  paths and 1 `.scb` path before, and 103 `.scb` paths after.
- The meta database (generation 2026-09-28, read 2026-10-03): `0xd467e8c0` is
  `VfxMeshDefinitionData.mSimpleMeshName`, a string from 13.15 to 16.19.
- The baseline run (2026-10-03): 1,000 of the 5,137 mods ship a file under a `.sco` name, 4,947
  files. ltk-forge converted 1,956 of them, in 346 mods, to a `.scb` at the same stem: files at
  a path the game dropped, where the game holds the `.scb` and the mod ships no file there. None
  failed to convert.
