# ADR-0054: A port's file no table names is named unnamed-hash

- **Status:** Accepted (2026-10-05)
- **Related:** #ADR-0027, #MOD-ARTIFACT-0007

## Context

#ADR-0027 places a port's own files under `mods/<ns>/characters/<character>/skin<T>/`, and a
file no table names keeps its hash as its name: `textures/4faf2c1c6f48efd2.tex`. The port's
bins name that file by the hash of its whole path.

LTK Manager reads a file whose stem is sixteen hex digits as the chunk's hash, in any folder
(#MOD-ARTIFACT-0007). It puts such a file at the hash its name spells, and the port's bins
look for it at the hash of its path. A port of a mod whose skin entry named its texture, loading
screen picture and icons by hash alone showed none of them in game.

The options were a name that is not sixteen hex digits under the namespace, or the file at the
mod's own hash outside the namespace.

## Decision

A file no table names is named `unnamed-<hash>`, sixteen hex digits after the prefix, in the
folder #ADR-0027 gives it (the user's call, 2026-10-05). No path a port makes has a file stem
of sixteen hex digits: an author's file name of that shape takes the next number, as a name
taken does.

## Consequences

- The file stays under the namespace and its name still says the hash it had in the mod.
- A port made before this decision ships such files where LTK Manager does not find them, and
  is made again.
- A repair keeps a nameless file at its own hash at the WAD's root, where a manager reads the
  name as that hash (#MOD-ARTIFACT-0007).
