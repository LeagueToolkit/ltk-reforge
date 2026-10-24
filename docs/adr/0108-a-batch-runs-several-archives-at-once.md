# ADR-0108: A batch runs several archives at once

- **Status:** Accepted (2026-10-24)
- **Related:** #ADR-0091, #ADR-0107; [docs/design/ROADMAP-UX.md](../design/ROADMAP-UX.md),
  "The Batch page"

## Context

A batch ran its archives one after another. A scan of one mod of a champion skin took 29
seconds on the user's machine and a port after it 22, so a library of two hundred mods scans
in well over an hour on one core while the others idle. The library's facts stage is parallel
inside one run; the stages around it are not.

The app opens its inputs once and shares them: the game, the history index, the names and the
schema sit behind one `Arc`, and the store behind another, so several runs can read them at
once. What a run holds of its own is the mod's bytes and the game bins it reads, which is what
bounds how many run together.

A kept run would let a port resume after a scan, and was considered: the store held 3.4 GB for
a dozen written runs, so keeping a run per scanned mod costs more than it saves (the user,
2026-10-24).

## Decision

A batch runs its archives in waves of `jobs` at once (the user's call, 2026-10-24), each
archive of a wave in a thread of its own over the one set of inputs. The request says `jobs`;
0, the app's choice, is one per four cores, one at least and eight at most. A wave's rows reach
the window as each ends, in the order they end, and the batch's rows come back in the request's
order. The inputs are let go of and opened again as a wave starts past every sixteenth archive,
as before. A cancel ends the archives under way after their step and the batch after that
wave.

## Consequences

- A scan or a port batch of a library takes a fraction of the time, down to the slowest
  archive of each wave.
- Memory grows with the wave: eight large mods at once hold eight archives' bytes. The cap is
  a guess until measured; the request can ask for fewer.
- The progress line names the archive that started last, not the one running longest.
- The runs of a wave write to the store one at a time, as SQLite serializes them.
- Two packages of one name and version in one output folder are written at once, so a
  writer's staging folder is named by the process and the write, not by the package alone;
  the last of them into place is the one that stays, as when they ran one after another.
