# ADR-0032: No file is dropped for looking like the game's

- **Status:** Accepted (2026-10-01)
- **Related:** #ADR-0022, #ADR-0023

## Context

A mod carries converted copies of game textures: the same picture in another container or
another compression, so the bytes differ from the game's. A port that knew them for the game's
could point at the game's file and ship less.

Judging by the picture was tried before ltk-forge, on 256-pixel thumbnails with alpha
premultiplied. It dropped textures that were not the game's, and broke mods. The history carries
no file bytes, so a converted copy of a texture the install no longer has cannot be compared at
all.

## Decision

A file is the game's only by its bytes. No file is dropped, repointed or left out because its
picture looks like a game file's. A comparison of pictures may report, and it is held: it is not
built until asked for (the user's call, 2026-10-01, during ltk-forge).

## Consequences

- A port ships converted copies of game textures as the mod's own.
- Stale copies are found by bytes against the history (#ADR-0023), which needs no picture.
