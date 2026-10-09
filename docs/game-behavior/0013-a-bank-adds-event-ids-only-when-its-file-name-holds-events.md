# GAME-BEHAVIOR-0013: A bank adds event ids only when its file name holds `_events`

- **Status:** Confirmed in game (2026-09-23 to 2026-09-26)
- **Builds:** the live builds of those days; the numbers were not recorded
- **Affects:** `.bnk` files with a `HIRC` section; a bank unit's `bankPath`
- **Related:** #GAME-BEHAVIOR-0014

## Behaviour

The client reads the events of a bank, and so can post them, only when the bank's file name
holds `_events`. A bank of events under another file name loads, and none of its events is
found.

## In a mod

An events bank a tool or a port writes keeps `_events` in its file name, whatever the rest of
the name is.

## Evidence

- The rule was settled in game between 2026-09-23 and 2026-09-26, before ltk-forge. ltk-forge's
  audio design states it and holds no account of the single checks.
- Every events bank a port of ltk-forge writes is named `<role>_events.bnk`. The ports checked
  in game on 2026-09-29 and 2026-09-30 play their events (#GAME-BEHAVIOR-0011,
  #GAME-BEHAVIOR-0014). A port whose events bank is written under a name without `_events`,
  checked in game at a current build with none of its events posted, would confirm the rule
  for that build.
