# ADR-0070: A run's bounds sit well above the largest value measured

- **Status:** Accepted (2026-10-06)
- **Related:** #ADR-0044

## Context

A run reads a mod and the game under bounds: the archive's caps, the bins followed from one
root of a mod's load, the bins read of one game skin, and the game chunks of a file's size read
to find its bytes under another path. Each was ltk-forge's value, with no measurement behind it.

Measured over the datasets and the game at 16.19 ([RUNS.md](../design/RUNS.md), "The behaviour
carried over from ltk-forge"), three of them were passed by real input with nothing raised: 533
of 19,609 game skins link more than the 16 bins a skin's catalog read, one mod's root reaches 89
bins against a cap of 64, and files meet up to 1,167 game chunks of their size against 64 read.
A bound passed in silence changes what a run finds. The cap on a packed WAD's chunks stood above a
stated 200,000, a margin the user chose, and the game's largest WAD holds 90,012.

## Decision

Each bound is set well above the largest value the datasets and the game hold, rounded up to a
power of two, with the measurement beside it in the code; and a run that passes one of the three
bounds nothing reported says so with a note (the user, 2026-10-06).

| bound | value | largest measured |
|---|---:|---|
| entries of an archive | 262,144 | 2,033 |
| chunks of a packed WAD | 262,144 | 5,261 in the datasets, 90,012 in the game |
| one entry or chunk, inflated | 1 GiB | 439,570,394 bytes |
| bins followed from one root | 128 | 89 |
| bins read of one game skin | 64 | 41 |
| bins the apply check loads for one patched bin | 128 | 44 |
| game chunks of a file's size read | 2,048 | 1,167 |

The caps on the archive file and on all its bytes inflated stay at 4 GiB, nine and four times the
largest measured. A passed archive cap was reported before and still is. The apply check loads
twice the bins read of one game skin, so a skin's chain and the package's own bins fit in it.

## Consequences

- `bin.load-capped`, `file.same-size-capped`, `catalog.capped` and `check.load-capped` report a
  passed bound. No run of the datasets raises one.
- The game's chunks of a file's size are read up to 2,048 times per file, from a cache per mod.
- A bound is measured again when the game or the datasets grow past its margin.
