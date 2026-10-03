# ADR-0017: A sub-character with no bin at its target gets a new one

- **Status:** Accepted (2026-09-30)
- **Related:** #GAME-BEHAVIOR-0001, #ADR-0013

## Context

A sub-character's unit follows its champion's target. When the champion's unit goes to a premium
slot T and the game ships the sub-character at skin0 alone, there is no game bin of the
sub-character at T to patch (#GAME-BEHAVIOR-0001).

The options: leave the sub-character to the game's skin0, or ship a bin the game does not have,
which the client loads.

## Decision

The port writes a `skin<T>.bin` for the sub-character (the user's call, 2026-09-30, during
ltk-forge): the game's skin0 bin of the sub-character, its entry and resolver renamed to T, with
the unit's changes applied and its links added.

## Consequences

- This bin ships whole, and shadows nothing of the game's.
- With T = 0 and a model that does not fit the base, the sub-character is left to the game.
