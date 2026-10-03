# ADR-0035: The census format is closed

- **Status:** Accepted (2026-10-03)
- **Related:** #ADR-0005

## Context

The census history is the source of the history index. During ltk-forge its format grew at
ltk-forge's request: a normalised object key, what each entry names, rig and texture facts, the
audio structure. Each request cost a new export of 755 builds and a rebuild of the index. The
history is published and other tools read it.

## Decision

The census format is closed at format 2 (the user's call, 2026-10-03, during ltk-forge).

- reforge adapts to what the history carries.
- A request to the census is for a fault in it, not for reforge's convenience.
- The history is checked against the game's bytes and against itself.

## Consequences

- A fact the history does not carry is read off an entry's text, or is not known.
- Builds of the test realm are not in the history: bytes that shipped only there pass as a mod's
  own.
