# MOD-ARTIFACT-0004: Stale copies of game files

- **Source:** tools that repack a whole WAD, and authors who start from an extracted WAD
- **Affects:** any file at a path the game ships
- **Related:** #GAME-CHANGE-0001

## What is found

A file at a path the game ships, holding the bytes the game shipped at that same path at an
earlier build, where the game holds other bytes at the path since.

It is told from the author's work by the history: the author's own bytes were never the game's.
It is told from a borrowed file by the path: bytes the game holds under another path are a file
the author chose (#MOD-ARTIFACT-0003).

A mod that restores a champion's model from before its rework (#GAME-CHANGE-0001) holds such
files on purpose: files at paths the rework dropped beside files at paths only the rework has.

## Why it is there

A WAD extracted at one patch and packed again carries every game file as it was then. The
author changed a few files and shipped them all.

## What reads it, and how

The client loads the mod's file in place of the game's. The asset reverts to its older form:
an effect Riot retouched looks as it did, a texture Riot fixed is unfixed.

## The repair

Leave the file out. The game's own bytes at the path stand.

## Evidence

- ltk-forge's sixth datasets run (2026-10-01): of the overridden particle textures no skin's
  bin claimed, 77% were such copies, by sample.
- Without a history nothing is known to be stale: against the install alone, an older game file
  looks like the author's own.
- Repath tools copy game files under a prefix of their own (#MOD-ARTIFACT-0029). LtMAO's bumpath
  also writes a game bin it reaches, a skin's linked animation bin for one, at the game's own path
  with its strings rewritten (source read 2026-10-06): the game's objects of the tool's build, in
  bytes the game never shipped, which the history does not show as a copy.
