# ADR-0008: A diagnostic code names what the finding is about

- **Status:** Accepted (2026-10-03)
- **Related:** #ADR-0003, #ADR-0004; `docs/design/DIAGNOSTICS.md`, "Names";
  `docs/design/CODES.md`

## Context

Half of ltk-forge's diagnostic codes begin with the name of the pass that raises them:
`survey.wad-guessed`, `lower.no-game-bin`, `strip-undeclared.lost`, `vfx-filtering.moved`.
reforge has no passes of those names. Each repair has codes of its own, so a fixup a person adds
would need a new code. A few codes have two severities.

Keeping ltk-forge's codes makes the comparison with it direct. Renaming them needs a table from
old to new.

## Decision

A code is `area.finding`. The area is the thing the finding is about, in the glossary's words,
and never a stage, a module or a function (the user's call, 2026-10-03, with the scheme and the
table left to the design).

- A code has one severity and one type of data.
- The fixups share their codes and are told apart by the fixup's id in the diagnostic.
- A code is permanent from the time the module that raises it lands.
- `docs/design/CODES.md` lists every code with ltk-forge's code beside it.

## Consequences

- A code survives when the code that raises it moves.
- The comparison with ltk-forge goes through the table.
- A report of reforge cannot be read by ltk-forge's corpus scripts as it is.
