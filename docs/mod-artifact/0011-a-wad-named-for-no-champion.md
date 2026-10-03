# MOD-ARTIFACT-0011: A WAD named for no champion

- **Source:** not established; the names read as authors' names for a folder
- **Affects:** the archive: the name of a WAD that holds a champion's skin bins
- **Related:** #MOD-ARTIFACT-0010, #MOD-ARTIFACT-0012

## What is found

A WAD whose stem, the name up to its first dot, is no champion's, and which holds skin bins of
a champion: `data/characters/<champion>/skins/skin<N>.bin`. The bins' own paths name the
champion.

The names seen are of three sorts:

- the mod's name: `Demon Blitzcrank.wad.client`, `winterblessed.wad.client`;
- another spelling of the champion's: `Jarvan.wad.client` for `jarvaniv`;
- a working folder's: `ryzenew.wad.client`, `repathed_Riven.wad.client`.

A champion here is a character the game keeps in a WAD of its own name. A sub-character is not
one: a WAD that holds only the skin bins of a champion's pet names no champion by them.

It is told from a WAD of another part of the game (`UI.wad.client`, `Map11.wad.client`,
`Common.wad.client`), which holds no champion's skin bin and is not carried (`wad.not-carried`).

## Why it is there

Not established. The names read as an author's own name for a folder. Which tools keep such a
name, and how the mod managers these mods were made for place a WAD of an unknown name, is not
established.

## What reads it, and how

A reader that takes a WAD's champion from its name finds no champion and no skin mod in the
archive (`archive.no-skin`).

Which of the game's WADs a mod manager lays such a WAD over is not established.

## The repair

The WAD under its champion's name, `<Champion>.wad.client`.

- A WAD that holds the skin bins of exactly one champion is taken as that champion's, when no
  other WAD of the archive is (`wad.champion-guessed`).
- A WAD that holds the skin bins of several champions is taken as none's
  (`wad.champion-ambiguous`).

## Evidence

- The baseline run (2026-10-03): 19 of the 5,137 mods, one such WAD in each. Each ports. No mod
  of the run holds a WAD with several champions' skin bins.
- 13 of the 19 are mods for two champions: 11 for Lee Sin and 2 for Jarvan IV. 13 of the WADs
  are folders and 6 are packed whole (#MOD-ARTIFACT-0009).
- ltk-forge's run of 2026-09-30 found 27 such mods among the mods that had halted. Its docs
  state that 19 of them port, and that the other 8 hold a sub-character's skin bins alone or
  those of a character that is no champion's.
