# GAME-BEHAVIOR-0017: A sound plays only from a bank that holds the nodes above it

- **Status:** Confirmed in game (2026-09-23 to 2026-09-26)
- **Builds:** the live builds of those days; the numbers were not recorded
- **Affects:** the objects of a `HIRC` section: sounds, containers, actor-mixers, actions, events
- **Related:** #GAME-BEHAVIOR-0014, #ADR-0030

## Behaviour

A node of a bank's hierarchy is reached from its parent, through the parent's list of
children. A bank that holds a sound and lacks a container or a mixer above it plays nothing
through that sound.

## In a mod

An events bank reduced to the events a mod needs keeps every node of the source. Only the
events and their actions that reach none of the mod's clips are taken out.

## Evidence

- The rule was settled in game between 2026-09-23 and 2026-09-26, before ltk-forge. ltk-forge's
  audio design states it and holds no account of the single checks.
- The ports checked in game on 2026-09-29 and 2026-10-01 carry events banks cut this way, every
  node kept, and play (#GAME-BEHAVIOR-0011, #MOD-ARTIFACT-0005).
