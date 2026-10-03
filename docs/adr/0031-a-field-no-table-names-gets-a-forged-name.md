# ADR-0031: A field no table names gets a forged name

- **Status:** Accepted (2026-10-01)
- **Related:** #ADR-0026, #ADR-0029

## Context

A PTCH record names a field by a path of plain names, and cannot name one by its hash
(#ADR-0026). The names come from the hash tables. A field no table names cannot be written: a
record stops above it, or the object is left out.

The client hashes each name of a record's path, lowercased, with FNV-1a. Any string with the
field's hash names it. The FNV prime is odd, so a step of the hash inverts mod 2^32, and a
suffix can be found that takes a chosen prefix to a chosen hash: six or seven characters of
`[a-z0-9_]` reach any hash.

## Decision

A field no table names gets a name that hashes to it: `_unk_<hash:08x>_<suffix>` (the user's
call, 2026-10-01, during ltk-forge). The suffix is the shortest, then the smallest in bytes, so
the name depends on the hash alone.

- A forged name is never written to a port's hash tables and never learnt as a name.
- A field a tool wrote and no build read is still removed: a name makes a field writable, not
  worth carrying.

## Consequences

- A record can go to the deepest changed field on any path.
- Not built in ltk-forge, and not confirmed in game: one record through a forged name goes to
  the game before the rest is built.
