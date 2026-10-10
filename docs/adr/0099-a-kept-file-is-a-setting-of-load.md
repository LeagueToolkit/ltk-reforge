# ADR-0099: A kept file is a setting of load

- **Status:** Accepted (2026-10-10)
- **Related:** #ADR-0023, #ADR-0090, #ADR-0094, #ADR-0098; `docs/design/ARCHITECTURE.md`,
  "Shape"

## Context

#ADR-0098 decides a keep per file, for a stale copy the run would drop, and says `decide`
and `plan` read it, so that a change of it resumes from the load snapshot. #ADR-0094 says
nothing the guided view asks (#ADR-0090) is read by `load`.

The code decides a stale copy at `load`: the stage marks each file the game's own bytes
supersede as stale, and the whole-mod stale keep is a setting `load` declares, which
#ADR-0094's own table lists. `plan` and `build` judge files against the same mark again,
through `Staleness`. A keep per file is the same question asked of one file, so the stage
that answers it is `load`, and the two ADRs contradict the table between them.

The options were to move the stale mark out of `load` so that the keep is `plan`'s, or to
declare the keep where the mark is made.

## Decision

A kept file is a setting `load` declares, as the stale keep is, and `plan` and `build`
declare it too where they judge staleness again. A change of it resumes from the raw
snapshot.

The stale mark stays at `load`: it is read by the provenance and the inventory of the same
stage, and moving it would put a load fact in a later stage for the resume's sake alone.

## Consequences

- #ADR-0098's bound that the keep is read by `decide` and `plan` and resumes from the load
  snapshot is superseded by this record.
- #ADR-0094's statement that nothing the guided view asks is read by `load` is superseded:
  the stale question is, and its answer costs a rerun of `load`, which is one to five
  seconds for most mods. Every other answer of the guided view resumes from the load
  snapshot as before.
- The guided view asks the stale question before the others where it can, so that its rerun
  of `load` happens once.
