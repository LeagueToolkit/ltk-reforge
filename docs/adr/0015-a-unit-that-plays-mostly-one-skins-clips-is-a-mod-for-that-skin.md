# ADR-0015: A unit that plays mostly one skin's clips is a mod for that skin

- **Status:** Accepted (2026-09-30)
- **Related:** #ADR-0013, #ADR-0014, #ADR-0016

## Context

Under `own-rig` (#ADR-0014), ltk-forge's fifth datasets run left 193 units fitting no slot, 162
of them for clips from other skins. `allow` would have ported them all at skin0 and moved 270
premium fallbacks to skin0 besides.

In game, an own model on another skin's whole animation set is that skin: one sample plays skin
60's 21 clips and most of its effects. Authors who do this take a premium skin whole and change
its model.

## Decision

Reuse is not allowed wholesale. When a skin0 unit fits neither the base nor its premium origin,
and most of the clips it plays are one skin j's game clips, and its work fits j, then T = j (the
user's call, 2026-09-30, during ltk-forge). A minority of borrowed clips does not block a slot.

"Most" is more than half of the clips the unit plays. A tie between skins goes to the lowest
slot.

## Consequences

- Over the 173 mods then fitting no slot, 6 remained. 101 units went to a premium skin: 81 by
  their clips, 20 fitting their origin once a borrowed clip no longer blocked it.
- Players of the base do not see these mods. The user expected that some would dislike it and
  held the rule to be right.
- One borrowed clip does not make a mod that skin's.
