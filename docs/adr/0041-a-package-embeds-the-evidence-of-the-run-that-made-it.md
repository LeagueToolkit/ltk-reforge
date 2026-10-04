# ADR-0041: A package embeds the evidence of the run that made it

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0039, #ADR-0011, #GAME-CHANGE-0001; `docs/design/ARCHITECTURE.md`,
  "Package" and "Report and dump"

## Context

A run judges a mod by everything its archive holds. A package holds less: only what the client
reaches (#ADR-0039). What stays behind was part of the evidence. Over the datasets (2026-10-04)
a repaired package judged from its own bytes alone dates at another build than its archive did
in 2,454 of 5,137 mods, because a fixed object equals the game's own at the target build. In 4
of the 43 mods that restore a model from before a rework, the files that showed the restoration
are left behind, and a second run would judge the old model's files stale and drop them.

So a package is not a mod archive that can be taken through a run again as if it were the
original. A later repair, a port of a repaired mod, or a move to a later build needs what the
first run established.

The options were to write that beside the package, in the report file, or inside the package.
A file beside a package is lost when the package is copied, shared or installed alone.

## Decision

A package carries, inside itself, the evidence and the metadata of the run that made it: the
build the mod was dated at and what dated it, the WADs that restore a model from before a
rework, the archive it was made from by its content id, the target build, the program's
version, and the fixups applied.

A later run that is given a package reads these and does not judge them again from the
package's bytes.

The report beside the package stays. It is for a person and for tooling. What a later run
needs is in the package.

## Consequences

- A package can be repaired or ported again and gives the same result as its archive would.
- The embedded data is the mod's metadata, not game data: the client does not load it, and it
  is traced to the run, not to a bin.
- What is embedded is a format of its own with a version. A package without it is judged as an
  archive is.
- Where it sits in a `.modpkg` and in a project directory is for the package module to settle
  with the formats `ltk_modpkg` and `ltk_mod_project` give.
- Embedded data is read from an untrusted file by the next run, so it is parsed as any mod
  input is, and a package that claims a date or a restoration its bytes cannot support is a
  case the reader has to bound.
