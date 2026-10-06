# ADR-0063: A game bank unit the mod's skin leaves out is left out of the port

- **Status:** Accepted (2026-10-06)
- **Related:** #ADR-0028, #ADR-0030, #MOD-ARTIFACT-0028, #GAME-BEHAVIOR-0016,
  #GAME-BEHAVIOR-0025; `docs/design/RUNS.md`, "Ports in game"

## Context

A skin entry's `skinAudioProperties.bankUnits` lists the bank units the client loads for the
skin. A mod ships its own skin entry, so a game unit its list leaves out is not loaded with the
mod, and that unit's sounds do not play.

A port keeps the game's audio block at the target as it is and adds the mod's units after it
(#ADR-0028). A game unit the mod left out is loaded again.

In `40606c273797…` the skin entry lists `Kennen_Base_SFX` alone, with the mod's banks, and leaves
out `Kennen_Base_VO`: the mod turns the champion into a creature that does not speak. With the
original installed, no voice line plays. Every port of it checked in game plays the game's voice
beside the mod's sounds (2026-10-06, `docs/design/RUNS.md`, "Ports in game").

A game unit can be missing from a mod's list for another reason: the game added it after the mod
was made. Then the author left nothing out.

The options: keep every game unit, as the port did; leave out every game unit the mod's list
lacks; leave out only those the game had when the mod was made.

## Decision

The user, 2026-10-06: **a game bank unit at the target that the mod's skin entry leaves out is
left out of the port, when the game had a unit of that name when the mod was made.**

- When the mod was made is the dating's bound, the oldest build it can have been made at. A unit
  the game had at that build it had at every later one, the mod's own included.
- A mod the history does not date keeps every game unit: nothing tells a unit the author left
  out from one the game added later.
- A mod whose skin entry holds no `skinAudioProperties` keeps the game's audio block as it is:
  no list of the author's says which units to load.
- A unit of the mod's under a game unit's name stands in for it as before (#MOD-ARTIFACT-0028).
- A voice unit left out is left out of every layer. The voice layer (#ADR-0028) still adds the
  mod's own voice when it is on.

## Consequences

- The port of `40606c273797…` loads no voice unit, as the original does.
- The port reads the game's skin entry at the mod's build from the history, a typed question of
  the facts (#ADR-0043).
- A unit an author left out by mistake is left out too: the port plays what the author's game
  played with the mod.
- A unit left out is reported, with the build that dates the mod.
