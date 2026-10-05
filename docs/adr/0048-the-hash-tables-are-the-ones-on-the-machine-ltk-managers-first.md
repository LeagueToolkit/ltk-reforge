# ADR-0048: The hash tables are the ones on the machine, LTK Manager's first

- **Status:** Superseded by #ADR-0058
- **Related:** #ADR-0046; `docs/design/ARCHITECTURE.md`, "The environment"

## Context

A run spells path, entry and field hashes from tables: the hash cache LTK Manager keeps and
updates on the machine, and CommunityDragon's text tables. The command reads the cache where
LTK Manager keeps it, and takes other places as arguments. The same holds for the schema
database of field types, which LTK Manager also caches.

A standalone app (#ADR-0046) could fetch and update tables of its own, as LTK Manager does,
or read the ones already on the machine.

## Decision

The app uses the hash tables on the machine, and looks for the ones LTK Manager installs and
keeps up to date first (the user, 2026-10-04). It fetches none of its own in a first release.

## Consequences

- A machine with LTK Manager needs nothing more. The tables are as fresh as LTK Manager keeps
  them.
- A machine without LTK Manager has no tables until the user points the app at some. A run
  with no tables works and names less: a chunk no table spells ships under its hash
  (#MOD-ARTIFACT-0007), and a patch cannot name a field no table spells (#ADR-0031).
- The app says which tables it read and how old they are, since a report differs with them.
- Fetching and updating tables without LTK Manager is left for a later release.
