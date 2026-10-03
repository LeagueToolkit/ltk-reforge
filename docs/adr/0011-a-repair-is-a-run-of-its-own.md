# ADR-0011: A repair is a run of its own

- **Status:** Accepted (2026-09-29)
- **Related:** #GAME-BEHAVIOR-0002, #ADR-0002, #ADR-0007; `docs/design/ARCHITECTURE.md`, "Shape"
  and "Fixups"

## Context

A mod made at one build and played at a later one loses every bin value whose type Riot changed
in between (#GAME-BEHAVIOR-0002). Bringing a mod's values to the build a player runs is wanted
by itself, with the mod left at its own slots and paths. LTK Manager calls this repair. A port
needs the same changes before it compares the mod with the game.

The changes could be steps inside the port only, or transforms of the mod that any run uses.

## Decision

A change that brings a mod to the target build is a general transform of the mod, and repairing
a mod is a run of its own beside porting it (the user's call, 2026-09-29, during ltk-forge).

- `repair` gives the mod as it is, at its own slots and paths, fixed for the target build.
- `port` runs the same fixups, then ports.
- Both write the same kind of package.

## Consequences

- A fixup knows no target slot.
- A fixup is idempotent: a repaired mod repaired again changes nothing.
- A repaired mod ships whole bins, which shadow Riot's at every patch. A port does not.
