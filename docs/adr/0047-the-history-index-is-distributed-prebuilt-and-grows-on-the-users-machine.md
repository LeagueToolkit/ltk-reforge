# ADR-0047: The history index is distributed prebuilt and grows on the user's machine

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0005, #ADR-0036, #ADR-0042, #ADR-0043, #ADR-0046; `docs/design/HISTORY.md`

## Context

A run asks the history index whether the game ever shipped a file, an object or a clip, and
when. Without an index a run is judged against the install alone, which the design allows and
reports (`history.absent`): a stale copy and a mod's date are then judged with less evidence.

The index is built from the census history, a git repository of some 5 GB. A whole build takes
about 38 minutes and gives a SQLite file of 2.75 GB (`docs/design/RUNS.md`, "Clip joints and
events banks in the history index"). That file compresses to about 880 MB with xz at its
default level (the user's measurement), and zstd is expected to give about the same.

The index holds no bin values. An object of the game as a build held it is an entry text in
the census history, which a run without an install (#ADR-0043) and a comparison with the game
at the mod's own build need.

## Decision

The user, 2026-10-04:

- **A run without an index is acceptable** for a first release. The app says what is judged
  with less.
- **A prebuilt index is distributed**, compressed, as a file of a GitHub release. The app
  fetches it and unpacks it into place.
- **A user's index takes further builds.** The app adds the builds after the index's head, as
  `reforge history index --append` does, from the census history.
- **The census history can be fetched by the app**, at the user's choice, as a clone on the
  machine. A user with a clone needs no prebuilt index and no later download: the app builds
  and appends from the clone.
- **A bin entry's text is read from the census history when a run asks for it.** With a clone
  on the machine it is read there. Without one it is fetched as it is needed.

## Consequences

- The index's format is fixed for a released app: #ADR-0036 holds only before a release. An
  app refuses an index of another format by its stamp, as the reader does, and fetches the one
  made for it. A change of format after a release is a new download or a migration, and that
  choice is taken then.
- A distributed index's stamp names where it was built from. A stamp made on a build machine
  holds that machine's path to the clone. A distributed index names the published repository.
- Who builds the released index, and when a new one is published, is not settled. An index
  that lags the game is taken forward by the append.
- The append needs the commits after the index's head. Fetching only those, with no full
  clone, is a shallow or partial fetch of the census repository, and is not built.
- Fetching one entry text without a clone is not built, and how it is done is not settled: a
  partial clone that fetches blobs on demand, or the repository host's file interface.
- `reforge` asks the history questions and does not walk the census (#ADR-0005). The fetch,
  the unpacking and the append are the app's, through `reforge-census`.
- A download of 880 MB and a clone of 5 GB are both optional. An app with neither still
  repairs and ports, against the install alone.
