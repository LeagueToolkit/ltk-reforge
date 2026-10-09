# ADR-0087: A skin0 unit that leaves fixes no target

- **Status:** Accepted (2026-10-09)
- **Related:** #ADR-0012, #ADR-0019, #ADR-0020, #ADR-0076, #ADR-0082

## Context

A port changes its target slot only: the character's target slot is where its skin0 unit
goes, and every other unit of the character leaves (#ADR-0082). When no unit of the author's
sits at skin0, every unit ships at its own slot. #ADR-0082 leaves open what a skin0 unit that
leaves the port decides for the other units.

A skin0 unit leaves for one of six reasons: it is a copy of another unit or another unit took
its slot, in which case that unit holds the target; it is the game's own skin shipped beside
the author's work (#ADR-0020); it is made for the champion's model before its rework
(#ADR-0019); it is a sub-character's that equals the game's; units of one rank tie on its
target (#ADR-0076); or it has no target for another reason.

Over `baseline-port-5` (`docs/design/RUNS.md`, "The baselines at 8cacfff"), 16 ports have a
champion's skin0 unit that leaves. In 2 another unit took its slot. In 4 it is the game's own
skin, and the author's unit sits at a premium slot: Zed's at skin 1, Gragas's at skin 4,
Darius's at skin 14. In 10 it is made before a rework, and the port ships nothing. No skin0
unit of a champion ties on its target.

## Decision

A champion's skin0 unit that leaves for any reason but a copy or a superseded one fixes no
target slot. Every other unit of the character ships at its own slot, as when no unit of the
author's sits at skin0. A copy's target is its original's, and a superseded unit's is the unit
that took its slot.

The game's own skin at skin0 is not the author's work for that slot, so it says nothing about
which skin the author meant; the author's unit at a premium slot stays there (#ADR-0012). A
unit made before a rework fits nothing at the target build, and a unit with no target decides
none.

## Consequences

- The 4 ports with the game's own skin at skin0 ship the author's premium unit, and the 10
  ports made before a rework ship nothing, as `baseline-port-5` has them.
- A champion whose skin0 unit ties on its target has every other unit ship at its own slot.
  No mod of the corpus shows the case.
