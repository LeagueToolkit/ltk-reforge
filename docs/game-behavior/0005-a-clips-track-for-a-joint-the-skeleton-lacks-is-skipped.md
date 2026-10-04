# GAME-BEHAVIOR-0005: A clip's track for a joint the skeleton lacks is skipped

- **Status:** Inferred
- **Builds:** the live builds of 2026-09-29 and before; the numbers were not recorded
- **Affects:** animation clips (`.anm`), skeletons (`.skl`)
- **Related:** #MOD-ARTIFACT-0025, #ADR-0014

## Behaviour

A clip holds one track per joint it animates. A track names its joint by the ELF hash of the
joint's name, as the skeleton spells it or lowercased: the casing varies with the clip's format.

When a clip plays on a skeleton, each track is bound to the joint whose name has its hash. A
track whose hash no joint of the skeleton has is skipped. The other tracks play.

So a clip made for one skeleton plays on another as far as the two share joint names. A clip
that shares no joint with the skeleton moves nothing of it.

Not established: what the client does with a joint of the skeleton that no track of the clip
names.

## In a mod

- A mod that plays another skin's clips on a skeleton of its own plays them, where its
  skeleton keeps the joint names of the skin's. The tracks for the skin's other joints (a
  cape, a companion) are skipped without a word.
- A clip with no track on a joint of the skeleton it plays on shows no motion.

## Evidence

- Inferred from mods that play in game. One mod ships at skin0 and plays 26 clips of a premium
  skin whose tracks also drive the 92 joints of that skin's companion, on a skeleton of its own
  with 164 joints and none of the companion's. A port of it to skin0 by ltk-forge plays in
  game (seen 2026-09-29).
- Until 2026-09-29 ltk-forge judged a clip to fit only when every joint it animates is in the
  skeleton. That refused such mods, which play.
- What would confirm it: a probe clip with one track on a joint of the skeleton and one on a
  name the skeleton lacks, played in game, with the first joint seen to move.
