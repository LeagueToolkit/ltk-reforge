# ADR-0042: The census history is read from its history-v2 branch

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0035, #ADR-0005, #ADR-0036; `docs/design/HISTORY.md`

## Context

#ADR-0035 closed the census format: reforge asks for no change to it. A run on the history
alone, with no install, then showed two things a port reads from the game that the history did
not hold: the joints an animation clip animates, and an events bank's header and objects
(`docs/design/ROADMAP.md`, "Open"). Of the 568 sound units of a mod's own in the baseline, 246
can be cut only from the game's events bank, and every bank a port writes takes its header from
one.

The user had the census add both (2026-10-04). The history with them is the branch
`history-v2` of the same repository. Its `census.yaml` still says format 2: the additions are
new keys in entry files, and nothing that was there changed meaning. The branch `history`,
without them, leaves the published repository.

## Decision

reforge reads the builds from the branch `history-v2`. It reads format 2 as before and takes
the new keys as optional: `clip.joints` on an animation clip's entry, and `header` and
`objects` in an events bank's `bank` block.

This is one exception to #ADR-0035, made by the user, for a run without an install. The format
stays closed otherwise.

## Consequences

- The commits of `history-v2` are other commits than those of `history`. An index built from
  the old branch cannot be appended to. It is rebuilt (#ADR-0036).
- The index does not hold the new data until the reader folds it. Until then a run still needs
  an install for a game clip's joints and for a game events bank.
- A clone that holds only the old branch gives the reader's error for a missing branch.
