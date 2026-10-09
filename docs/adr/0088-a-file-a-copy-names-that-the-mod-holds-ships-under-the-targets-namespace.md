# ADR-0088: A file a copy names that the mod holds ships under the target's namespace

- **Status:** Accepted (2026-10-09)
- **Related:** #ADR-0082, #ADR-0083, #GAME-BEHAVIOR-0007

## Context

A name the target's unit reaches that only the game's bins of another skin hold is carried as
a copy of the game's object, under the target's namespace (#ADR-0083). The files a copy names
stay at the game's paths, since a file the champion's WAD holds is there at any skin
(#GAME-BEHAVIOR-0007), and no file ships for a copy.

A mod often holds its own bytes at such a path: an author who changes a premium skin's effect
overrides its texture at the game's path. A port ships the mod's own bytes under the target's
namespace and names them where its objects name them. A carried game object that names an
overridden file is carried for that reason, so that the file ships and the links to it follow.
A copy is a game object carried the same way, but the port names the files of its own and
carried objects only: an own file that only a copy names ships nowhere, and the copy shows the
game's bytes. The 1,585 copies of the corpus name 5,136 files (`docs/design/RUNS.md`, "Names
other skins hold").

## Decision

A file a copy names that the mod holds at that path ships under the target's namespace, as a
carried object's file does, and the copy's link follows it. Every other file a copy names stays
at the game's path.

## Consequences

- A copy shows the author's bytes where the mod holds them, and the game's where it does not.
- A port ships a file for a copy only when the mod holds it. #ADR-0083's consequence that no
  file ships for a copy is superseded by this.
- Of the 5,136 files the corpus's copies name, the mods hold 261, in 25 of the 161 ports that
  copy; those ports ship them (`docs/design/RUNS.md`, "A copy's own files").
