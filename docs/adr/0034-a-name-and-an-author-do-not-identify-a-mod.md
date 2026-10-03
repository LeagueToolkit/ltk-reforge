# ADR-0034: A name and an author do not identify a mod

- **Status:** Accepted (2026-09-30)
- **Related:** #ADR-0033

## Context

A dataset holds earlier versions of a mod beside the latest, and the duplicates table merges
byte-identical archives only. A count over the datasets therefore counts some mods several
times. Grouping by the name and the author in the archive's metadata looks like the fix.

Names are generic. People port or fix another author's mod and publish it again, under the same
name or another.

## Decision

A name and an author do not identify a mod (the user, 2026-09-30, during ltk-forge). Archives
that share most of their own files' bytes are one family: versions, ports and fixes.

- The same name and author under another version number are versions of one mod.
- A member of a family with another author is a port or a fix.

## Consequences

- A statistic weighs a family once.
- For coverage, as in a reduced corpus, the versions are harmless.
- The namespace of a port is made from the name and the author (#ADR-0027), so two members of a
  family may share one.
