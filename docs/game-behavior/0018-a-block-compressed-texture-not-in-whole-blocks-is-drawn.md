# GAME-BEHAVIOR-0018: A block-compressed texture not in whole blocks is drawn

- **Status:** Inferred
- **Builds:** builds before 16.19, for the game's own textures of this kind; which builds was
  not recorded
- **Affects:** `.dds`, `.tex`; the block-compressed formats (DXT1, DXT3, DXT5 and the other BC
  formats), whose blocks are 4 by 4 pixels
- **Related:** #GAME-BEHAVIOR-0003, #GAME-CHANGE-0007, #ADR-0022

## Behaviour

The client draws a block-compressed texture whose width or height is not a multiple of four.
A size that is not in whole blocks does not by itself stop a texture from loading.

LTK Manager's rule `tex/block-alignment` names a crash of the client (`ALE-D0D00020`) for a
block-compressed texture of such a size. What else is true of a texture that crashes is not
established: its format, its mip chain, or the `.dds` path it sits at.

## In a mod

- A texture of such a size that a mod ships loads as the game's own such textures do, as far
  as is known.
- TEX's container holds a texture in whole blocks only (#GAME-CHANGE-0007). A DDS of such a
  size stays the DDS it is at its `.tex` path (#GAME-BEHAVIOR-0003).
- Resampling every such texture to whole blocks would change textures the client draws.

## Evidence

- The 14 in-game sample mods ship 17 block-compressed textures not in whole blocks: DXT ramps
  of 256 by 1 and 128 by 1, and textures of 10 by 10, 125 by 125 and 256 by 261 (ltk-forge's
  `TRANSFORMS.md`, no date recorded). No check in game of one of these textures is recorded.
- Several of the 17 are bytes the game shipped at earlier builds, a DXT1 ramp of 256 by 1
  among them. The game shipped such a texture, so the size alone is not what crashes.
  ltk-forge's docs state that the game drew it and name no check of that.
- Over 1,451 dataset mods with no skin bin (2026-09-30), 25 textures a port ships at a `.tex`
  path stay DDS for a size not in whole blocks.
- The crash and its code are LTK Manager's statement, taken from ltk-forge's `TRANSFORMS.md`.
  No crash was reproduced.
- Hematite's source states it of a TEX in DXT1 or DXT5: the client "reads garbage past the end
  of the buffer" and draws noise, "or crashes outright on some patches". It crops every such
  TEX to whole blocks and drops its mip chain, and names the Python and Celestial fixers as
  doing the same (read 2026-10-05). It names no build and no check.
- Two checks would confirm it: a count, over the census history's texture facts, of the
  block-compressed textures of such a size the game ships at the target build; and one mod
  texture of 10 by 10 in DXT1, at a `.dds` path and at a `.tex` path, seen in game.
