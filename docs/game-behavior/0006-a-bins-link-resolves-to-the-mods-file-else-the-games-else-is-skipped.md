# GAME-BEHAVIOR-0006: A bin's link resolves to the mod's file, else the game's, else is skipped

- **Status:** Inferred
- **Builds:** 16.19
- **Affects:** a bin's list of linked bins; the names objects link by
- **Related:** #GAME-BEHAVIOR-0001, #GAME-BEHAVIOR-0007

## Behaviour

A bin lists the paths of the bins it links. The client loads each. A path resolves to one chunk
by its hash: a mod's file at the path takes the place of the game's, a path only the game holds
gives the game's bin, and a path nothing holds is skipped without a word.

Every bin loaded for a skin shares one set of object names. An object's link to another object
is a name, and it resolves in whatever loaded bin holds an object under that name. A champion
and its sub-characters load together and share the set.

The bin of shader definitions, `data/shaders/shaders.bin`, is loaded whatever plays, so a
material's link to a shader resolves with no bin listing it.

Not established: the order in which two loaded bins that hold one name are searched.

## In a mod

- A bin of a mod loads only when a loaded bin links its path, or it sits at a path the game's
  own bins link.
- A link to a game bin keeps working when the mod ships no file there.
- A name that no loaded bin holds resolves to nothing: the effect, the material or the graph it
  names is not there.
- A name the game held in the bins of one skin and not in the bins of another resolves only
  where a loaded bin links the bin holding it.

## Evidence

- ltk-forge's load of a mod follows these rules, and the 14 mods ported by hand and confirmed
  in game before ltk-forge carry only bytes its load marks as used.
- Over those 14 mods every link to a path neither the mod nor the game holds is a path the game
  shipped at earlier builds: a pasted bin still names the shared bins of the patch it was made
  at, and the mods work in game with those links skipped.
- A port that applies a patch and links its own bins resolves every name of the 14 sample mods
  under this rule, and the ports work in game (2026-09-29).
- What would confirm the open part: two loaded bins defining one name with different values,
  and the value seen in game.
