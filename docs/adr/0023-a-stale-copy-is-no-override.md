# ADR-0023: A stale copy is no override

- **Status:** Accepted (2026-10-01)
- **Related:** #MOD-ARTIFACT-0004, #ADR-0019

## Context

A mod's WAD repacked at an older patch carries game files as they were then
(#MOD-ARTIFACT-0004). Such a file is no work of the author's. Carried as an override, it reverts
the game's file to the older one.

## Decision

A stale copy is no override: the game's newer bytes stand and the file does not ship (the user's
call, 2026-10-01, during ltk-forge). A stale copy is stale whatever names it.

Two exceptions:

- A restoration of a champion's model from before its rework keeps its copies as its own
  (#ADR-0019).
- `--keep-stale` carries every stale copy, for a mod that restores old textures on purpose.

## Consequences

- Without the history nothing is known to be stale, and every such file is an override.
- A restoration of a champion with no rework looks like a stale repack. When it matters, a
  person is asked.
