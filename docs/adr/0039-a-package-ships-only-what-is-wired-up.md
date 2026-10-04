# ADR-0039: A package ships only what is wired up

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0011, #ADR-0002, #ADR-0004, #MOD-ARTIFACT-0004, #GAME-CHANGE-0009;
  `docs/design/ARCHITECTURE.md`, "Package"

## Context

A mod archive carries more than the client uses. Beside the files a skin loads it holds bins at
paths the game dropped, which no client loads, files nothing names, and copies of game files as
an earlier build shipped them (#MOD-ARTIFACT-0004).

ltk-forge's repair writes every file but the ones nothing uses. Over the datasets (2026-10-04)
that ships bins at a former path in 98 mods whose values no fixup has brought to the target
build, since no fixup reads a bin the client does not load.

Two aims of the rewrite decide what a package may hold. A package is to stay working as the
game changes, which is what the package format is for. And a package is to be safe by
construction: everything in it is there because something reads it, and that reading can be
followed through a bin or through a patch of a bin.

## Decision

A package holds a file only when the client reaches it:

- a bin the client loads for a unit the package carries;
- a file a loaded bin names, in the mod's bin or in a patch the package writes;
- a file at a path the game's own bins read, which the mod's file overrides;
- a file a run makes for one of those.

A file the client does not reach is left out, by a repair and by a port alike: a bin at a
former path that nothing loads, a file nothing names, and a stale copy unless the caller asks
to keep stale copies.

Every file left out is counted in a diagnostic of the package, by the reason it is left out.
None is dropped without a line.

## Consequences

- A repaired mod is smaller than the archive it came from, and holds no bin the fixups have
  not seen.
- The type check after a repair no longer reports values in bins nothing loads.
- A repair departs from ltk-forge here (#ADR-0004). The comparison lists it as an accepted
  deviation with the count of mods and files.
- What "reaches" means is the load stage's file use. A use that is judged wrong there drops a
  file the client needed, so a mistake shows as a missing file in game, not as extra bytes.
