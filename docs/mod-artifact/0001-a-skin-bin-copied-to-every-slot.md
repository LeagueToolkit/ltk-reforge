# MOD-ARTIFACT-0001: A skin bin copied to every slot

- **Source:** authors, and tools that do it for them: TopazModFixer by default
  (#MOD-ARTIFACT-0030), and Celestial's SkinLite by Hematite's source
- **Affects:** `data/characters/<character>/skins/skin<N>.bin` and the bins each links
- **Related:** #MOD-ARTIFACT-0002

## What is found

One character with many skin bins that are one piece of work. Two bins of a character are copies
when any of these holds:

| relation | test |
|---|---|
| byte-identical | the same bytes |
| renamed | equal once each bin's own slot names (its skin entry, resolver and graph) are normalised, the bins they link included |
| near | 80% or more of the normalised objects shared, and the same mesh, skeleton and texture |
| same rig | the same mesh, skeleton and texture, by the bytes of the files where shipped, whatever else differs |

Copies form clusters. A cluster that covers a skin other than its representative's own is a
spread. A mod that writes a different bin for each slot, each slot's own skin edited, is not
one: its bins are distinct work.

A spread may also hold the game's own bins for other slots, unchanged or from an older build.

## Why it is there

So that the mod shows whatever skin the player has picked. The client loads `skin<N>.bin` for
the skin in use, and a mod made for the base shows only on the base unless the same bin is
at every slot.

## What reads it, and how

The client loads the copy at the slot of the skin in use. Every skin of the champion then looks
like the mod, and the premium skins a player owns are replaced by it.

## The repair

The one unit the author made, at one slot. Showing a base-skin mod on every skin is a mod
manager's setting, not a property of the mod.

## Evidence

- ltk-forge's first corpus run (2026-09-29): 54,893 copies over 1,029 mods, 692 clusters that
  are spreads.
- Comparing rigs by the bytes of the shipped files and not by their paths took the characters
  with several distinct units from 715 to 60 in that run.
- 16 of those mods write a different bin for each of 99 or 100 slots.
- The tools' copies are renamed copies. TopazModFixer copies skin0's bin to every skin the game
  held at its run, with the skin entry's name, the resolver's name and the link to it set to the
  slot (source of TopazModFixerCLI, read 2026-10-05). Celestial's SkinLite, by Hematite's
  source, copies it to slots 1 to 99 with the same three names set and the two renamed objects
  moved to the end of the bin.
- The baseline repair run (2026-10-04): in 41 of the 54 mods laid out by TopazModFixer
  (#MOD-ARTIFACT-0030), one character has 8 to 101 skin bins.
