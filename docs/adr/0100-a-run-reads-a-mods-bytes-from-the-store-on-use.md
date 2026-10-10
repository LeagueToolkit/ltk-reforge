# ADR-0100: A run reads a mod's bytes from the store on use

- **Status:** Accepted (2026-10-10)
- **Related:** #ADR-0051, #ADR-0093, #ADR-0094, #ADR-0095, #ADR-0096; `docs/design/ARCHITECTURE.md`,
  "Shape" and "Untrusted input"; `docs/design/RUNS.md`, "The raw snapshot as a cache"

## Context

#ADR-0095 made the raw snapshot a cache so that a corpus run would skip inflating and
unpacking each archive. Measured over the reduced corpus (2026-10-10, "The raw snapshot as a
cache"), a run that finds every mod in the cache takes the time of a run with no store, and
populating the cache doubles the summed time: the cache holds the raw bytes, 3.4 times the
archive's, and a hit reads and verifies every one of them where a plain run reads the
archive and inflates it. The saving that record expected does not exist.

The run's memory is the problem the store can solve. A run reads the whole archive into
memory, inflates each WAD from its zip entry as one slice, unpacks every chunk into memory,
and holds every file's bytes on its node from `load` to `write`. One mod of 256 MB on disk
with three packed WADs, one a map's, inflates to several times that before a stage runs, and
a mod is sometimes bloated on purpose. The app runs on machines with 8 GB, beside the game's
files and the history index.

What a stage reads of a mod's bytes is a part of them: `load` parses the bins, `facts` reads
the meshes, skeletons and clips it judges, `build` reads the files it transforms or judges, and
a carried file is named by its id and never read. A run that reads bytes on use holds the
parsed bins and what the stage touches.

## Decision

The store bounds a run's memory, not its time (the user's call, 2026-10-10). A run reads a
mod's bytes from the store when a stage asks for them, and `read` streams them into the store.

- **Read streams from the file.** The archive is read from its file, entry by entry, never
  as one slice. A stored entry is a window of the file, and a packed WAD in one is mounted
  over the window. A deflated entry that holds a WAD is inflated once onto disk, into the
  store's directory as a file hashed as it is written and renamed to its content id, and
  mounted from there. Each chunk is unpacked into the store and dropped. The peak of `read`
  is one chunk and the reader's window.
- **A file node holds an id, not bytes.** Its bytes come from the store the first time a
  stage asks, unchecked inside the run's own layer, and the node does not keep them past the
  stage unless the stage does. A carried file is never read.
- **The caps bound memory too** (#ADR-0051): a cap on one chunk and a cap on the working set,
  with the note a passed cap raises today.
- **Facts per blob are kept beside it, stamped.** The size and the SHA-256, which never
  change, carry no stamp. The kind, which the magic matcher decides, carries the matcher's
  generation: a constant in `reforge-magic` raised when its rules change. A blob's facts of
  another generation are recomputed on the next read and never trusted. The facts are a
  table in the ingest directory, not the blob store's index. More that is derived per blob and
  stable per generation may join later.
- **Memory-bounded means store-backed.** Without a disk store there is nowhere to stream to,
  and the command's memory-only run keeps today's peak. The app always has its ingest tier
  open, and the command's `--store` is what a person with a large mod passes.

## Consequences

- #ADR-0095's rationale, the corpus skipping read and unpack for time, is superseded by this
  record. The cache stays as the ingest tier of #ADR-0093 and the corpus may use it, but a
  corpus run is not expected to be faster for it.
- The measure of this record is peak memory, not wall time: a run of one mod of 256 MB with
  and without a store, and the bytes each stage reads over the reduced corpus, in the runs
  log.
- A resume after a late stage reads back only what the stage touches, which is what makes
  the guided view's reruns cheap (#ADR-0090, #ADR-0094).
- The second run of one mod through `check`, with its bytes and facts in the store, is the
  iteration a person waits on, and its time is measured beside the memory.
- Per-kind compression dictionaries and compression at rest are not part of this record;
  disk reads and CPU are not the bound once bytes are read on use.
