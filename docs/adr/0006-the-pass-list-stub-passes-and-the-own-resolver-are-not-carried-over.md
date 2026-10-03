# ADR-0006: The pass list, stub passes and the own resolver are not carried over

- **Status:** Accepted (2026-10-03)
- **Related:** #ADR-0001; `docs/design/REVIEW.md`, "The compiler shape, taken literally"

## Context

Three parts of ltk-forge exist for its pass registry or failed in game:

- `--passes a,b,c` runs any list of registry passes, checked for registry order. The named
  pipelines cover every use but debugging one repair.
- Five pipeline passes and two analyses are stubs with a milestone number, and the runner has
  rules for a stub that a built pass depends on.
- `own-resolver` ships a unit's resolver as the port's own object. In game (2026-09-30) a graph
  event did not find it: an emote lost its particles. It is opt-in, and its branches stay in
  lowering and in the output types.

## Decision

None of the three is in reforge (the user's call, 2026-10-03).

- A caller stops a run after a stage and chooses fixups and checks. There is no list of passes.
- What is not built is in the roadmap, not in the code.
- The resolver keeps the game's name, and its map is written whole.

## Consequences

- One fixup is tried alone by a selection that turns the others off.
- The resolver's map is frozen at the build a port is made at. Keys the game adds later are
  lost until the port is made again.
