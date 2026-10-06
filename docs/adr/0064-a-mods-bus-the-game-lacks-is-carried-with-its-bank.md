# ADR-0064: A mod's bus the game lacks is carried with its bank

- **Status:** Accepted (2026-10-06)
- **Related:** #GAME-BEHAVIOR-0025, #MOD-ARTIFACT-0035, #ADR-0030; `docs/design/ARCHITECTURE.md`,
  "Port"

## Context

A port writes each media bank it makes with the mod's own clips alone. A mod's media bank can
hold bus objects beside its clips: copies of the game's buses and buses of its own
(#MOD-ARTIFACT-0035). Without its own buses, none of the sounds of its events bank are heard; the
copies of the game's buses change nothing heard (#GAME-BEHAVIOR-0025).

The options: carry no bus, as the port did; carry every bus object of the mod's bank; carry the
ones whose ids the game's init bank does not define.

## Decision

The user, 2026-10-06: **a media bank made from a mod's bank carries the bus objects of that bank
whose ids the game's init bank does not define. The copies of the game's buses are left out.**

- The game's buses are the bus and auxiliary bus objects of the init bank of the game the port
  is judged against. A game with no init bank carries no bus.
- A bus object in the events bank a port cuts stays with every other object of the cut
  (#GAME-BEHAVIOR-0017). No events bank of the datasets holds one (#MOD-ARTIFACT-0035).

## Consequences

- The three mods of the datasets with such buses carry them, and their events play
  (checked on one, #GAME-BEHAVIOR-0025).
- A bus the port carries has an id the game lacks, so it changes none of the game's sounds.
- A mod's sound that needs a copy's parameters over the game's would not have them. No such sound
  is seen.
