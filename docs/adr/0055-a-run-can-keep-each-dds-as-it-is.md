# ADR-0055: A run can keep each DDS as it is

- **Status:** Accepted (2026-10-05)
- **Related:** #ADR-0022, #GAME-BEHAVIOR-0003, #GAME-CHANGE-0007, #ADR-0050

## Context

A run puts a DDS at a `.tex` path in TEX's container where its blocks go in as they are, and
ships any other DDS as it is (#ADR-0022). The client reads either container by its magic
(#GAME-BEHAVIOR-0003), which is inferred and not checked in game. A user who suspects a texture
the run moved, or who checks that behaviour in game, has no way to ship the mod's own bytes.

## Decision

A run takes an option to keep each DDS as it is: no texture goes into TEX's container, and each
is counted as kept under the option (the user's call, 2026-10-05). The option is off by default.
It is a setting of `Porter` and `Repairer`, `--keep-dds` on `port` and `repair`, and a switch
in the app's advanced view beside "keep stale copies".

## Consequences

- With the option on, a DDS ships at the `.tex` path its port or repair gives it, as the bytes
  the mod held. The paths and the names in bins do not change.
- A texture that is a stale copy only once in TEX's container is not judged so, and ships.
- The simple view runs with the option off.
