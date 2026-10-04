# ADR-0040: Of several files for one asset, the mod's own bytes win

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0038, #ADR-0039, #GAME-CHANGE-0009, #GAME-CHANGE-0010, #MOD-ARTIFACT-0004

## Context

A mod can hold several files that are one asset under the game's older names: a texture with
and without a variant tag, and the reduced copies of a texture beside it (#GAME-CHANGE-0009,
#GAME-CHANGE-0010). Where the game ships the asset at one path, one of those files is made the
file at that path.

Authors often edit one of the files and leave the others as the game shipped them. A texture
edited at one resolution and not at the others is common, and so are reports that such a mod's
texture does not show for a player at another quality setting than the author's.

A file left as the game shipped it is a stale copy (#MOD-ARTIFACT-0004). Made the asset's file,
it is left out of the package (#ADR-0039), and the asset then has no file of the mod's at all,
while the file the author did edit sits beside it unused.

## Decision

Among the files that could become one asset, a file holding the mod's own bytes is taken before
a file holding bytes the game shipped at its path.

Among files of the mod's own bytes, the order of #ADR-0038 holds: the larger reduced copy
first.

## Consequences

- The file the author edited is the one the asset is made from, whichever of the old names it
  sits under.
- A mod repaired again makes the same file: the choice does not depend on a file the first
  repair left out.
- This departs from ltk-forge (#ADR-0004), which takes the first candidate by the order of its
  rename rules. Over 245 mods run again (2026-10-04) it changes the load stage in 6. It was not
  compared over the whole datasets.
