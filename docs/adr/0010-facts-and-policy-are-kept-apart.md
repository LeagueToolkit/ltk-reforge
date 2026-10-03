# ADR-0010: Facts and policy are kept apart

- **Status:** Accepted (2026-09-29)
- **Related:** #ADR-0001; `docs/design/ARCHITECTURE.md`, "Facts" and "Decide"

## Context

A port has to choose the slot each skin unit goes on. The choice rests on what is true of the
mod: which units are copies, which game skin a unit derives from, whether its mesh, skeleton and
clips fit a slot. It also rests on a preference: how much borrowed animation is accepted, whether
a person forced a slot.

When the same code finds a fact and chooses by it, the fact cannot be reported without the
choice, and another preference cannot be tried without finding the facts again.

## Decision

What is true of a mod and what is chosen for it are two things (the user's call, 2026-09-29,
during ltk-forge).

- Facts choose nothing. A fact holds for every policy.
- A policy chooses from the facts. It is neither part of reading the mod nor part of writing the
  output.
- A run reports what the other policies would have chosen beside what it chose.

## Consequences

- A report up to the decision needs no output.
- A judgment that a fixed rule makes from evidence (the origin of a unit, the representative of
  a cluster of copies) is a fact. Its evidence is kept with it.
- A level of acceptance (the animation-reuse level) is policy, and the facts under it are
  gathered for every level.
