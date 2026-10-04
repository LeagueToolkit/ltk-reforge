# GAME-BEHAVIOR-0019: A graph event does not look in the additional resolvers

- **Status:** Inferred
- **Builds:** the live build of 2026-09-30; the number was not recorded
- **Affects:** a skin entry's `mAdditionalResourceResolvers`; `ResourceResolver`,
  `resourceMap`; the particle events of an animation graph (`mEffectKey`)
- **Related:** #GAME-BEHAVIOR-0008, #ADR-0006

## Behaviour

A skin entry lists further resolvers in `mAdditionalResourceResolvers`, beside the one it links
in `mResourceResolver`. A particle event of the skin's animation graph looks its key up in the
resolver under the skin's own name (#GAME-BEHAVIOR-0008) and in no resolver of that list.

Not established: whether a spell that plays an effect by key looks in the list.

## In a mod

- A key held only by a resolver of the list plays no effect from a graph event: an emote that
  plays particles by key loses them.
- A port cannot leave the game's keys to the game's resolver and add its own keys in a
  resolver of the list. The keys a graph event needs are in the resolver under the skin's
  name, so that resolver's map is written whole, the game's keys with the mod's laid over, and
  holds the game's keys as they were at the build ported against.

## Evidence

- ltk-forge's `WRITER.md` and the module of its own resolver state it as seen in game, beside
  the checks of 2026-09-30 that established #GAME-BEHAVIOR-0008. Neither gives a date for
  this check, the mod it was made on, or what was shipped and what was seen.
- A check would confirm it: a port whose keys sit in a resolver of its own, added to
  `mAdditionalResourceResolvers`, with the game's resolver untouched, and an emote whose graph
  events play effects by those keys, seen with and without the port.
