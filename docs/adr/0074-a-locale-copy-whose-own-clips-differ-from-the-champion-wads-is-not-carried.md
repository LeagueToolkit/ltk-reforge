# ADR-0074: A locale copy whose own clips differ from the champion WAD's is not carried

- **Status:** Accepted (2026-10-06)
- **Related:** #ADR-0028

## Context

When a mod ships a bank in its champion WAD and a copy in a locale WAD, the copy is carried once
when its own clips are the champion WAD's. When they differ, the copy is not carried and
`audio.not-carried` says so. No mod of the datasets has a differing copy
([RUNS.md](../design/RUNS.md), "The behaviour carried over from ltk-forge"): of 1,471 locale
copies with own clips, one has a copy in the champion WAD at the same path, byte for byte the
same. Whether a differing copy is an author's localisation or a tool's stale copy cannot be read
off the corpus.

The options: keep it; carry the copy as a unit of its own in `voice-<xx-xx>`, after the champion
WAD's unit; carry the clips of both.

## Decision

A locale copy whose own clips differ from the champion WAD's copy is not carried, and is reported
(the user, 2026-10-06).

## Consequences

- No mod of the datasets is affected.
- A mod that localises its own voice in such a copy loses it, and the report says which.
