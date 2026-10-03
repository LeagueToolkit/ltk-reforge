# ADR-0002: Every loaded object is fixed

- **Status:** Accepted (2026-10-03)
- **Related:** `docs/design/REVIEW.md`, "Stage boundaries" and "Fixups";
  `docs/design/ARCHITECTURE.md`, "Fixups"

## Context

The fixups bring a mod's bin values to the types the target build reads. What a port leaves out
(copies, the game's own skins, objects no entry reaches) is found by comparing the mod's values
with the game's, and that comparison holds only at the same types. So the fixups run before the
facts are gathered, and before anything is left out.

A mod loads more than a port writes: a whole-WAD repack ships the champion's gameplay data, and
an old skin bin links the root bin. ltk-forge marks the objects a port may write before its
repairs run (`scope`), repairs only those, and checks later that the port carries nothing left
unmarked. That is a second reachability walk that has to agree with the first. Without it,
ltk-forge counted 18,842 lost fields over 281 mods against 1,117.

The options:

- fix every object the client loads, and list in the report only the changes on what ships;
- mark a set before the fixups, as ltk-forge does;
- fix after the discard, which the order above rules out for all but byte-identical copies.

The user left the default to the implementation and asked that neither way be locked in
(2026-10-03).

## Decision

The fixups run on every object the client loads. The report lists a change when the object it
is on ships.

The engine takes the set of objects as an argument. Narrowing the set is a change at the one
call that passes it.

## Consequences

- Nothing unrepaired can reach a port, so there is no check for it.
- Gameplay data that never ships is fixed for nothing. The cost is measured over the corpus
  when the fixups are built, and the set is narrowed if it matters.
- A run that stops before the plan has no list of what ships. It lists every change, grouped by
  whether a skin unit's entry reaches the object.
- `repair` writes every loaded bin and lists every change.
