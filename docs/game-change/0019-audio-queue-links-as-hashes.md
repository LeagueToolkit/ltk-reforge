# GAME-CHANGE-0019: Audio queue links as hashes

- **Build:** 16.18; 16.18.8159717 is the first build the schema has it at
- **Affects:** `AudioQueueConfigList.queues`, `AudioQueueConfig.0x1c6439cf`,
  `AudioPriorityBehavior.TargetQueue`
- **Related:** #GAME-BEHAVIOR-0002, #MOD-ARTIFACT-0022

## Before

Up to 16.17 three properties name an audio queue by a `link` to an `AudioQueueConfig` object:

| property | type |
|---|---|
| `AudioQueueConfigList.queues` | a list of `link` |
| `AudioQueueConfig.0x1c6439cf` | a list of `link` |
| `AudioPriorityBehavior.TargetQueue` | `link` |

The three classes exist from 16.9. `queues` is declared from 16.11 and `0x1c6439cf` from 16.12.

## After

From 16.18 the three hold a `hash` where they held a `link`: `queues` and `0x1c6439cf` are lists
of `hash`, and `TargetQueue` is a `hash`.

## Riot's conversion

Not established from the game's data. The census history holds no object of the three classes
at the builds counted, from 16.9 to 16.19, so no game value shows what a link became.

A `link` and a `hash` are both written as four bytes, and a link's value is the hash of the
linked object's name. That each link became the hash of the same four bytes is inferred from
that. A game object of one of the classes read at 16.17 and at 16.18 would confirm it.

## In a mod

A mod that holds a `link` in one of the three properties loses the value at a build from 16.18,
which reads a `hash` there (#GAME-BEHAVIOR-0002).

No mod of the baseline run holds one. LTK Manager's repair rule does not convert between `link`
and `hash` (#MOD-ARTIFACT-0022).

## Evidence

- The meta database (generation 2026-09-28, read 2026-10-04): `AudioQueueConfigList.queues` is
  a list of links to `AudioQueueConfig` from 16.11 to 16.17 and a list of `hash` from 16.18.
  `AudioQueueConfig.0x1c6439cf` is the same from 16.12 to 16.17 and from 16.18.
  `AudioPriorityBehavior.TargetQueue` is a link to `AudioQueueConfig` from 16.9 to 16.17 and a
  `hash` from 16.18. The three changes are at build 8159717. The three classes are declared
  from 16.9.
- Counted in the census history's entry texts by the classes' hashes and by the hashes of
  `TargetQueue` and `0x1c6439cf` (2026-10-04): no line at 16.11.7787901, 16.17.8104348,
  16.18.8159717 or 16.19.8207193. By the class hash of `AudioQueueConfigList` and
  `AudioQueueConfig`: none at 16.9.7690805 either.
- The hash of the name `queues` is held by one game object at those builds, a
  `ModeSelectViewController`, whose `queues` is a list of links to `MatchmakingQueue` and does
  not change at 16.18.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: no mod holds a
  `link` where 16.19 reads a `hash`.
