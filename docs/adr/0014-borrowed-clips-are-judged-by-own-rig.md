# ADR-0014: Borrowed clips are judged by own-rig

- **Status:** Accepted (2026-09-29)
- **Related:** #ADR-0010, #ADR-0013, #ADR-0015

## Context

A unit's animation graph may play clips that only other skins of the game play. Whether such a
clip lets the unit sit on a slot is a level of acceptance:

| level | a clip only other skins play |
|---|---|
| `block` | blocks the slot |
| `same-joints` | fits when the skin it comes from has the slot's skeleton joints |
| `own-rig` | fits when the unit's mesh and skeleton are the mod's own and at least half its clips are its own, or when the clips from other skins are fewer than half of what it plays; otherwise blocks |
| `allow` | fits when it moves some joint of the unit's skeleton |

Over ltk-forge's first corpus (1,029 mods) under `block`, clips alone ruled out skin0 for 317 of
677 fallbacks. `allow` would have moved 403 more units to skin0, most of them pastes that bring
a premium skin's clips with little of their own.

## Decision

The default level is `own-rig` (the user's call, 2026-09-29, during ltk-forge). An author who
made their own model and most of its animations may borrow a premium skin's remaining clips, and
the port stays on skin0. A paste that brings a skin's clips with little of its own does not.

Every level is judged in every run, and the report says what each would choose.

## Consequences

- An own model with 33 of 59 clips its own goes to skin0. A champion swap with none of 22 its
  own does not.
- A borrowed dance does not move a unit off a slot.
- `--anim-reuse` chooses another level for a run.
