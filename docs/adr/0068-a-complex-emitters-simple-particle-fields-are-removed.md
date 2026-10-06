# ADR-0068: A complex emitter's simple-particle fields are removed

- **Status:** Accepted (2026-10-06)
- **Related:** #GAME-CHANGE-0004, #GAME-BEHAVIOR-0024, #GAME-BEHAVIOR-0002

## Context

At 12.19 Riot moved eleven fields of an emitter into its `LegacySimple`, and at 12.20 removed the
`LegacySimple` from most of the game's emitters, with its values (#GAME-CHANGE-0004). At 16.19
the emitters that hold one are the simple ones.

A system lists each emitter as simple (`simpleEmitterDefinitionData`) or complex
(`complexEmitterDefinitionData`). The client draws a simple emitter at the size its
`LegacySimple` gives, and reads no `LegacySimple` on a complex emitter (#GAME-BEHAVIOR-0024). A
mod made before 12.19 holds the fields on its emitters, of both lists.

The options for a complex emitter: move its fields into a `LegacySimple`, as for a simple one;
or remove them, as Riot did. The client draws it the same either way.

## Decision

The user, 2026-10-06: **an old emitter's simple-particle fields are moved into `LegacySimple` on
a simple emitter, and removed from a complex one,** as Riot removed its own at 12.20.

- Simple and complex are the list the system holds the emitter in (#GAME-BEHAVIOR-0024).
- An emitter no system lists is drawn by none, and its fields are moved.
- A removal from a complex emitter is reported as removed with nothing lost: the client draws
  the emitter the same with them.

## Consequences

- A repaired complex emitter holds no `LegacySimple`, as Riot's do.
- A complex emitter with a primitive is not checked in game (#GAME-BEHAVIOR-0024). A check that
  shows one drawn by its `LegacySimple` reopens this.
