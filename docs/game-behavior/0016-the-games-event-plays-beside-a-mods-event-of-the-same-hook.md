# GAME-BEHAVIOR-0016: The game's event plays beside a mod's event of the same hook

- **Status:** Confirmed in game (2026-10-01)
- **Builds:** the live build of 2026-10-01; the number was not recorded
- **Affects:** a skin entry's `skinAudioProperties.tagEventList` and `bankUnits`
- **Related:** #GAME-BEHAVIOR-0011, #MOD-ARTIFACT-0028

## Behaviour

A hook is composed once under each tag of the entry's list (#GAME-BEHAVIOR-0011). When the
game's audio block stays and a mod's unit and tag are added after it, the hook under the game's
tag plays the game's event and the hook under the mod's tag plays the mod's: both sound.

## In a mod

- A mod that replaced a skin's whole audio block has its own events alone. A port that adds
  the mod's units after the game's plays the game's sounds beside the mod's where the mod
  replaced a sound.
- An event renamed under the game's own tag at the target, with no tag added, did not stand in
  for the game's event in the one case tried.

## Evidence

- Seen in game on 2026-10-01, on ports of two samples, with and without the mods' own events
  banks: where the mod replaced a sound, the port plays the game's and the mod's.
- The same day, a replay of one port with the kept events renamed under the game's tag at the
  target and no tag added sounded no different.
- One port plays the game's sounds beside its own where the mod does not, though its unit loads
  the game's base events bank and a copy of the base media bank that is the game's clip for
  clip. The cause is not established.
