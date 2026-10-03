# ADR-0005: The census reader is carried over and its index starts at format 0

- **Status:** Accepted (2026-10-03)
- **Related:** #ADR-0001; `docs/design/ARCHITECTURE.md`, "Crates and modules"

## Context

A run asks the build history whether the game ever shipped a file, an object or a clip, and
when. The source is the census history: one commit per live build, published as
`LeagueToolkit/ltk-census-history`, builds on its `history` branch, in the census's format 2.
ltk-forge walks it with gix and folds it into a SQLite index, which a run reads.

ltk-forge's index is at its own format 8, the last of several while the census's format moved.
An earlier census export, format 1, was never published and is gone. ltk-forge's research
scripts still read it.

ltk-forge's history crate holds the walk, the index writer and the reader together, and depends
on the bin value helpers, the bank reader and `ltk_ritobin` for one function that reads what an
entry's text names.

## Decision

The census reader and the index are carried over from ltk-forge's code (the user's call,
2026-10-03): the walk, the fold, the tables, the append.

- The reader of the census is a crate of its own, `reforge-census`. It holds gix, `ltk_ritobin`
  and the patched `ltk_*` crates.
- The index's tables and the questions asked of them are the `history` module of `reforge`.
  `reforge-census` writes what that module defines.
- The index's format number starts at 0. reforge reads no index ltk-forge built.
- Only the census's format 2 is read. Nothing of format 1 is carried over: no reader, no script,
  no comparison.

## Consequences

- A run of reforge links no gix and no `ltk_ritobin`.
- An index is built once from the published history, 20 minutes on 32 threads as ltk-forge
  measured, and appended to afterwards.
- The tables no stage reads (13 of 18 in ltk-forge) are carried over with the rest. Leaving
  them out of a default build is decided when their cost is measured.
- ltk-forge's research scripts on format 1 are not ported. A conversion is read off the
  format 2 history when the next one is needed.
