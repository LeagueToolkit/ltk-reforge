# GAME-BEHAVIOR-0001: A sub-character's skin bin is looked for at the champion's skin, then at skin0

- **Status:** Confirmed in game (2026-09-30)
- **Builds:** the live build of 2026-09-30; the number was not recorded
- **Affects:** `data/characters/<sub-character>/skins/skin<N>.bin`
- **Related:** none

## Behaviour

A champion's WAD also holds its sub-characters: a pet, a clone, a trap, a spirit. Each is a
character with skin bins of its own under `data/characters/<sub-character>/skins/`.

When the champion wears skin N, the client looks for the sub-character's `skin<N>.bin`. When a
file is at that path, it loads it. When none is, it uses the sub-character's `skin0.bin`.

For many sub-characters the game ships `skin0.bin` alone. The client still looks at
`skin<N>.bin`: a bin a mod ships there is loaded, though the game never shipped one at that
path.

Not established: whether a sub-character with bins at some premium slots and not at N uses
skin0 or another slot's bin.

## In a mod

- A mod that ships a sub-character's `skin<N>.bin` where the game has none changes the
  sub-character under skin N only.
- A sub-character's unit ported to a slot where the game has no bin has no game bin to patch.
  A whole bin shipped at that path loads.

## Evidence

- Checked in game on 2026-09-30, before ltk-forge's port of such units was built. A probe mod
  defined an idle effect only in a new `skin16.bin` of a champion's spirit, a sub-character the
  game ships at skin0 alone. With the champion on skin 16 the effect showed on the spirit. With
  no mod it did not.
- The fall back to skin0 is inferred from the same check: without the mod the spirit drew as
  its skin0 defines it.
- ltk-forge's first full datasets run wrote 7 such bins over 5,544 mods.
