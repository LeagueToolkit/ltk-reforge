# ADR-0097: A stage is cancelled between its steps and reports its progress in them

- **Status:** Accepted (2026-10-10)
- **Related:** #ADR-0091, #ADR-0094; `docs/design/ROADMAP-UX.md`, "What the library lacks
  for the app"

## Context

A stage runs to its end. The app shows a stage as running or ended and nothing between, and
cannot stop one: the roadmap lists "progress inside a stage, and a way to stop one" as what
the library lacks. Most mods take one to five seconds through every stage and the slowest of
the datasets about 45, and a batch (#ADR-0091) runs many in a row.

A stage is already a loop over steps of one kind: fix over the fixups and the bins, facts and
plan over the units, check over the checks, build over the files. Between two steps the
stage's working state is whole, and after a cancelled stage the snapshot before it stands
(#ADR-0094).

## Decision

A stage checks for a cancel between its steps, and reports its progress in the same steps
(the user's call, 2026-10-10).

- The caller hands the run a cancel flag. A stage reads it between steps and returns a
  cancelled result, which ends the run the way a fatal finding does: the stages after it do
  not run, and the report says which stage was cancelled.
- A step is not interrupted. A fixup over one bin, one unit's facts, one check runs to its end.
- A stage reports its progress as the step it is on and the steps it has: the fixup and the
  bin, the unit, the check, the file. The app's run events carry it as they carry a stage's
  start and end.
- The command takes a cancel from the terminal's interrupt and ends the way the app does.

## Consequences

- A cancelled run has a snapshot for every stage that ended, and none for the one cancelled.
  The cancelled stage's bytes are in the run's memory layer (#ADR-0093) and go with it; the
  snapshots that ended are whole, and a rerun resumes from the last.
- Progress is counted in steps, not time: a step's length varies with the mod, and the app
  shows what is being done, not how long is left.
- The check for the flag between steps is a read of an atomic, and no stage is measurably
  slower for it. The reports are unchanged by it, which the corpus comparison holds.
