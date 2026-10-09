# ADR-0091: The app explores a mod, the history and its runs

- **Status:** Accepted (2026-10-09)
- **Related:** #ADR-0041, #ADR-0046, #ADR-0047, #ADR-0078, #ADR-0090;
  `docs/design/ROADMAP-UX.md`, "The second version"

## Context

The app runs one mod and shows the run (#ADR-0050). Everything else the library knows reaches
a person only through a run: what an archive holds, which build it was made at, which of its
files are the game's own bytes, when a path or an object existed in the game and what it held
at a build. The history index holds every entry's object bytes at every build (#ADR-0078),
and the questions that found the game changes of this project were answered by hand with
scripts over it. A fixup and a finding cite a record under `docs/` that the app shows as a
code and cannot open. A run's report is embedded in the package it writes (#ADR-0041) and
kept nowhere else.

## Decision

The app explores beside running (the user's call, 2026-10-09). It gains these pages, each a
reader of what the library already answers:

- **Mod**: what an archive holds before a run: its WADs, files by kind, characters and units
  with their slots, the build it was made at, which files are the game's own bytes and from
  when, and its image. This is the load report, rendered.
- **History**: a path or an object name searched, hashes spelled through the hash tables; at
  which builds and in which WADs it existed; a bin entry at any build rendered as text with
  names; one object compared between two builds.
- **Runs**: every run the app made, kept in its data folder and reopened; two runs of one mod
  compared; a package's embedded evidence read as a run.
- **Records**: the decisions, game changes, game behaviors and mod artifacts under `docs/`,
  bundled into the app at build time and opened from the fixup or the finding that cites one.
- **Batch**: a folder of mods run as one job, a table of outcomes, each row its run.

The pages add no rule about a mod (#ADR-0046): a renderer of the library's answers, a
command per question, and the names passed through `clean`.

## Consequences

- The library gains, each with its test: a read of an archive alone with its image; history
  queries that take the names and return spelled results, and an object diff; a reader of a
  package's embedded evidence; a run over several mods with one environment for a few.
- The app's data folder holds the runs' reports; their size is bounded and the oldest are
  removed first.
- The records are read-only in the app and edited in the repository.
