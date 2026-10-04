# ADR-0004: ltk-forge is the oracle of the rewrite

- **Status:** Superseded by #ADR-0044
- **Related:** `docs/design/ROADMAP.md`, "Rules of the rewrite"; `docs/design/CODES.md`

## Context

ltk-forge ports 5,136 of the 5,137 champion skin mods in its corpus, and its output is confirmed
in game on a sample set. The rewrite changes the structure and is meant to keep the behaviour.
A rewrite checked only by its own tests can lose behaviour nobody wrote a test for.

The tooling that runs ltk-forge over the corpus and compares two runs (the runner, the diff, the
reducer, the reduced corpus list, the baseline runs) sits in ltk-forge's ignored `data/` folder.
It compares unit states, decision reasons and diagnostic codes as strings.

The options for the comparison's home: this repository, a repository of its own, or ltk-forge.

## Decision

Each step of the rewrite is compared over the corpus with what ltk-forge gives: unit fates and
targets, diagnostics, package contents. The output does not have to be the same. A step ends
when the deviation is small and each kind of difference has been looked at (the user's call,
2026-10-03).

What the comparison needs is built in ltk-forge, which is changed directly for it (the user's
call, 2026-10-03). This repository holds no corpus tooling.

## Consequences

- ltk-forge stays buildable and runnable for as long as the rewrite lasts.
- ltk-forge is changed for the comparison only. Its faults are not fixed there.
- A change of behaviour that is meant (a policy, what a port carries) is an ADR here. A small
  deviation that is not is noted with the run that measured it, in the runs log.
- A large deviation, or one nobody can explain, is a fault of the rewrite.
- reforge names its diagnostics by another scheme (#ADR-0008). `docs/design/CODES.md` gives
  ltk-forge's code for each, and the comparison reads it.
