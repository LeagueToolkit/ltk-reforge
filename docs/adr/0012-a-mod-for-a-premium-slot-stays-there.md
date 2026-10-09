# ADR-0012: A mod for a premium slot stays there

- **Status:** Accepted (2026-09-29); every unit of a mod with a bin for each slot kept superseded
  by #ADR-0082
- **Related:** #ADR-0010, #ADR-0013, #ADR-0020

## Context

Every skin bin a mod ships is a unit, at a source slot S. A port chooses the target slot T its
output goes on. A mod written for a premium skin N ships `skin<N>.bin` and builds on that
skin's rig and effects. Only players who own skin N see it.

Such a mod could be moved to the base skin so that every player sees it, where its work fits the
base's rig.

## Decision

A unit with S other than 0 that is neither a copy of a skin0 unit nor part of a spread gets
T = S (the user's call, 2026-09-29, during ltk-forge). There is no fit test and no downgrade.
Its origin is still reported.

## Consequences

- A mod made only for a premium skin is found and ported though it ships no `skin0.bin`.
- A mod that writes a different bin for each slot keeps every unit on its own slot.
- Nothing moves a skinN mod onto skin0. The only spread is the mod manager's, from skin0 to
  every skin (#ADR-0020).
