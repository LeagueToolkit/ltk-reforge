# ADR-0080: The install answers what the target build holds, and the index what builds held

- **Status:** Accepted (2026-10-08)
- **Related:** #ADR-0043, #ADR-0042, #ADR-0078, #ADR-0079; `docs/design/ARCHITECTURE.md`, "The
  environment"; `docs/design/HISTORY.md`

## Context

#ADR-0043 had the stages after `load` ask the game typed questions in place of reading a
chunk's bytes, and planned a second source of their answers backed by the history: a run with
no install, judged against the game at any build the history holds.

That source was built and run over the reduced corpus (#ADR-0079). The index holds what an
install holds in another form, and not all of it: no vertex positions, every locale's WADs
where an install holds one, a bin as its objects and links with no file's bytes. A run against
it needed a locale chosen, reported the copied-submesh check as not judged, and wrote bins in
another order than a run against the install.

The user decided that the target install stays required (2026-10-08).

## Decision

A run is judged against the target install. The install answers what the target build holds,
and the history index answers what builds held.

- The stages after `load` ask the game typed questions (`GameSource`), and no question gives a
  chunk's bytes: the skins a character has, a bin, a skeleton, a mesh's submeshes and vertex
  positions, the joints a clip animates, an events bank, a bank's clips, and whether the game
  holds a file's bytes at a path or at another path of a champion's WAD. `GameView` answers each
  from the install.
- No game source answers from the history. What a build before the target held, the game at
  the mod's own build among it, is asked of the index (`History`).
- Two comparisons are kept apart. Where a unit goes (`facts`, `decide`) is judged against the
  target build: which skin its work derives from, and whether it fits a slot's rig as the game
  holds it. What the author changed (`plan`, `build`) is the mod against the game at the mod's
  own build, applied onto the target build.

## Consequences

- No code of `facts` or `port` reads a game chunk's bytes: the game they reach is a
  `GameSource`, and a test holds them to it.
- Every typed question is answered from the install, so no check is reported as not judged for
  want of a fact.
- The index stores no vertex positions: nothing asks what a mesh's positions were at an earlier
  build.
- The index holds every locale's WADs, and a run takes the install's. No locale is chosen.
- `reforge port`, `repair`, `inspect` and `load` need `--game`. #ADR-0079 is withdrawn.
