# ADR-0060: The app bootstraps its census clone from a released archive when asked

- **Status:** Accepted (2026-10-05)
- **Related:** #ADR-0047, #ADR-0057, #ADR-0059; `docs/design/ROADMAP-UX.md`, "The history
  index"; `docs/design/HISTORY.md`, "Fetching"

## Context

#ADR-0047 has the census history cloned whole by the app at the user's choice. #ADR-0057 has the
app's clone hold only what an append reads, so a whole build from it, or an entry's text at a
build before the head, stops at an object it lacks. A clone from GitHub takes 12.3 million
objects, which GitHub serves compressed with zlib: the mirror clone of 2026-10-05 held one pack
of 10.06 GiB.

The release `history-v2-snapshot-0` (2026-10-05) holds, beside the index, the census history as
a bare repository, `ltk-census-history.git.tar.zst`, of 1,771,553,727 bytes. It holds one pack of
the 10,986,063 objects of `history-v2`, `main` and the tag before it, packed by git with
`--path-walk`, a window of 1000 and a depth of 50 and stored without zlib, with its index,
its reverse index and a `.keep`; then tar, then zstd `--ultra -22 --long=31`. Its `history-v2` is
the index's head, 16.19.8230722. The index's database is `history.sqlite.zst` there, 766,522,480
bytes; the release before held it as `.xz`, 890,971,876 bytes. Packs of the same objects in other
orders were up to 9% smaller as packs and within 1.6% of each other after zstd
(`docs/design/RUNS.md`, "Packing the census history").

Unpacking a `--long=31` frame holds a window of 2 GiB. GitHub serves a release's file in ranges
(`206 Partial Content`) and ignores `If-Range` (checked 2026-10-05): a download can resume, and
whether the file is still the same one is the client's to check.

## Decision

The user, 2026-10-05:

- **The archive bootstraps a whole clone, when the user asks.** The app downloads the newest
  release's archive and makes a bare clone of its pack and its `history-v2` commit, in a folder
  of its own. Until there is one, updates fetch into a clone of their own as #ADR-0057 has them.
  Once there is one, updates fetch into it, and the fetched clone is removed. Asked again, the
  app makes the whole clone again from the newest archive, in place of the one it holds.
- **Only the pack is taken from the archive.** The archive is read as a stream. The pack's
  files, `pack-<hash>.{pack,idx,rev,keep}`, are written under names the app builds from the
  hash, and `packed-refs` gives the branch's commit. No path of the archive reaches the disk,
  and no other member is written: the clone's config and refs are the app's own.
- **Both downloads resume.** The index's database and the archive download into a file of the
  app's data folder, which is kept when the download stops. The next download asks for the rest
  when the file's ETag and size are the ones it started with, and starts again otherwise. The
  whole file is checked against the SHA-256 GitHub lists, and unpacked after.
- **The window stays 2 GiB.** The app unpacks frames written with up to `--long=31`.

## Consequences

- A bootstrap downloads 1.77 GB and writes 5.4 GB. Both are on disk until the clone is in
  place, and the download is removed then. From the published release it took 191 s on the
  user's machine, 2.5 minutes of it the download (2026-10-05).
- The clone holds every object of the history, so a whole build and an entry's text at any
  build can be read from it. Neither is built.
- An update after a bootstrap fetches the commits since the archive's head and what the append
  reads of them that the clone lacks.
- The clone's pack stays the release's: gix repacks nothing, and the `.keep` holds the pack
  against a `git gc` run by hand.
- An index install holds the compressed database and the unpacked one on disk at once, 0.8 GB
  and 2.75 GB at 16.19.
- Unpacking holds up to 2 GiB of memory.
