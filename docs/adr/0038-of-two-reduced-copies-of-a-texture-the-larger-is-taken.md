# ADR-0038: Of two reduced copies of a texture the larger is taken

- **Status:** Accepted (2026-10-04)
- **Related:** #GAME-CHANGE-0010, #GAME-CHANGE-0007, #ADR-0004

## Context

Up to the build its folder's textures moved to `.tex`, the game held two reduced copies beside a
texture: `2x_<name>.dds` at half the width and height and `4x_<name>.dds` at a quarter
(#GAME-CHANGE-0010). A later build holds the texture once, at `<name>.tex`, and no prefixed
path.

A mod made before that build ships files at the prefixed paths. Where it ships no file for the
unprefixed name, the file at a prefixed path is the only texture the mod has for the asset, and
it is made the file at the path the game reads.

A mod that ships both copies gives two candidates for one path. ltk-forge takes the `4x_` file
first. It names the prefixes as resolutions and takes the higher number, which is the smaller
file.

## Decision

The `2x_` file is taken before the `4x_` file: of two copies of one texture, the one with more
pixels.

The mod's own file at the unprefixed path still comes before both.

## Consequences

- A mod that ships both copies and no full texture shows the half-size texture where a port by
  ltk-forge shows the quarter-size one.
- This departs from ltk-forge, the oracle (#ADR-0004). The comparison lists it as an accepted
  deviation, and it is made after the comparison of the load stage is clean without it.
- A mod that ships one copy is not affected.
- Over the datasets (2026-10-04) the choice changes 100 of 5,137 mods: 941 textures are made
  from another file, 914 of them with other bytes.
