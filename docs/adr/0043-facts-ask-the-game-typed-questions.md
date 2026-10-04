# ADR-0043: Facts ask the game typed questions

- **Status:** Accepted (2026-10-05)
- **Related:** #ADR-0010, #ADR-0042, #ADR-0004; `docs/design/ARCHITECTURE.md`, "Facts"

## Context

forge's analyses read the install's bytes where they need the game: a skin's bins, a skeleton,
a mesh, an animation clip. A run then needs an install, and it can only be judged against the
build that install holds.

The user wants a run on the census history alone, and against the game at any build the
history holds (2026-10-04). The user also judged what a comparison with the current game can
show: it is sound for porting a mod to the current build, and it does not prove what the
author changed. What the author changed is the mod against the game at the mod's own build.

The census history holds a skeleton's joints, a mesh's submeshes, a clip's joints and a bin's
objects (#ADR-0042). It holds no file bytes.

## Decision

A stage after `load` does not read a game chunk's bytes to learn a fact about it. It asks a
typed question, and the game answers:

- the skins a character has, with each skin's identity, rig files, links, clips and objects;
- a skeleton's joints and influences;
- a mesh's submeshes, vertex positions and highest bound joint;
- the joints a clip animates.

`GameView` answers each from the install's bytes and keeps the answer. A game source backed by
the history answers the same questions from what the census recorded.

Two comparisons are kept apart:

- **Where a unit goes** (`facts`, `decide`) is judged against the target build: which skin its
  work derives from, and whether it fits a slot's rig as the game holds it today.
- **What the author changed** (`plan`, `build`) is the mod against the game at the mod's own
  build, applied onto the target build.

## Consequences

- No code of `facts` or `port` calls `Game::read`. A test holds them to it.
- A question the history cannot answer is known by its type: the share of a submesh's vertices
  on another mesh needs positions, which the census does not record. A history-backed source
  answers it as not known, and the check it feeds is reported as not judged.
- The decision of step 4 is compared with forge's, which read the install (#ADR-0004). The
  answers are the same facts, so the comparison holds.
