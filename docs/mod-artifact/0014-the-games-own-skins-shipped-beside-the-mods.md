# MOD-ARTIFACT-0014: The game's own skins shipped beside the mod's

- **Source:** not established
- **Affects:** `data/characters/<character>/skins/skin<N>.bin` at slots the mod did not work on
- **Related:** #MOD-ARTIFACT-0001, #MOD-ARTIFACT-0004, #ADR-0020

## What is found

A unit that holds nothing of the mod's, in a mod that has work at another slot. All three hold:

- Every object the unit's entry reaches is the game's: an object of that slot's own skin, of a
  chroma of its family, or of no known skin, as the game holds it at the target build or held
  it at an older one.
- Every file it names is the game's.
- Another unit of the character is not such a unit, and names every own clip this one names.

It comes with a spread (#MOD-ARTIFACT-0001): the mod's work at some slots, and the game's own
bins at others.

It is told from a copy by what it holds. A copy is the mod's work again at another slot. This
unit is the game's skin of its own slot.

It is told from a mod's only unit by the third condition. A sound mod that ships the game's
skin bin beside its banks has one unit, and that unit is the mod.

## Why it is there

Not established. A WAD extracted and packed again carries every skin bin the game held at that
patch (#MOD-ARTIFACT-0004), and which of these mods were made so is not known.

## What reads it, and how

The client loads the mod's bin at the slot in place of the game's.

- A bin equal to the game's at the target build changes nothing.
- A bin from an older build puts the skin back as it was at that build. One from before 16.10
  links the shared bins at paths the game dropped (#GAME-CHANGE-0012).

## The repair

The unit left out. The game's own bin stands at the slot (`unit.left`, #ADR-0020).

## Evidence

- The baseline run (2026-10-03): 31 of the 5,137 mods, 681 units. A mod holds from 1 to 68 of
  them, 19 at the median.
- ltk-forge's port run over its first corpus (2026-09-30): 2 of 1,029 mods. Its full datasets
  run of the same day: 26 of 5,544.
