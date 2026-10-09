# ADR-0061: The app removes what it downloaded when asked, and nothing it shares

- **Status:** Accepted (2026-10-05)
- **Related:** #ADR-0047, #ADR-0057, #ADR-0059, #ADR-0060; `docs/design/ROADMAP-UX.md`, "The
  inputs page"

## Context

The app writes three things to its data folder for the history: the indexes it installs
(#ADR-0047), its clones of the census history (#ADR-0057, #ADR-0060) and the downloads a stop
keeps (#ADR-0060). At 16.19 an index is 2.75 GB and a whole clone 5.4 GB. An index the settings
no longer name is removed at the next start only while the settings name another of the app's.
With the settings naming none, or a folder of the user's, the app's indexes stay. A clone and a
kept download are removed only by the work that replaces them. The inputs page had no way to
free any of it.

The user asked, 2026-10-05, for a way to clear specific things from the inputs page. Which
things, and what the app must never remove, was open. The hash cache and the schema database are
shared with the other League Toolkit programs (#ADR-0059), and an input can be a folder or a file
the user chose anywhere on the machine.

## Decision

The app removes, each when the user asks for it and confirms, the three things it wrote to its
own data folder, and nothing else.

- **The indexes.** Every index folder under the app's own goes. When the settings name one of
  them, they name none after. An index in a folder the user chose is not removed: "Use none"
  stops the runs reading it and leaves it on disk.
- **The clones.** The whole clone, the clone fetched for updates, and what a bootstrap that
  stopped left beside them.
- **The downloads kept.** The whole folder.
- **Never the shared caches.** The hash cache and the schema database are not removed by the
  app: another program reads them.
- **One at a time with the downloads.** A removal is work on the history as a download is, and
  neither starts while the other runs.
- **An index kept and not used is offered again.** With the settings naming none, the page
  names the app's index with the most builds and offers to use it or remove it.

The page shows the bytes each of the three holds, so a removal says what it frees.

## Consequences

- Removing the indexes costs a download of the release to have one again, 767 MB at 16.19.
- Removing the clones costs nothing a run reads. The next update of the index fetches what it
  reads into a new clone, and a whole clone is a bootstrap again.
- An index a run holds open does not go on Windows. The removal reports the folder, and the
  settings name none by then.
- The shared caches are cleared in the program that owns them, or by hand.
