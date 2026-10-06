# ADR-0065: A value no build since the mod's own read is removed

- **Status:** Accepted (2026-10-06)
- **Related:** #GAME-BEHAVIOR-0002, #GAME-CHANGE-0004, #GAME-CHANGE-0023

## Context

A fixup brings a value to the place and the type the target build reads. Some values of a mod
were read by no build from the mod's own to the target's: a field the mod's own build did not
declare where the value is, or a value of a type no build of that range read there. The client
dropped such a value when the author made the mod (#GAME-BEHAVIOR-0002), so the author never saw
it. A tool wrote it, or the author pasted it from an older mod (#GAME-CHANGE-0004, "In a mod").

The options: convert it as an older mod's value of that field is converted, so that it shows at
the target build for the first time; or remove it, so that the mod shows at the target what it
showed when it was made.

The fixups convert only a value some build since the mod's own read, and the strip removes the
rest. That was a policy of ltk-forge's docs, with no decision behind it.

## Decision

The user, 2026-10-06: **a value no build since the mod's own read is removed, not revived.**

- The builds since the mod's own run from the build the mod is dated at to the target's. A mod
  the history does not date, or one dated before what the schema describes, may have shown the
  value, and its value is converted.
- An exception is a conversion with a game change behind it, never a policy. A record of Riot's
  change says why the value is converted whatever the date: the health bar style of a champion's
  skin entry names the numbering it was written in (#GAME-CHANGE-0023).

## Consequences

- A repaired mod shows what its author saw, and nothing its author's game dropped.
- A value an author pasted from an older mod is removed, with what the author meant by it.
- A mod dated later than it was made, by a game file a tool copied in, loses values its author
  saw. A conversion that keeps them rests on a record of that artifact.
- A removal is reported as removed with nothing lost.
