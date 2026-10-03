# ADR-0001: The stages are types in one crate

- **Status:** Accepted (2026-10-03)
- **Related:** `docs/design/REVIEW.md`, "Stage boundaries"; `docs/design/ARCHITECTURE.md`, "Shape"
  and "Crates"

## Context

ltk-forge holds every pass in one crate, `forge-passes`, as free functions over one mutable
module. Its layering runs both ways: analyses call transforms and lowering, and the frontend
calls analyses. Nothing in the code says which stage a module is in.

The rewrite makes each stage a type that takes the earlier stages' results and returns its own,
so the compiler enforces the order of the stages. Two ways to keep a stage from calling a later
one were open:

- one crate per stage boundary, about 14 crates, where the dependency list enforces it;
- one crate with a module per stage, where a test enforces it.

## Decision

The stages are modules of one crate, `reforge` (the user's call, 2026-10-03; the name by
#ADR-0009). A crate of its own is kept for a part with a boundary of its own: the file kinds,
the bank reader, the census reader, the command, the test builders.

A module of a stage uses only the modules of earlier stages. A test reads the `use` lines of the
crate and fails on a use that points the other way.

## Consequences

- One manifest and one compile unit for the stages. A change across two stages is one change.
- The order of the stages is enforced by their types. The direction of the modules is enforced
  by a test, not by the build: a use the wrong way compiles and fails in `cargo test`.
- Splitting a module into a crate later is possible while the test holds.
