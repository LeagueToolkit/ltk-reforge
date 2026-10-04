# GAME-BEHAVIOR-0014: A play action finds its bank by the id in the header

- **Status:** Confirmed in game (2026-09-30)
- **Builds:** the live build of 2026-09-30; the number was not recorded
- **Affects:** `.bnk` files, the id in the `BKHD` section; the play actions of a `HIRC` section
- **Related:** #MOD-ARTIFACT-0023, #GAME-BEHAVIOR-0013

## Behaviour

A play action names the bank it expects its target in, by a bank id. The client matches that
id with the id in a loaded bank's header. The bank's file name takes no part: a bank whose file
name does not hash to the id in its header plays.

A game bank's header id is the 32-bit FNV-1 hash of the lowercased stem of its file name
(#MOD-ARTIFACT-0023).

## In a mod

- A bank may ship under any file name, so long as its header id is the one its play actions
  name.
- An events bank cut from another has every play action that named the source pointed at the
  new bank's id, or the actions look for a bank that is not loaded.
- A header id that is unique across mods and skins keeps two mods' banks apart.

## Evidence

- Checked in game on 2026-09-30, on ports of two samples: the effects and voice of one and the
  voice of the other play from banks named by their role (`sfx_audio.bnk`, `vo_events.bnk`),
  whose header ids are the hashes of their bank units' names and not of the file names.
