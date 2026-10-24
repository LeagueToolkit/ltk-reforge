# ADR-0109: The index is taken forward by a published delta

- **Status:** Accepted (2026-10-25)
- **Related:** #ADR-0047, #ADR-0057, #ADR-0078; `docs/design/HISTORY.md`, "Deltas"

## Context

#ADR-0057 takes a user's index forward on the user's machine: the app fetches from the census
history what the builds after the index's head changed, and appends them. On a fresh install
with the published snapshot at 16.19 and the game at 16.21, that took eight minutes on a fast
machine and connection (2026-10-25): the fetch asks for tens of thousands of trees in rounds,
and between rounds the clone is asked about each object it may hold, and the append then
parses every changed file. Every user does this work again, for the same two builds.

An append from the snapshot to the tip is the same on every machine, so its result can be
computed once. An append rewrites pages of the SQLite file in place and adds pages at its end,
so the file taken forward lines up with the snapshot's chunk for chunk, and zstd's patch mode,
one chunk of the new file compressed against the same chunk of the old, is small: the two
builds after the 16.19 snapshot patched to 220 MB against a 4.9 GB database, written in 27 s
and applied in 7 s with the digest checked (2026-10-25). A diff of rows would be no smaller,
since most of the new bytes are object blobs, and would need the index's schema on both
sides.

A snapshot is cut every few patches (#ADR-0078's releases), and the builds between snapshots
are few.

## Decision

A delta is published beside the snapshot it starts from: the effect of the append from the
snapshot's head to the history's tip, computed once (the user's call, 2026-10-25). The app takes
the index forward by downloading the delta and applying it to a copy of the snapshot, and
fetches from the census history only when no delta covers its snapshot. Maintainers keep the
census path behind the "For maintainers" fold.

- **What a delta holds.** A tar: `delta.json` (the delta's format, the base's head, the new
  head, how many builds the delta adds, the new database's size and SHA-256, and the chunk
  size), the new `builds.tsv` and `stamp.json` whole, then one zstd frame per 512 MiB chunk
  of the new database, each compressed with the same chunk of the base loaded as its
  dictionary, a window of one GiB and long-distance matching, as `zstd --patch-from` does.
- **How it is made.** `reforge history delta --base <snapshot> --index <head> --out <file>`,
  from the snapshot and the index appended from it, the append done as before.
- **How it is applied.** `reforge history apply` and the app decode each frame with the
  base's chunk and write the database, check its digest against the note, and write the
  builds and the stamp. The result is the appended index byte for byte. The base's stamp must
  name the delta's base head, else the delta is refused.
- **How it is found.** The delta's asset in the snapshot's release is named
  `history-v<format>-delta-<head12>.tar.zst`, the head being the first twelve digits of the
  new head's commit. The app's check for updates reads the release of the snapshot the index
  came from, and offers the delta when its head is not the index's. An index taken forward is
  named `head-<head12>-v<format>` as before.
- **Cumulative, from the snapshot.** Each delta starts at the snapshot, not at the delta
  before, so one file brings any user of that snapshot to the tip, and the publisher keeps one
  current file per snapshot. The snapshot stays on disk as the base of the next delta.

## Consequences

- An update is a download of the delta and a decode against the snapshot, with no parsing on
  the user's machine, and the snapshot's database mapped, not copied.
- The publisher runs the append from the snapshot to the tip at every patch, which grows with
  the builds since the snapshot until the next snapshot is cut. A snapshot every few patches
  bounds it.
- Two indexes sit on disk, the snapshot and the head taken from it, 4.8 GB and more each at
  format 1. The previous head is removed when a new one is made.
- A delta applies only to the snapshot it names; a user who built an index of their own, or
  who is on an older snapshot, gets the census path as before.
- The `zstd` crate gains the `experimental` feature, for the parameter that loads a
  dictionary whole, and the workspace gains `memmap2`.
