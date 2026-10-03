# ADR-0016: A unit that fits no slot is written at skin0 with an error

- **Status:** Accepted (2026-09-30)
- **Related:** #ADR-0003, #ADR-0013, #ADR-0015

## Context

A skin0 unit may fit neither the base, nor its premium origin, nor a skin whose clips it plays:
half its clips are another skin's, it mixes two skins' clips, or its own clips move no joint of
its own skeleton. After #ADR-0015, ltk-forge's datasets held 6 such units, and a champion swap
that replaces one champion with another.

The run could leave the unit out, or write it somewhere for a person to look at.

## Decision

The unit keeps T = 0 and is written at skin0, with an error that lists every check that failed
(the user's call, 2026-09-30, during ltk-forge). A person reviews it.

## Consequences

- The package holds a unit known not to fit. The report says so beside it (#ADR-0003).
- Whether such a unit should be written at all stays open until a person has looked at the ones
  the corpus holds.
