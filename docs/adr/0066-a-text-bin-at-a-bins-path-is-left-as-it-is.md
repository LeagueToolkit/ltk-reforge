# ADR-0066: A text bin at a bin's path is left as it is

- **Status:** Accepted (2026-10-06)
- **Related:** #MOD-ARTIFACT-0015, #MOD-ARTIFACT-0016, #GAME-CHANGE-0012

## Context

64 mods of the datasets hold a bin written as text or JSON at a bin's path, 221 files, every one
at the path of a bin the game shares between skins (#MOD-ARTIFACT-0015). A reader of binary bins
rejects such a file, and in 56 of the 64 a loaded bin links it.

The options: parse the text and write the binary bin, so that what the text holds loads; or leave
the file as it is. Which tool leaves the text there is not known, and whether the client read it
when the mod was made is not established.

## Decision

The user, 2026-10-06: **a text bin at a bin's path is left as it is.** No binary bin is made of
it.

- The file is reported (`inventory.bin-text`), and a loaded one as a bin that does not parse
  (`bin.unreadable`).
- A text bin at a shared bin's former path is not carried to the bin's path of the target build
  (#GAME-CHANGE-0012).

## Consequences

- What the text holds does not load in a repaired mod, as it is taken not to have loaded in the
  author's game.
- A check in game that the client reads such a text would reopen it.
