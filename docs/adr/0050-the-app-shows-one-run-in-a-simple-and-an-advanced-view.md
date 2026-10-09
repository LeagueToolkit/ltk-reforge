# ADR-0050: The app shows one run in a simple and an advanced view

- **Status:** Accepted (2026-10-04); the two views as the only ones superseded by #ADR-0090
- **Related:** #ADR-0003, #ADR-0024, #ADR-0045, #ADR-0046, #ADR-0090;
  `docs/design/ROADMAP-UX.md`

## Context

The app is a caller of `reforge` (#ADR-0046). Most users want a mod fixed with the standard
settings and nothing to choose. A user whose mod the defaults do not fix needs to see where the
run went wrong and to change what it is given.

A `Run` runs each stage once and holds every result up to the last stage it reached. A run with
other settings is a new `Run` from the archive, which takes from one to some forty-five seconds
on the datasets. A cache of stage results would make a rerun faster and would be a second
place where a stage's result lives.

## Decision

The app shows one run in two views, the user's choice of 2026-10-04. The two are views of the
same run, not two programs and not two code paths.

- **The simple view.** The user picks a mod and the game folder and presses one button. The app
  runs every stage with the standard settings, shows what was fixed and every warning and
  error, and writes the `.modpkg`.
- **The advanced view.** The stages are shown in order: read, load, fix, facts, decide, plan,
  build, check, write; a repair has no facts, decide or plan. Each stage shows whether it ran,
  its result and the findings it raised. The user runs up to a chosen stage, looks at the
  result, changes a setting and runs again.

A changed setting runs a new `Run` from the archive. The app keeps no cache of stage results.

## Consequences

- The app offers only the settings the library has. A setting the UI wants and the library
  lacks is a library change with its test (#ADR-0045), never imitated in the UI.
- The simple view hides no error. A finding does not block the write unless the user's policy
  says so (#ADR-0003), so the errors are shown before the write.
- A rerun costs the whole run up to the chosen stage. When that is too slow in practice, the
  measurement is reported and the choice is revisited.
- A target is chosen for all of a mod's units at once. A target per unit is wanted, and is a
  change to the library.
