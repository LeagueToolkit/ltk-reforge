# ADR-0033: The corpus covers champion skin mods

- **Status:** Accepted (2026-09-30)
- **Related:** #ADR-0004, #ADR-0025, #ADR-0034

## Context

The datasets are dumps of mod sites with light filtering, each archive named by its SHA-256. A
table classes each mod: champion skin, audio and skin, audio, map, UI, bin only, other, no skin.
A second table maps byte-identical archives to the one kept.

A port is defined for a champion's skin. Map, UI and all-champion mods have no skin unit to
target, and an all-champion pack takes hours to run.

## Decision

A corpus run covers the classes `champion-skin` and `audio+skin`, less the duplicates (the
user's calls, 2026-09-29 and 2026-09-30, during ltk-forge).

- `map`, `ui` and the classes with no character run only when a task needs them.
- Moved out of the datasets, with the class table saying where: special skins (sub-character
  only, no champion's, all-champion packs, a bin shared by several skins, add-ons to another
  mod) and broken skins (#ADR-0025).
- A run says how many mods it skipped.
- The class table is copied to a dated backup before it is edited.

## Consequences

- 5,137 mods are in scope at ltk-forge's eighth datasets run.
- A change is tried on the mods it affects first, and on the full datasets only for a new
  baseline.
