# MOD-ARTIFACT-0030: A skin bin split into concat and StaticMat bins

- **Source:** TopazModFixer, the fixer of the cslol-go manager
- **Affects:** `data/characters/<character>/skins/skin<N>.bin`, and the bins
  `data/<character>_skin<N>_concat.bin` and `data/<character>_skin<N>_StaticMat.bin`
- **Related:** #MOD-ARTIFACT-0001, #MOD-ARTIFACT-0029, #GAME-CHANGE-0012

## What is found

A skin bin holding two objects, the skin entry and its resolver, whose links are, in this order:
the character's root bin `data/characters/<c>/<c>.bin` when the skin linked it, the linked
paths the tool found nowhere, `data/<c>_skin<N>_concat.bin`, and
`data/<c>_skin<N>_StaticMat.bin` when the skin has materials.

- **The concat bin** links nothing. It holds, in this order, the gear upgrades, the effects, the
  contextual actions, the animation graph and every object of another class, gathered from the
  skin bin and every bin it links, the game's linked bins included.
- **The StaticMat bin** links nothing and holds the skin's `StaticMaterialDef` objects.
- **Gameplay objects are absent.** `SpellObject`, `AbilityObject`, `CharacterRecord`, item and
  jungle-path recommendations and stat stones are not written.
- **Kept objects are filtered.**
  - An effect that is no value of the resolver's map, or of a gear upgrade's, is not written.
  - Of several skin entries, resolvers, contextual actions or graphs, only the one the skin
    names is kept.
- **A copy at every slot.** The tool's default copies skin0's bin to every skin the game held,
  with the skin entry's name, the resolver's name and the link to it set to the slot. Each copy
  links skin0's concat and StaticMat bins.
- **Sub-characters.** Their skin bins are written the same way, from the game's bins where the
  mod has none.

The names are directly under `data/`, which no game bin has been at since 16.10
(#GAME-CHANGE-0012). The character is spelled as configured (`data/drmundo_skin3_concat.bin`).

## Why it is there

The tool merges every bin a skin loads into bins of its own so that the mod no longer depends
on the names of the game's linked bins, which Riot renames. It keeps the objects it judges the
skin's and drops the rest.

## What reads it, and how

The client loads the skin bin, follows its links and loads the concat and StaticMat bins: the
mod's names are paths the mod ships. The objects merged from the game's linked bins load from
the mod's copy, as they were at the build the tool ran at, and later changes to them do not
reach the mod.

An effect that no resolver map names, which the tool dropped, is not there. One reached only by
a link from another object is lost so.

## The repair

None is needed for the bins to load. The objects that are the game's, by their name and content
in the game's bins at some build, are copies like the files of #MOD-ARTIFACT-0029. The copies at
every slot are #MOD-ARTIFACT-0001.

## Evidence

- The baseline repair run (2026-10-04), over the 5,137 mods of the datasets: 54 mods ship 63
  concat bins and 20 StaticMat bins, each loaded. 53 of the 54 hold files under the tool's
  repath prefix (#MOD-ARTIFACT-0029).
- In 41 of the 54 one character has 8 to 101 skin bins, the copies at every slot. In 13 it has
  one.
- The layout, the order of the objects and the filters are read from the source of
  TopazModFixerCLI (2026-10-05). Nothing was run.
