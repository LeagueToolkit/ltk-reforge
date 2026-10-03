# ADR-0036: Nothing is migrated before a release

- **Status:** Accepted (2026-10-03)
- **Related:** #ADR-0005, #ADR-0008

## Context

The history index, the report and the command line each have a format that other things read:
an index on disk, scripts over reports, a person's habits. Keeping an old format readable costs
code for each change. ltk-forge was declared a prototype, and reforge has no release.

## Decision

The index's format, the report's schema and the command line change without migration (the
user's call, 2026-10-03, during ltk-forge, and carried to reforge).

- An index of another format is rebuilt, never migrated. An index behind the history is appended
  to.
- A report says its schema, and a reader of another schema refuses it.

## Consequences

- A change of the index's format costs a rebuild from the history.
- A diagnostic code is the exception: it is permanent from the time the module that raises it
  lands (#ADR-0008).
- This ends at the first release, by a later ADR.
