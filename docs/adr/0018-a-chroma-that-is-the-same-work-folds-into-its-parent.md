# ADR-0018: A chroma that is the same work folds into its parent

- **Status:** Accepted (2026-09-30)
- **Related:** #ADR-0012, #ADR-0020

## Context

A chroma is a recolor of its parent skin, at a slot of its own. A mod for a premium skin may
ship its work at the skin's chromas too. Often the chroma's unit is the parent's unit again, and
what differs is the game's own chroma textures. Sometimes the author recolored a texture for
the chroma.

## Decision

A unit at a chroma leaves the port when another unit targets the chroma's parent and it is the
same work (the user's call, 2026-09-30, during ltk-forge): every own file it names is, by its
bytes, one the parent's unit names, and so is every own clip.

A chroma with work of its own, even one recolored texture, stays a unit. The files both name
ship once.

## Consequences

- A player who picks the chroma sees the game's chroma, not the mod.
- A change is not written onto a chroma the author did not ship.
- ltk-forge's first full datasets run folded 1,956 chroma units.
