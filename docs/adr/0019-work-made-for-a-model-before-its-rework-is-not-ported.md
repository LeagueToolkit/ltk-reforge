# ADR-0019: Work made for a model before its rework is not ported

- **Status:** Accepted (2026-10-01)
- **Related:** #GAME-CHANGE-0001, #ADR-0013, #ADR-0023

## Context

A rework replaces a champion's mesh, skeleton and effects for every skin at once
(#GAME-CHANGE-0001). Nothing a mod made for the old model leans on has a successor.

Some mods bring the old model back on purpose. They are made after the rework.

## Decision

A unit made for a champion's model before its rework, carrying no model of its own, is not
ported (the user's call, 2026-10-01, during ltk-forge). It gets no target and an error, and its
sub-characters go with it.

- Before or after is judged by the mod's files at the champion's own paths against the history,
  and by the mod's dating. Without the history nothing is judged.
- A mod that carries the old model whole (its mesh, its skeleton and most of the clips it plays)
  is judged like any own rig.
- Following a file to where the game holds it stops at a rework.

## Consequences

- 15 mods of ltk-forge's eighth datasets run are left to the game.
- A restoration made after the rework is ported, and its copies of the game's older bytes are
  its own, not stale (#ADR-0023).
- A person who wants a pre-rework mod anyway has to be asked. The run does not guess.
