# GAME-BEHAVIOR-0012: Of two banks defining one id the client keeps one

- **Status:** Confirmed in game (2026-09-23 to 2026-09-26, and 2026-09-29)
- **Builds:** the live builds of those days; the numbers were not recorded
- **Affects:** `.bnk`, `.wpk`; the clips and objects of the banks a skin's bank units load
- **Related:** #GAME-BEHAVIOR-0011, #GAME-BEHAVIOR-0014

## Behaviour

Clips and objects are looked up by id across every loaded bank. When two loaded banks define
one id, the client keeps one of the two. For voice it keeps the later unit's.

Not established: which of two effects banks is kept, and whether the order of the units in
`bankUnits` decides it there too.

## In a mod

- A media bank listed in a unit after the game's, holding clips under the game's ids, has its
  clips played by the game's own events.
- A unit listed after the game's needs no copy of the game's clips: the ids it does not
  define are found in the game's banks.

## Evidence

- The rule was settled in game between 2026-09-23 and 2026-09-26, before ltk-forge. ltk-forge's
  audio design states it and holds no account of the single checks.
- Checked in game on 2026-09-29: a port of one sample whose 60 own effects clips and 7 own
  voice lines ship in units after the game's, under the game's ids and with events banks that
  hold no object, plays with its own sounds.
- Checked in game on 2026-09-30: one sample's 252 own voice lines, in a unit after the game's
  voice unit, play with their layer on and not without it (#ADR-0028).
