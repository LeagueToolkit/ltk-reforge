# ADR-0073: A locale WAD's bank of a unit that is not voice goes to the base layer

- **Status:** Accepted (2026-10-06)
- **Related:** #ADR-0028, #GAME-BEHAVIOR-0007

## Context

A bank of a locale WAD went to that locale's voice layer, whatever its unit (#ADR-0028). Of the
2,665 copies of banks with a locale in the datasets ([RUNS.md](../design/RUNS.md), "The
behaviour carried over from ltk-forge"), one is of a unit that is not voice: an effects bank of
its own clips, shipped in the en_US WAD alone. Its effects were silent unless the voice layer
was on.

The options: keep it; the base layer, with the bank in the champion WAD, where it plays on every
install; the base layer, with the bank left in the locale WAD, where it plays on that locale's
installs alone, as the mod did.

## Decision

A locale WAD's bank of a unit that is not voice is carried as the champion WAD's would be: in the
base layer, in the champion WAD (the user, 2026-10-06).

## Consequences

- That mod's effects play on every install, the voice layer on or off.
- A voice unit's locale bank goes to its locale's layer as before.
