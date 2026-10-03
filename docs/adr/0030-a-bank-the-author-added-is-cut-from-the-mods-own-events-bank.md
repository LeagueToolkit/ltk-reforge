# ADR-0030: A bank the author added is cut from the mod's own events bank

- **Status:** Accepted (2026-10-01)
- **Related:** #MOD-ARTIFACT-0005, #ADR-0028

## Context

A port carries only the mod's own clips. For a media bank at a path a game unit loads, the
game's events play them: the base's for a bank at the base's path, and for a bank at a premium
skin's path the game's events bank there, cut to the events that reach an own clip.

Some authors add a bank at a path no game unit loads, with events of their own
(#MOD-ARTIFACT-0005). No event of the game's plays its clips.

## Decision

Such a bank's clips are carried, and its events are cut from the mod's own events bank when that
bank is of the version the client loads (the user's call, 2026-10-01, during ltk-forge, once a
sample's added sounds played in game). `--game-events-only` reports such clips as not carried.

## Consequences

- The mod's own event names are kept, since its bins name them.
- A kept event that shares an id with one of the game's bank is counted in the report. The
  samples hold none.
- An added bank beside an events bank of an older version is not carried, and is reported.
