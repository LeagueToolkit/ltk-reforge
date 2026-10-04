# ADR-0045: Every edge case has a baseline and a test of its own

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0044; `docs/design/ARCHITECTURE.md`, "Tests"

## Context

Up to parity with forge, behaviour was held by one comparison of whole runs over 5,137 mods. A
run of that size shows that something changed and in which mods. It does not show which rule
changed it, it takes twenty minutes, and it needs the datasets, the game copy and the history
index, which are on one machine.

The rules themselves are known by now: each game change, game behavior, mod artifact and
policy is a record, each finding is a diagnostic code, and each way a unit, a file or a clip
is judged is a variant of a closed set.

## Decision

Every edge case is established once and then tested in isolation (the user, 2026-10-04).

- **An edge case** is one rule with an outcome of its own: a record's case, a diagnostic code,
  a variant of a closed set (a reason a unit leaves, a reason for a target, a fixup, a
  conversion, a way a file is found).
- **Its baseline** is what reforge gives for it today, taken from the corpus: how many mods
  show it and which ones, by SHA-256. The fewest mods that together show every edge case are
  the reduced corpus.
- **Its test** builds the smallest fixture that shows the case, calls the one stage that
  decides it, and asserts the typed result. It needs no dataset, no game copy and no history
  index.
- A change is made to one rule at a time. Its test changes with it, and the run over the
  reduced corpus shows what else moved. The full datasets are run for a new baseline only.

## Consequences

- An edge case with no test is a gap, and the list of gaps is kept.
- An edge case no mod of the corpus shows is still tested, and is marked as not seen.
- A fault found in game becomes a fixture and a test before it is fixed.
- The reduced corpus is made again when an edge case is added.
