# GAME-BEHAVIOR-0025: A sound bank's bus objects are loaded with it

- **Status:** Confirmed in game (2026-10-06)
- **Builds:** 16.19
- **Affects:** `.bnk`: the bus objects of a `HIRC` section; the output bus of a sound or a
  container
- **Related:** #GAME-BEHAVIOR-0012, #GAME-BEHAVIOR-0017, #ADR-0030, #MOD-ARTIFACT-0035

## Behaviour

A sound bank other than the game's init bank may hold bus objects in its `HIRC`, and the client
loads them with it: a bus the init bank does not define is there for the objects that route to
it.

Inferred: when one object of an events bank routes to a bus no loaded bank defines, none of the
bank's events play, not only the ones that reach that object.

Not established: whether a bank's bus object under an id the init bank defines changes anything.
The mod's copies of three of the game's buses, carried alone, changed nothing heard.

## In a mod

A mod's banks may define buses of their own, beside copies of the game's. A port that carries the
mod's events without the buses they route to plays none of the bank's sounds; carrying the buses
the game lacks is enough.

## Evidence

- Seen in game (2026-10-06, the 16.19.8207193 copy, a replay of Kennen at skin 0), on the port of
  `40606c273797…` (31 own clips, 12 events cut from the mod's own events bank, #ADR-0030). The
  mod's media bank `Pikachu_SFX_audio.bnk` holds the 31 clips and five bus objects: 3803692087,
  393239870 and 712897226, ids the game's init bank defines with other parameters, and
  3276768291 and 3605772915, which it does not define. The eleven sounds the mod's effects events
  play route to 393239870. The music container 1028971914, which one event plays, routes to
  3605772915.
  - The original mod, installed as it is: the mod's sounds on the emotes.
  - The port, its media bank the 31 clips alone: Kennen's own sounds, none of the mod's, at a
    recall, a death and the respawn, the R and the emotes.
  - The port with the five bus objects added to its media bank, nothing else changed: the mod's
    sounds play.
  - The port with the three buses the init bank defines added: Kennen's own sounds only.
  - The port with the two buses the init bank lacks added: the mod's sounds play.
  - The port with the mod's banks moved into the game's `Kennen_Base_SFX`, as the original lists
    them, and its media bank the clips alone: Kennen's own sounds only.
  - In each check where the mod's sounds play, Kennen's own voice is heard beside them. The
    original's skin entry loads no voice unit.
  - The port of a build that carries the two buses the init bank lacks, and leaves the voice
    unit out of the block (#ADR-0063): the mod's sounds play, and no voice.
- The bank of each variant was read back before the check: the clips and the bus objects byte for
  byte the original's.
- The inference: the effects route to 393239870, which the init bank defines, and play only when
  3605772915 or 3276768291 is defined. Of the objects the port carries, only the music container
  routes to one of the two. A port with the music container left out and no bus added would
  confirm it.
- The five ids (computed 2026-10-06): 3803692087, 393239870 and 712897226 are the FNV-1 hashes of
  `Master Audio Bus`, `SFX` and `mus`. In the mod's bank `SFX` and `mus` are children of
  `Master Audio Bus`, and the two ids the init bank lacks are children of `mus`.
- An authors' guide to custom banks (read 2026-10-06) states that a mod cannot change the game's
  buses, only route to them or add child buses, and that of two banks defining one element the
  second is discarded. Neither is checked; the copies of the game's buses changing nothing heard
  agrees with both.
