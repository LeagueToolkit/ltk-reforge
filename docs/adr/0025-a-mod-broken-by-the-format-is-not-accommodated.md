# ADR-0025: A mod broken by the format is not accommodated

- **Status:** Accepted (2026-10-01)
- **Related:** #ADR-0033

## Context

Some archives are not mods a manager would load: a mod zipped whole inside its own WAD folder,
for one. A reader could be taught each such shape and recover the mod inside.

## Decision

An archive broken by its format is not accommodated (the user's call, 2026-10-01, during
ltk-forge). It is reported as what it is and left.

## Consequences

- The reader accepts what the mod managers the mods were made for accepted, and no more.
- Such archives are moved out of the corpus, with uploads their authors replaced and mods made
  for a model the game replaced.
