# ADR-0058: The app downloads the hash tables into the shared cache

- **Status:** Superseded by #ADR-0059
- **Related:** #ADR-0046; supersedes #ADR-0048; `docs/design/ROADMAP-UX.md`, "The inputs"

## Context

#ADR-0048 has the app read the hash tables on the machine, LTK Manager's first, and fetch none
of its own. A machine without LTK Manager has no tables, and a run on it names less: a chunk
no table spells ships under its hash, and a patch cannot name a field no table spells.

The tables LTK Manager keeps are Mimir's: one cache per machine, shared by every League
Toolkit program, in `%LOCALAPPDATA%\LeagueToolkit\hashes` on Windows. Mimir's own crate holds
the updater. It compares the cache with the newest release of `LeagueToolkit/mimir`, downloads
the tables that differ, checks each against its SHA-256, and installs them with the cache's
list of tables swapped last, under a lock that keeps a second updater out. It takes any client
to download with. The newest release held 8 tables, 65 MB, on 2026-10-05.

## Decision

The user, 2026-10-05: the app downloads and updates the hash tables, for a machine where LTK
Manager does not keep them. It runs Mimir's updater over its own web client, into the cache the
settings choose, else the one LTK Manager keeps, when the user asks. It reads the tables on the
machine first, as before, and downloads none unasked.

## Consequences

- A table the app downloads is LTK Manager's too, and the reverse: the machine holds one copy.
- While another program updates the cache, the app's update is refused as busy, and the
  reverse.
- The inputs page reads the release's list of tables to say which ones are newer: one small
  request to GitHub, which answers sixty an hour to a client that does not sign in.
- A run already holding the tables keeps them. The inputs open again after an update. On
  Windows a table file still open stays on disk until a later update removes it.
- The schema database, which LTK Manager keeps too, is not downloaded by the app.
