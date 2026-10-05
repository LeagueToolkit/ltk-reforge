# MOD-ARTIFACT-0032: A champion's root bin shipped whole

- **Source:** authors, who copy the champion's root bin to change a few of its values
- **Affects:** `data/characters/<character>/<character>.bin`: `CharacterRecord`, `SpellObject`,
  `AbilityObject`, item recommendations and the other gameplay objects it holds
- **Related:** #MOD-ARTIFACT-0004, #GAME-BEHAVIOR-0004, #ADR-0029

## What is found

A mod that ships its champion's root bin, the bin the client loads for the champion whatever
skin it wears. The bin holds the champion's gameplay data as the game held it at the build the
author copied it at: the character record, every spell and ability, the item recommendations.

It is told from a stale copy (#MOD-ARTIFACT-0004) by its bytes: the bin is not one the game
ever shipped, so the author changed something in it.

## Why it is there

The root bin names what a skin bin cannot change for the champion: its spell icons
(#GAME-BEHAVIOR-0004) and the effects and textures its spells name. An author who changes one
of those edits the whole bin and ships it.

## What reads it, and how

The client loads the mod's root bin in place of the game's. The champion's gameplay data is
then the mod's build's: values Riot changed since, spells it reworked and objects of classes it
dropped come back with the bin.

What the client does with it at a later build is not checked in game. Two other fixers never
ship it: TopazModFixer links the game's root bin and drops the gameplay classes it finds,
and Hematite removes any bin that holds one, stating that such bins "break mods after patches"
and that the root bin "goes stale every patch".

## The repair

The author's change, applied to the game's root bin of the target build. For spell icons and
HUD textures a port does it so (#ADR-0029).

## Evidence

- The baseline repair run (2026-10-04), over the 5,137 mods of the datasets: 259 mods ship 270
  root bins of their characters. Each is loaded, none is a stale copy, and each is in the
  repair's package.
- 227 of the mods hold `ItemRecommendationContextList` in a root bin, a class that left the
  game at 16.6 (`docs/design/RUNS.md`, "The repair (2026-10-04)").
- The other fixers' handling is read from the source of TopazModFixerCLI and of Hematite
  (2026-10-05). Neither names a build or a check.
- Which of a root bin's values the authors changed is not counted.
