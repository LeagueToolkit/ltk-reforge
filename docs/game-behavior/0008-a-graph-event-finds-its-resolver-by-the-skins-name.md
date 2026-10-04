# GAME-BEHAVIOR-0008: A graph event finds its resolver by the skin's name

- **Status:** Confirmed in game (2026-09-30)
- **Builds:** the live build of 2026-09-30; the number was not recorded
- **Affects:** `ResourceResolver`, `resourceMap`; a skin entry's `mResourceResolver`; the
  particle events of an animation graph (`mEffectKey`)
- **Related:** #GAME-BEHAVIOR-0019, #ADR-0006

## Behaviour

A skin entry links its resolver in `mResourceResolver`. The game names that object
`Characters/<C>/Skins/Skin<N>/Resources`. A spell that plays an effect by key looks the key up
in the resolver the entry links.

A particle event of the skin's animation graph looks its key up in the resolver under the
skin's own name, whatever object the entry links. The resolvers of the entry's
`mAdditionalResourceResolvers` are #GAME-BEHAVIOR-0019.

## In a mod

- A resolver shipped under another name and linked from the entry serves the spells and not
  the graph's events: an emote loses its particles.
- A key added to the game's own resolver, under the game's name, serves both.

## Evidence

- Checked in game on 2026-09-30, on a port of one mod. With the mod's resolver shipped as an
  object of the port's own name and the entry's link patched to it, the spells played their
  effects and an emote lost its particles. The emote's graph events play effects by key.
- With the game's resolver patched in place, its map holding the game's keys and the mod's,
  the emote played. Confirmed in game on 2026-09-30.
