# ADR-0003: A diagnostic does not stop the write

- **Status:** Accepted (2026-10-03)
- **Related:** `docs/design/DIAGNOSTICS.md`; `docs/design/REVIEW.md`, "Diagnostics"

## Context

A port of an old mod is seldom whole. A value has no conversion to the target's type, a file
the mod names is found nowhere, a unit fits no slot, a check finds a name unresolved. Most of
these mods still work in game with the fault in them, as the original did. ltk-forge reports
such a finding as an error and writes the package.

The rewrite proposed that an error from a check stop the write. The user's answer (2026-10-03):
a run cannot stop on every error. The diagnostics are to be carried through the whole run,
filtered well, named by a scheme and serialised, and many errors are to be shown to the person
with the output.

## Decision

A run writes its package whatever its diagnostics say, unless no package exists.

- Only a diagnostic that leaves the run with nothing to write ends it.
- A caller may raise a code to a level that refuses the write. No code has that level by
  default.
- The diagnostics are written with the output, in full and as a summary a person reads.

## Consequences

- A package may hold a known fault. The report beside it says which.
- A caller that wants a strict run (a corpus run, a release check) sets the levels for it.
- Severity says what the output lost against the mod as its author saw it. It does not say what
  the run did next.
