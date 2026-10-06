# ADR-0067: A sub-character's game value of the mod's build is left to the port

- **Status:** Accepted (2026-10-06)
- **Related:** #GAME-CHANGE-0023, #ADR-0043; `docs/design/ROADMAP.md`, "The porter: next"

## Context

Riot renumbered the health bar styles at four builds (#GAME-CHANGE-0023). A champion's skin entry
holds the champion's bar at every build from 13.4, so its value names the numbering it was
written in, and a fixup renumbers it. A sub-character's value does not: `4` is Tibbers' bar up to
14.16 and another kind of bar from 14.17. The mod's date is a lower bound, by game files a tool
may have copied in, so the date does not name the numbering either.

Such a value is most often the game's value at the mod's build, which the author did not change.
The port's comparison with the game at the mod's own build (#ADR-0043; the roadmap, "The porter:
next", item 4) tells a value the author changed from one the game held.

The options: a fixup that renumbers a sub-character's value by the mod's date; or leave it to the
port's comparison.

## Decision

The user, 2026-10-06: **a value of a sub-character that is a game value of the mod's build is
not a fixup's to renumber.** The port's comparison with the game at the mod's build is where it
is handled.

- The health bar style of #GAME-CHANGE-0023 is one such value.
- The champion's skin entry's style stays a fixup's: its value names its numbering.

## Consequences

- A repair keeps a sub-character's value of the mod's build: it does not compare with the game.
- A port takes the game's value at the target build where the author's equals the game's at the
  mod's build, once the comparison is built.
