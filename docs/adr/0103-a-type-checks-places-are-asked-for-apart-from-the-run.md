# ADR-0103: A type check's places are asked for apart from the run

- **Status:** Accepted (2026-10-11)
- **Related:** #ADR-0003, #ADR-0044, #ADR-0096, #GAME-BEHAVIOR-0002;
  `docs/design/ARCHITECTURE.md`, "Stages"; `docs/design/ROADMAP-UX.md`, "The run page"

## Context

The type check (`check.type-mismatch`, `check.field-undeclared`, `check.class-undeclared`)
counts the values of a package the client skips, by class, field and the two shapes, and
names one place per row: the WAD, the bin and the object's entry. The value's path inside
the object is not kept, and the other values of the row are a count. A person reading the
finding cannot tell which field of which struct holds the value, and cannot open the bin at
it.

A row of a port holds up to thousands of values: one field written wrong in every emitter of
every VFX system. The report's bytes are what the corpus compares between runs (#ADR-0044),
what a kept run stores (#ADR-0096) and what a package embeds (#ADR-0102).

The options were:

- the path on the finding's first place, and nothing more;
- every place with its path in the finding;
- the finding as it is, and the places asked for apart, by a second walk of the same check.

## Decision

The finding stays as it is. The places are asked for apart (the user's call, 2026-10-11).

- `Types::places` runs the check's walk over the same context `check` reads and gives every
  row with the places of its values, each place the chunk, the entry and the value's path
  from the object's root. It raises nothing. A row's places stop at a bound; its count goes
  on.
- A run gives them through `Run::type_places`, which reads the package and, in a port, applies
  the patches as `check` does. A kept run resumed through `build` can answer.
- A caller asks when a person opens a row's places or asks to see a value in its bin, and
  not when a run ends: the three findings are errors, and no run halts on them.
- Hashes stay hex in the report and in the places. A caller spells them with the run's names
  and the tables when it shows them.

## Consequences

- The report, the baseline and the stored runs are unchanged.
- Seeing a value costs a second read of the package's bins and, in a port, a second overlay.
  Both take seconds, and are paid only when asked.
- The places are not kept: a kept run with the bytes of its `build` snapshot answers again
  at any time, and a run with no bytes kept cannot.
- The same walk serves the three findings of the check, and no other check gives places until
  one needs to.
