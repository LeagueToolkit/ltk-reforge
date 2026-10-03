# ADR-0013: A skin0 unit goes to skin0 first, then to its premium origin

- **Status:** Accepted (2026-09-29)
- **Related:** #MOD-ARTIFACT-0002, #ADR-0010, #ADR-0012, #ADR-0015, #ADR-0016

## Context

Many mods at slot 0 are a premium skin pasted into the base slot (#MOD-ARTIFACT-0002). Ported
onto the base as it is, such a mod plays the premium skin's clips on the base's rig, or names
objects the game loads only for skin N.

The origin O of a unit is the game skin its content derives from: the base, a premium skin, the
author's own work, or unknown.

## Decision

A skin0 unit, and a spread, is placed in this order (the user's call, 2026-09-29, during
ltk-forge):

1. T = 0 when the unit's work fits the base's rig: its mesh, submeshes, skeleton and clips. A
   paste of a premium skin whose own mesh and skeleton are the base's fits as it is.
2. Otherwise T = N when O is the premium skin N and the work fits N. The mod becomes a mod for
   skin N.
3. Otherwise #ADR-0015, then #ADR-0016.

A sub-character's unit takes its champion's target for the same slot.

## Consequences

- A paste that fits the base is cleaned onto the base and every player sees it.
- A paste that does not fit becomes a mod for the premium skin, and players of the base do not
  see it.
- An unknown check does not count as a fit, and does not block. It is reported.
