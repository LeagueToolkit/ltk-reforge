# ADR-0009: The crates and the command are named reforge

- **Status:** Accepted (2026-10-03)
- **Related:** #ADR-0001, #ADR-0004

## Context

The rewrite lives in the repository ltk-reforge. ltk-forge stays buildable beside it for the
whole rewrite, and both are run over the same corpus. With the same crate names and the same
command, a report, a log line or a path does not say which of the two made it.

## Decision

The rewrite is named reforge (the user's call, 2026-10-03).

- The crates are `reforge`, `reforge-cli`, `reforge-census`, `reforge-magic`, `reforge-wwise` and
  `reforge-testkit`.
- The command is `reforge`.
- What a run writes carries the name: `reforge-report.json`, the report's schema id.

"forge" in this repository's docs means ltk-forge.

## Consequences

- The two tools install and run side by side.
- A commit's scope is a crate without its prefix (`census`, `magic`, `wwise`, `cli`, `testkit`)
  or a module of `reforge` (`model`, `diag`, `load`, `fixups`, and the others).
