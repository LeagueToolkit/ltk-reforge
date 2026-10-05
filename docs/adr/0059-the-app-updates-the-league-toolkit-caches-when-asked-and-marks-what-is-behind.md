# ADR-0059: The app updates the League Toolkit caches when asked and marks what is behind

- **Status:** Accepted (2026-10-05)
- **Related:** #ADR-0046; #ADR-0047; #ADR-0057; supersedes #ADR-0058;
  `docs/design/ROADMAP-UX.md`, "The inputs"

## Context

#ADR-0058 has the app download the hash tables, when asked, into the cache it names as LTK
Manager's, and leaves the schema database to LTK Manager. Neither folder is LTK Manager's.
`LeagueToolkit` under the platform's data folder holds the data of every League Toolkit
program, this one among them: `hashes` holds Mimir's hash cache, and `meta` beside it the
schema database.

LTK Manager keeps both up to date (its `hashtables` and `meta_schema::cache` modules, read
2026-10-05):

- The hash tables with Mimir's updater, as #ADR-0058 has the app do. A check reads the
  release's list of tables, takes no lock, and gives up after 20 s.
- The schema database from the LTK Meta Wiki, `https://meta-api.leaguetoolkit.dev/v1/db`, as
  served, byte for byte, in `meta/meta-schema.json`. Beside it, `meta/meta-schema.stamp.json`
  holds the entity tag it was served under and the database's generation. LTK Manager asks
  with that tag, and installs a body only when it parses, written beside and renamed into
  place.
- It updates both at its start, and again with one button.

On 2026-10-05 the published database was 3,865,780 bytes, served with a tag. A HEAD that sends
the current tag is answered 304. The copy on this machine, which LTK Manager installed on
2026-10-04, was behind it.

## Decision

The user, 2026-10-05: the app treats the `LeagueToolkit` folders as its own, as every League
Toolkit program does, and updates the hash tables and the schema database when the user asks,
as LTK Manager does. It downloads nothing unasked. It checks at its start instead, and marks
an input a download would bring up to date with a warning triangle that names what is behind
when hovered: beside Inputs in the list of pages, on the run page as a link to the inputs page,
and on the input's row there.

- The hash tables: Mimir's updater, as #ADR-0058 has it, into the cache the settings choose,
  else the shared one.
- The schema database: the publisher's copy, into the shared `meta` folder, in LTK Manager's
  files, so either program's copy is the other's. A check is a HEAD with the stamp's tag and
  reads no body. A database the app does not read is not installed, and the copy before it
  stays. A file the user chose is the user's, and the app does not update it.
- Marked: a hash table absent, or older than the release's; the schema database absent, or
  another than the one published; an index whose head is behind the census history
  (#ADR-0057). An absent index is not marked: it is a download of some 880 MB, offered before a
  run (#ADR-0047).

## Consequences

- The machine holds one copy of each: what the app downloads, LTK Manager reads, and the
  reverse.
- The start costs three requests: the release's list of tables, a HEAD to the wiki, and the
  census history's tip. A check that fails or gives up marks nothing.
- When the wiki publishes a database the app does not read, the app keeps its copy, and the
  mark stays until a newer app reads it.
- A run already holding the inputs keeps them. They open again after an update.
