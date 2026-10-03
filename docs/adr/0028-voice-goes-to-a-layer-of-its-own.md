# ADR-0028: Voice goes to a layer of its own

- **Status:** Accepted (2026-09-29)
- **Related:** #ADR-0027, #ADR-0029, #ADR-0030

## Context

A skin mod may carry voice lines beside its effects sounds: in the champion's WAD, or in a
locale WAD, which a player has only for the one locale the game is installed in. A mod's voice
changes how a champion speaks in every game, which a player may not want with the model. A
package can hold layers a player turns on and off.

## Decision

Every voice unit a port carries goes into a layer of its own, off until the player turns it on
(the user's call, 2026-09-29, during ltk-forge).

| voice from | layer |
|---|---|
| the champion's WAD | `voice` |
| a locale WAD | `voice-<xx-xx>` |

A layer holds its units' banks and one patch on the skin bin that sets the whole audio block:
the game's units, the base layer's, then the layer's, and the tags likewise. It applies over the
base layer's result.

## Consequences

- With the layer off, the base layer's audio block stands.
- A unit in a layer puts its tag in the layer's record only, and a literal sound name in the
  base layer never names a layer's renamed event. A literal name does not depend on a layer
  being on.
- A locale copy equal to the champion WAD's is carried once, in `voice`.
- Confirmed in game (2026-09-30): one sample's 252 voice lines play with the layer on and not
  without it, beside its effects in the base layer.
