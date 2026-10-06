# ADR-0072: What a unit reaches is reported for units that ship

- **Status:** Accepted (2026-10-06)
- **Related:** #GAME-BEHAVIOR-0006

## Context

`Reach` and `FileRefs` are found for every representative unit with a target, and their findings
(`object.link-dangling`, `object.unreached`, `object.game-carried`, `ref.dangling`,
`ref.unmounted`) were raised for each. A unit can leave the port after that: a chroma folded into
its parent, or the game's own skin shipped again. 3,529 findings in 118 mods of the datasets
([RUNS.md](../design/RUNS.md), "The behaviour carried over from ltk-forge") were on such units,
about output the port does not write.

## Decision

The findings of what a unit reaches and of the files it names are raised for the units that ship
(the user, 2026-10-06).

## Consequences

- A unit that leaves is reported by `unit.left` alone.
- `Reach` and `FileRefs` are still found for a unit that leaves: `Shipping` judges from them.
