# ADR-0095: The raw snapshot is a cache the corpus reads

- **Status:** Accepted (2026-10-10)
- **Related:** #ADR-0044, #ADR-0093, #ADR-0094; `docs/design/RUNS.md`

## Context

A corpus run (#ADR-0044) reads every archive from its `.fantome`: the zip inflated, each WAD's
zstd chunks unpacked, every file hashed. That part is a function of the archive's bytes alone,
and it is the same in every run over the corpus whatever changed in the program. The rest of
load and every stage after it depend on the inputs and the program, and are what a corpus run
measures.

The raw snapshot (#ADR-0094) is exactly that part, keyed by the archive's SHA-256, with its
bytes in a blob store (#ADR-0093). Over the datasets' 4,909 mods the raw bytes are 235.1 GB,
and 130.4 GB kept once per distinct content (#ADR-0093).

## Decision

The raw snapshot is a cache the corpus scripts may populate once and read in every run after
(the user's call, 2026-10-10).

- The cache is a blob store with a table of archive SHA-256 to raw snapshot. A run that finds
  its archive's snapshot there skips read and the unpacking half of load; one that does not
  reads the archive as today, and writes the snapshot when asked to.
- A cache entry's key is the archive's SHA-256 and the raw reading's version: one number,
  raised when the zip or WAD reading, the hashing or the raw snapshot's shape changes, so that
  a fix to the reader is a miss and not a stale hit. No build, no index, no setting and no
  program commit is part of the key, because none is part of what the raw snapshot holds.
- The cache is invisible in the output. A corpus run with the cache and one without give equal
  reports for every mod. This is checked by comparing the two runs as a change is compared
  with a baseline, and the check is run once when the cache lands and once whenever the raw
  snapshot's shape changes.
- The cache is the plain-files store of #ADR-0093, since the runner is 24 processes and
  `fjall` opens for one process at a time. A blob written by a rename is whole or absent, so
  the processes populate and read it at once with no lock. The table of archive SHA-256 to
  raw snapshot is a file per archive beside the blobs, written the same way.

## Consequences

- A corpus run starts at the dating and naming half of load, and the time to read and unpack
  comes off every run. The saving is measured when the cache lands and recorded in
  `docs/design/RUNS.md`.
- Two archives with one raw snapshot are one mod repacked, and the corpus scripts can list them
  before a run.
- The cache is gitignored under `data/corpus/` as the baselines are. A raised reading version
  means the entries of the old one are misses and are swept, not migrated.
