# ADR-0027: A port's files sit by unit and by use under a namespace

- **Status:** Accepted (2026-09-29); the name of a file no table names superseded by #ADR-0054
- **Related:** #ADR-0026, #ADR-0028, #ADR-0029

## Context

A port ships the mod's own files and objects beside the game's. They need paths and names that
no other mod and no game file has, that say what a file is for, and that keep the units of one
mod from overwriting each other. ltk-forge once wrote every unit's bin of a character to one
path, and the last overwrote the rest.

## Decision

The layout settled with the user on 2026-09-29, during ltk-forge:

- **Namespace.** `<name>.<author>` from the archive's metadata, lowercased and reduced to
  `[a-z0-9_-]` per part. The mod id, the first twelve hex digits of the archive's SHA-256, stands
  in for a part that is missing or reduces to nothing.
- **Files** go under `mods/<ns>/characters/<character>/skin<T>/`, one directory per unit. A file
  goes with its first user in the order the entry reaches them: the entry's own under
  `textures/` (the model at the unit's root), a material's under `materials/<material>/`, an
  effect's under `vfx/<effect>/`, the rest by kind (`animations/`, `meshes/`, `audio/`, `bins/`,
  `files/`).
- **An effect's folder** is its resolver key, the name the game plays it by, else its own name.
- **File names** keep the author's, with the extension from the bytes. A repather's tag and a
  leading `<character>_skin<N>_` or `<character>_base_` are taken off where something is left.
- **Objects** are named in the game's shape under `Mods/<ns>/`:
  `Mods/<ns>/Characters/<C>/Skins/Skin<T>/...`, and the graph
  `Mods/<ns>/Characters/<C>/Animations/Skin<T>`.
- **One patch per unit**, `overrides/<character>-skin<T>.ptch`, on the game's `skin<T>.bin`.
- **A carried object has one new name**, and a unit's objects follow the renames of its group
  only: the champion's units and its sub-characters' at one target, which load together.

## Consequences

- Two units of one character never share a directory.
- A file no table names keeps its hash as its name.
- The textures a port ships are named `.tex`, as the game names its own.
