# GAME-BEHAVIOR-0015: Events banks load at the client's version only

- **Status:** Confirmed in game (2026-10-01)
- **Builds:** the live build of 2026-10-01, whose bank version is 145, as at 16.19
- **Affects:** `.bnk` files, the version in the `BKHD` section; `.wpk` packages
- **Related:** #MOD-ARTIFACT-0027, #MOD-ARTIFACT-0005, #ADR-0030

## Behaviour

The client loads a bank with a `HIRC` section only at the bank version it was built for, 145
at 16.19. A media bank, with `DIDX` and `DATA` alone, loads at older versions too.

A voice package (`.wpk`) sits beside a bank with a header alone. When that header is of an
older version, the package is blocked: none of its clips plays.

Not established: the builds at which the client's version changed. The census history holds
events banks of versions 125, 132, 134 and 145.

## In a mod

- An events bank of an older version is silent. A mod made for an older client loses the
  events of its own, and keeps the clips the game's current events play.
- A voice package beside a stub of an older version is silent whole, the lines the game's
  events would play included (#MOD-ARTIFACT-0027).
- A bank written with a header of the client's version loads.

## Evidence

- Checked in game on 2026-10-01: one mod released twice with the same 41 voice lines, its stub
  of version 134 in the release made at 15.11 and of version 145 in the one made at 15.16. The
  older release plays none of the author's lines. The newer release plays them, and so do the
  ports of both, whose banks are all written at version 145.
- That media banks load at older versions was settled in game between 2026-09-23 and
  2026-09-26, before ltk-forge. ltk-forge's audio design states it and holds no account of the
  single checks.
- The versions in the history: its index, built 2026-10-04, lists 5,775 events banks of
  version 125, 43,471 of 132, 79,844 of 134 and 33,840 of 145.
