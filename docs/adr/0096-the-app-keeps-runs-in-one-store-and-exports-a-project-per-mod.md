# ADR-0096: The app keeps runs in one store and exports a project per mod

- **Status:** Accepted (2026-10-10)
- **Related:** #ADR-0041, #ADR-0046, #ADR-0091, #ADR-0093, #ADR-0094;
  `docs/design/ROADMAP-UX.md`, "The second version"

## Context

#ADR-0091 decides a Runs page: every run the app made, kept in its data folder and reopened,
two runs of one mod compared, a package's embedded evidence read as a run. Its consequence
bounds the folder's size and removes the oldest first. With the blob store (#ADR-0093) and the
snapshots (#ADR-0094), a kept run is a small value naming bytes the store holds, and the
question is how many stores there are and what a person can carry away.

The options were a file per mod, holding its runs and its bytes; one store for the app,
holding every run's snapshots and every blob once; or both, one derived from the other.

## Decision

The app keeps every run in one store, and exports a project per mod on request (the user's
call, 2026-10-10).

- The store keeps three kinds of thing, after #ADR-0094's groups, each referring to the one
  before it by key and never holding it twice:
  - a **raw** snapshot, keyed by the archive's SHA-256 and the raw reading's version
    (#ADR-0095), with the path the archive was read from;
  - a **load**: the snapshot after load, keyed by its raw snapshot, the program's commit, the
    install's build, the hash tables' identity (#ADR-0094), the history index's head and the
    settings load declares. Every run of the mod under those shares it;
  - a **run**: the snapshot of each stage after load that was reached, keyed by its load; the
    whole settings; the schema's build; the program's version and commit; the path of the
    package written, when one was. It is written when the run ends.
- The three are rows of a SQLite file in the app's data folder, with the mod's name, author
  and version beside each raw snapshot so that the Runs page lists mods without opening a
  snapshot; the bytes they name are in the disk store beside it (#ADR-0093). The two have
  one format version between them, and a store of another format is opened read-only or
  rebuilt, never migrated in place.
- The store is one per app, in its data folder. Two runs of one load share it and every blob
  it names; two mods that share files share those.
- A **project** is a run with its load, its raw snapshot and the bytes they name, as one
  file. Exporting one is a query: the content ids the three name, and a copy of each blob.
  Opening one is a layer over the app's store (#ADR-0093), and importing it is the copy the
  other way.
- Sweeping is by reference. The store numbers runs in the order written, and a run is removed
  lowest number first when the store passes its bound; a run a person marked kept is not. The
  time a run was made is the store's, shown by the app, and never part of a report.
- A person removes a run, every run of a mod, or every run, and unmarks a kept run. A removal
  drops the run and sweeps what no remaining snapshot names: its load and its raw snapshot
  when no other run refers to them, and the blobs.
- The sweep marks every id the remaining snapshots and an open run's name, then deletes what
  is unmarked, in one SQLite transaction for the rows and a delete per blob after it, which
  gives the space back at once. It runs after a removal and after the bound cuts, never while
  a run is flushing. The settings page shows the store's size. A load stays while a run refers to it, a raw
  snapshot while a load does, and a blob while any snapshot names it.
- Reopening a run resumes it when its archive is at its path with its SHA-256, the program is
  the one that wrote it and the inputs match (#ADR-0094). Otherwise the run opens read-only
  from its snapshots, which the page can show and compare but not continue, and a new run of
  the archive starts from its raw snapshot.
- A package's embedded evidence (#ADR-0041) opens as a kept run with no snapshots: its report
  only, read-only.

## Consequences

- The data folder holds one store, bounded, and the baselines of what it holds are a query
  over it: how many runs, how many blobs, how many bytes, how much the sharing saved.
- The Runs page, the comparison of two runs and the project file are each a reader of kept
  runs, and add no rule about a mod (#ADR-0046).
- A project carries a mod's bytes, so it is handled as the mod is: opened under the caps, its
  names shown through `clean`, never extracted under its names.
- The command does not keep runs. A command run writes its report and its package as today.
- The corpus cache (#ADR-0095) is swept the same way by the scripts, the raw reading's version
  as the mark.
