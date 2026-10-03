# ADR-0022: Textures keep their pixels

- **Status:** Accepted (2026-09-30)
- **Related:** #GAME-BEHAVIOR-0003, #ADR-0032

## Context

A port renames textures and ships a DDS at a `.tex` path, as the game names its textures. Some
repairs would need pixels: a block-compressed texture resampled to whole blocks, a missing mip
chain generated. Decoding a texture and encoding it again is lossy and slow. ltk-forge once
re-encoded DXT3 textures and partial mip chains to BC3, and the user asked why.

A DDS at a `.tex` path loads (#GAME-BEHAVIOR-0003).

## Decision

Nothing decodes a texture and encodes it again (the user's call, 2026-09-30, during ltk-forge).

- A texture is renamed with its bytes as they are.
- A DDS moves into TEX's container only where its blocks go in as they are. Anything else ships
  as the DDS it is.

## Consequences

- A repair that would resample is not made. Block-compressed textures not in whole blocks stay
  as they are, and the game draws its own such textures.
- Mip chains are not rebuilt. Merging a mod's resolution variants into one chain, or generating
  levels, waits until everything else works, or is a fixer's tool.
