# MOD-ARTIFACT-0028: A unit of the game's name on banks of its own

- **Source:** authors
- **Affects:** the skin entry's `skinAudioProperties.bankUnits`; `.bnk`
- **Related:** #MOD-ARTIFACT-0005, #GAME-BEHAVIOR-0016

## What is found

A bank unit of the mod's skin entry with the name of a unit the game has at that skin
(`<Champion>_Base_SFX`), whose bank paths are all the mod's own: none is a path the game's
unit of that name loads. A repathed bank counts at the game's path, so a unit whose banks a
repath tool moved is not one of these.

## Why it is there

The author replaced the champion's sounds whole, with banks built for the mod under names of
its own, and kept the unit's name.

## What reads it, and how

The client loads the banks the unit names. The game's banks of that unit are not loaded, so
the game's sounds do not play: the mod's stand in for them.

## The repair

The mod's unit in place of the game's unit of that name, where a port otherwise adds its units
after the game's. With the game's unit left in, the game's sounds play beside the mod's
(#GAME-BEHAVIOR-0016). Only the mod's own clips are carried, as in every unit.

## Evidence

- ltk-forge, 2026-10-01, over the mods of its eighth datasets run: 39 mods hold such a unit.
  In 24 the unit is carried and the game's unit of that name is left out. No check of the
  package fails for it.
- ltk-forge's ports of the 5,137 mods of the datasets (2026-10-04): 40 units in 27 mods stand
  in for a game unit.
- One sample's unit loads the game's base events bank and, at a path of its own, a copy of the
  base media bank that is the game's clip for clip. It is not one of these.
