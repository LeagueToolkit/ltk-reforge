# MOD-ARTIFACT-0012: Two WADs for one champion

- **Source:** not established
- **Affects:** the archive: `WAD/<Champion>.wad` beside `WAD/<Champion>.wad.client`
- **Related:** #MOD-ARTIFACT-0009, #MOD-ARTIFACT-0011

## What is found

Two WADs of one stem in one archive, neither a locale WAD: `Shaco.wad` beside
`Shaco.wad.client`. The game names a champion's WAD `<Champion>.wad.client`.

In three of the four mods seen, every path of the `.wad` is in the `.wad.client`. In the other,
the `.wad` holds 37 files and 31 of them are at paths the `.wad.client` lacks.

The `.wad.client` is packed whole in all four (#MOD-ARTIFACT-0009). The `.wad` is a folder in
three and packed whole in one.

## Why it is there

Not established. ltk-forge's code calls `.wad` an older spelling of the name.

## What reads it, and how

A reader that makes one character for each WAD gets the champion twice. ltk-forge did so in its
first corpus run (2026-09-29): one mod gave two characters of one champion, both ported to one
slot, with two patches on one bin.

How each mod manager merges two WADs of one stem is not established.

## The repair

One WAD for the champion: the one the game spells, `.wad.client`, and where neither is, the
first in archive order.

- The other WAD is not carried (`wad.duplicate`).
- When it holds files at paths the carried one lacks, those files are lost and the finding is a
  warning (`wad.duplicate-differs`).

## Evidence

- The baseline run (2026-10-03): 4 of the 5,137 mods, for four champions. In 3 the second WAD
  adds no path. In 1 it holds 31 files the carried WAD lacks.
- ltk-forge's first corpus run (2026-09-29): 1 of 1,029 mods.
