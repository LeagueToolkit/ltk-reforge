# ADR-0076: Units of one character tied on a slot are left to the game

- **Status:** Accepted (2026-10-06)
- **Related:** #ADR-0013, #ADR-0017

## Context

Of two units of one character on one slot, one keeps it: a unit whose entry reads, then the unit
written for the slot, then a skin0 unit, then a spread. The rank ended with the source slot, so
two units never tied and `target.conflict` was never raised. One mod of the datasets has a tie
the source slot broke ([RUNS.md](../design/RUNS.md), "The behaviour carried over from
ltk-forge"): 18 units of a sub-character, from slots 14, 27 to 36 and 47 to 53, follow their
champion to skin25, none fitting there, and the unit of the lowest slot kept it.

The options: keep the lowest slot; leave the tied units to the game and say so; end the run with
no package.

## Decision

The rank ends with the spread. Units of one character on one slot whose rank ties at the top all
leave the slot to the game, and `target.conflict` names them (the user, 2026-10-06).

## Consequences

- In that mod, the sub-character keeps the game's skin25, and the champion still ports.
- No unit of the character goes to the slot, the lower-ranked ones included.
- A unit that leaves this way is `unit.left` with the reason `conflict`.
