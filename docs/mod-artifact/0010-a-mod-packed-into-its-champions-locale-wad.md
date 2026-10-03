# MOD-ARTIFACT-0010: A mod packed into its champion's locale WAD

- **Source:** a mod manager's import of a WAD file for most; the others not established
- **Affects:** the archive: a WAD named `<champion>.<xx_XX>.wad.client`
- **Related:** #MOD-ARTIFACT-0009, #MOD-ARTIFACT-0011, #ADR-0028

## What is found

A WAD named for a champion and a locale, such as `Graves.en_US.wad.client`, that holds the
champion's skin bins (`data/characters/<champion>/skins/skin<N>.bin`), in an archive with no
champion WAD of that champion.

A name is a locale WAD's when the part after the stem's first dot is two letters, an underscore
and two letters.

It is told from a locale WAD that carries voice by the skin bins and by the missing champion
WAD. A mod's locale WAD beside its champion WAD is no artifact (`wad.locale`).

## Why it is there

Most of these mods say `from wad` in their description: a mod manager made the archive from one
WAD file (#MOD-ARTIFACT-0009), and that file was named for the locale. Why the author's WAD
carried the locale's name is not established.

## What reads it, and how

ltk-forge's docs state two things about it, and name no check in game for either:

- The game installs the locale WADs of one locale, so what a mod holds in a locale WAD shows
  only on an install in that locale.
- LTK's overlay builder puts a localized WAD's new content in the unlocalized WAD beside it.

How each mod manager places such a WAD is not established.

## The repair

The mod's files in the champion's WAD, `<Champion>.wad.client`. The locale WAD is taken as the
champion's, with a note (`wad.locale-packed`).

What the game localizes keeps its locale: a file at a path the game ships in a locale WAD, or
under the locale's folder (`vo/en_us/`), goes to that locale's voice layer (#ADR-0028).

## Evidence

- The baseline run (2026-10-03): 71 of the 5,137 mods. Each holds one WAD, named for `en_US` and
  packed whole. Each ports.
- 62 of the 71 say `from wad` in their description.
- 481 other mods of that run carry a locale WAD that is not taken as the champion's.
- ltk-forge's run of 2026-09-30 over the mods that had halted for holding no champion WAD found
  71, and its docs count 61 of them as imported from a WAD.
