# ADR-0075: Of a unit's own events banks, the first that plays its clips is cut

- **Status:** Accepted (2026-10-06)
- **Related:** #ADR-0030, #GAME-BEHAVIOR-0015, #GAME-BEHAVIOR-0012

## Context

A media bank the author added is cut from the mod's own events bank of the client's version
(#ADR-0030). A unit can list several: its `bankPath` names them in an order of the author's
tool. Only the first was read. In 64 units of 18 mods of the datasets
([RUNS.md](../design/RUNS.md), "The behaviour carried over from ltk-forge") the first plays none
of the unit's own clips and one later bank plays all of them, so their 630 clips were not
carried. The order carries no meaning: 18 mods ship one of two copies of one added bank, and of
the 8 with its larger copy, the 3 that list it first were carried and the 5 that list the base's
first were not. No event a later bank keeps is in another bank of its unit.

## Decision

A bank's own clips are cut from the first of the unit's own events banks of the client's version
whose events reach one of them, and from the first when none does.

## Consequences

- The 64 units carry their clips, 1 to 26 events each.
- One mod carries 26 events under the ids of the game's events bank its block keeps. Which of two
  effects banks defining one id the client keeps is not established (#GAME-BEHAVIOR-0012), and the
  report counts shared ids against the game's bank at the unit's events path alone.
