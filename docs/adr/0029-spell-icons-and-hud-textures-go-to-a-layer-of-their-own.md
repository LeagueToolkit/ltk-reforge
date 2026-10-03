# ADR-0029: Spell icons and HUD textures go to a layer of their own

- **Status:** Accepted (2026-10-01)
- **Related:** #GAME-BEHAVIOR-0004, #ADR-0027, #ADR-0028

## Context

A mod may override a champion's spell icons and HUD textures. The champion's root bin names
them, so they show under every skin, whatever skin the rest of the port targets
(#GAME-BEHAVIOR-0004).

They could be left out, shipped with the skin's unit, or shipped apart.

## Decision

The port keeps them in a layer of their own, `hud` (the user's call, 2026-10-01, during
ltk-forge). The files ship under `mods/<ns>/characters/<character>/hud/`, with a patch on
the champion's root bin whose records name the shipped copies.

- A record replaces the deepest field that changed, never a whole spell, so the game's later
  balance changes show.
- Portraits (the circle and square icons) are a skin's own and stay with its unit.

## Consequences

- The layer is champion-wide: with it on, the icons show under every skin.
- A field on the way to an icon that no table names cannot be written, and that object is left
  out (#ADR-0031).
