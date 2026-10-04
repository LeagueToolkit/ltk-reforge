# ADR-0044: ltk-forge is retired, and reforge is compared with its own baseline

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0004, which this supersedes; `docs/design/RUNS.md`, "The port", "Ports in
  game"; `docs/design/ROADMAP.md`

## Context

#ADR-0004 made ltk-forge the oracle of the rewrite: each step was compared with forge over the
corpus, and the comparison was built in forge.

Every step has been compared. Over the 5,137 mods of the datasets no port of reforge's differs
from forge's outside the listed deviations, in the units that ship, the chunks, and each patch
applied to the game's bin. Five ports of reforge's were checked in game by the user and work
(2026-10-04).

What comes next changes behaviour away from forge on purpose: a run on the history alone, the
author's change taken against the game at the mod's own build, the faults of forge that were
carried over for the comparison. A comparison with forge then shows the changes that are meant
beside the ones that are not.

## Decision

ltk-forge is retired (the user, 2026-10-04). reforge is compared with its own baseline: the
reports of an earlier run of reforge over the same mods.

- The corpus tooling moves into this repository: the scope of the corpus, the runner, the
  reducer and the comparison of two runs.
- A change of behaviour is measured as the difference between the baseline and a run with the
  change. Each difference is meant, or it is a fault.
- forge's checkout is kept as a backup outside the repository, with the reports of its last
  run. It is not built, run or changed for any later work.

## Consequences

- The last comparison with forge is the record of parity: `docs/design/RUNS.md`, "The port".
- A baseline is as good as the program that made it. A fault both forge and reforge have stays
  in the baseline until a test or a check in game shows it.
- The column of forge's codes in `docs/design/CODES.md` stays, as the map for a person who
  reads an old forge report.
- The faults carried over from forge for the comparison (`docs/design/ROADMAP.md`, "Open") can
  be fixed, each as a change measured against the baseline.
