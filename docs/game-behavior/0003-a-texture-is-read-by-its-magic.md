# GAME-BEHAVIOR-0003: A texture is read by its magic

- **Status:** Inferred
- **Builds:** 16.19
- **Affects:** `.tex`, `.dds`
- **Related:** none

## Behaviour

The client decides how to read a texture from the first bytes of the file, not from the
extension of its path. A file in the DDS container at a path ending `.tex` is read as DDS. A
file in TEX's container at a path ending `.dds` is read as TEX.

Not established: whether every DDS pixel format the client reads at a `.dds` path is read at a
`.tex` path too.

## In a mod

- An older mod's `.dds`, copied as it is to the `.tex` path the game reads the asset from,
  loads.
- A port that names every texture `.tex`, as the game does, may ship a DDS under that name where
  TEX's container cannot hold the format.

## Evidence

- ltk-forge's ports ship DDS files at `.tex` paths wherever a texture's blocks do not go into
  TEX's container as they are (DXT3, a partial mip chain, sizes not in whole blocks). Over 1,451
  dataset mods on 2026-09-30, 1,218 textures shipped so.
- ltk-forge's docs state the rule and name no dated check in game. A check of one texture under
  both containers at one path would confirm it.
