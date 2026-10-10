# ADR-0094: A run keeps a snapshot at each stage and resumes from one

- **Status:** Accepted (2026-10-10)
- **Related:** #ADR-0003, #ADR-0045, #ADR-0090, #ADR-0091, #ADR-0093, #ADR-0095, #ADR-0096,
  #ADR-0097, #ADR-0098;
  `docs/design/ARCHITECTURE.md`, "Shape" and "Report and dump"

## Context

A run is one-shot. The archive is taken out of the run when load starts, the loaded module is
moved out when fix starts, and fix rewrites the module in place. A stage's result is held as a
field of the run and read by the stages after it, and the report is written from them at the
end. So a run cannot be entered at a stage: #ADR-0090 makes each answer of the guided view a
new run from the archive, and the roadmap lists "progress inside a stage, and a way to stop
one" as lacking.

What a person wants of a run, in the guided and the advanced view, is to change a choice and
see what it changes, cheaply, and to look at the mod as it stood at any step: the files, a
bin, what a fixup moved, what a port renamed. Every stage is a function of the stage before it
and the settings (`AGENTS.md`, "Output is deterministic"), so the state at a stage boundary is
enough to run the rest again, and nothing between boundaries needs keeping.

The options were a copy of the whole module at each stage, held in memory; persistent data
structures through the model, so that a copy shares what did not change; or a snapshot at file
grain, the bytes in the blob store (#ADR-0093), with the parsed objects a working copy of the
stage.

## Decision

A run writes a snapshot at the end of each stage, and a run can start from a snapshot (the
user's call, 2026-10-10).

- A snapshot is a value: for each WAD, the map of path to content id; the extras of the
  archive by name, size and the id of their first bytes, since an extra is no game file and is
  read no further than its kind takes; the stage's part of the report (the tables and the findings
  as they stood); the inputs the stage was judged under: the install's build, the history
  index's head and the schema's build; the settings the run had; and the program that wrote
  it: the workspace version and the commit. It is immutable, and its size is the number of
  paths, not the bytes.
- A snapshot carries a format version, one number for the shape of every snapshot. It is
  raised when the shape changes, and a snapshot of another format is not read. The raw
  snapshot's reading has a version of its own (#ADR-0095).
- The snapshot is the authority. The parsed objects a stage works on are read from the
  snapshot's bytes and written back as bytes when the stage ends; they are a cache of the
  snapshot, never the other way round. A bin keeps the content id it was read from, and the
  id of what fix wrote is recorded beside the fixup's changes.
- The first snapshot, after read, is what the archive alone determines: its entries inflated,
  its WADs unpacked, each file hashed. It is the raw snapshot, keyed by the archive's SHA-256
  (#ADR-0095).
- Each stage declares what it reads: which inputs and which settings. A snapshot names the
  identity of exactly what the stages up to it declared, not of everything the run has. A run
  resumes from a snapshot when every one of those holds and the program is the one that wrote
  it; otherwise from the latest earlier snapshot that holds, and from the raw snapshot at
  worst, which depends on the archive alone. A change of a setting resumes from the earliest
  stage that declares it.
- The declarations put the snapshots in three groups, which the store keeps apart
  (#ADR-0096). The **raw** snapshot reads the archive. The **load** reads the install, the
  hash tables and the history, and of the settings only the mode, the file fixups, the stale
  keep and the reworks list. The **run** is every stage after, which reads the policy, the
  bin fixups and rules, the checks, the keeps (#ADR-0098) and the block. Nothing the guided
  view asks (#ADR-0090) is read by load, so every answer resumes from the load.
- A cancelled stage writes no snapshot, and the one before it stands (#ADR-0097).

## Consequences

- #ADR-0090's consequence that each run from an answer is a new run from the archive is
  superseded: a run from an answer resumes from the snapshot before the first stage the answer
  changes.
- The report names every input a stage declares. It gains the schema's build and the hash
  tables' identity: for the shared hash cache, the manifest's SHA-256 and version of each of
  the five tables a run opens, which `ltk_mimir_cache` reads; for a directory of text tables,
  a SHA-256 of each file read. The `port` part of the report gets a type of its own, since a
  snapshot carries it.
- A snapshot after fix costs a bin write per bin a fixup changed, which build does today; the
  bytes of the files no fixup touched are the raw snapshot's. The two copies of a mod's parsed
  objects the memory-only design would have held are not held.
- The app can show the mod at any stage from its snapshot, and the difference between two
  stages as the difference of two maps. The viewers of a file's bytes are #ADR-0091's.
- Build and check read a snapshot and write one; they do not read the run's earlier fields. The
  stages' fields on the run go, and `Run::through(stage)` becomes a fold over snapshots. Each
  stage's change is one rule with its test (#ADR-0045), and the reports over the reduced corpus
  must equal the baseline's at every step.
