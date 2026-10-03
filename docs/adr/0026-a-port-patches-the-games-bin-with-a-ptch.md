# ADR-0026: A port patches the game's bin with a PTCH

- **Status:** Accepted (2026-09-29)
- **Related:** #ADR-0011, #ADR-0027

## Context

A mod that ships a whole `skin<N>.bin` shadows the game's bin at every patch: whatever Riot
changes in it afterwards is lost until the mod is rebuilt. LTK Manager's overlay can apply
changes to the game's own bin at build time, through a `game_data` declaration. Two forms were
open for a port:

- a patch, in the PTCH format: records that replace fields of the game's entry by path, with the
  links to add in the declaration;
- declarations alone: path edits on the game's objects and constructions of new objects, with a
  PTCH only on a class change.

The client does not load a PTCH as a file's data, so a patch reaches the game only through
the declaration.

## Decision

A port is a mod project whose `game_data` declaration applies a PTCH to the game's skin bin and
links the port's own bins, packed to a `.modpkg`. The declarations form is deprioritised (the
user's call, 2026-09-29, during ltk-forge).

## Consequences

- A port carries only what the mod made. The game's later changes to the rest of the bin show.
- A PTCH cannot remove a field, insert a map entry or grow a list: a map or a list is replaced
  whole, and a field the mod lacks stays the game's.
- Every name on a record's path has to be spelled (#ADR-0031).
- Another output form shares everything up to the build of the package.
