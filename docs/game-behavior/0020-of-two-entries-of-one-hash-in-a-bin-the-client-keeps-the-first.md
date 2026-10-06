# GAME-BEHAVIOR-0020: Of two entries of one hash in a bin, the client keeps the first

- **Status:** Confirmed in game (2026-10-06)
- **Builds:** 16.19
- **Affects:** `.bin`: a bin's table of contents and its objects
- **Related:** none

## Behaviour

A bin's table of contents lists each object by its entry hash, and it may list one entry hash
more than once, with an object for each listing. The client keeps the first object listed under
the hash. A later object under the same hash is not drawn.

Checked for a skin's own entry (`SkinCharacterDataProperties`) in a skin bin. Other classes and
other bins are not checked.

`ltk_meta`'s reader keeps the last object instead: its tree holds one object per entry hash, and
a later object replaces an earlier one at the earlier one's place in the bin's order.

## In a mod

- A reader that keeps the last object judges, fixes and ports an object the author did not see in
  game, when the two differ. The first, which the client drew, is not carried.
- When the two objects are equal, the choice changes nothing. How many of the bins hold two that
  differ is not counted.

## Evidence

- Seen in game (2026-10-06, the 16.19.8207193 copy, a replay of Azir at skin 0). Three test mods,
  each Azir's `skin0.bin`. The changed skin entry draws the whole model with
  `ASSETS/Characters/Azir/Skins/Base/Particles/Azir_base_BA_Beam_1x3.tex`, nearly black with
  orange streaks, where the game's draws his own texture.
  - `control`, the changed entry alone: Azir nearly black with orange streaks.
  - `changed-first`, the changed entry, then the game's under the same hash: Azir nearly black
    with orange streaks.
  - `changed-last`, the game's entry, then the changed one: Azir not drawn dark; the user named
    `changed-first` as the case of the two that is.
- `ltk_meta`'s reader, on the same two bins, keeps the game's entry from `changed-first` and the
  changed one from `changed-last`.
- The baseline run (2026-10-03): 128 of the 5,137 mods hold a bin that lists an entry hash
  more than once.
