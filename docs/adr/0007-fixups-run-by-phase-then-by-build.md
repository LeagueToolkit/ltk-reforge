# ADR-0007: Fixups run by phase, then by build

- **Status:** Accepted (2026-10-03)
- **Related:** `docs/design/ARCHITECTURE.md`, "Fixups"

## Context

The order of the fixups matters in places. A tool's damage is undone before the layout it would
confuse is read. The fields Riot moved are moved before values are brought to the schema's
types. Fields are stripped last.

ltk-forge states the order as one list in its registry, with an edge from each repair to the one
before it. The list does not say which neighbours depend on each other and which only follow.
A fixup added by a person through a GUI has no place in a list written in code.

## Decision

A fixup states its phase, and a fixup that follows a change of Riot's states the build. The
engine runs the phases in order, and within the layout phase the oldest build first (the user's
call, 2026-10-03).

| phase | holds |
|---|---|
| `Undo` | a tool's damage taken out |
| `Layout` | a change of Riot's followed, by build |
| `Conform` | values brought to the target schema |
| `Strip` | what the target build does not declare, removed |

Two fixups of one phase and build run in the order of their ids.

## Consequences

- A fixup added later takes its place with no list to edit.
- The order of two fixups may differ from ltk-forge's. Where the outcome differs, the comparison
  with ltk-forge shows it (#ADR-0004), and the fixup's phase and build are looked at.
- A test pins the order of the standard set.
