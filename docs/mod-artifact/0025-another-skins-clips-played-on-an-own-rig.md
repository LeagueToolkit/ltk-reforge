# MOD-ARTIFACT-0025: Another skin's clips played on an own rig

- **Source:** authors
- **Affects:** `AnimationGraphData`, animation clips (`.anm`), skeletons (`.skl`)
- **Related:** #GAME-BEHAVIOR-0005, #MOD-ARTIFACT-0002, #ADR-0014, #ADR-0015

## What is found

A unit whose graph plays animation files that are a game skin's: by the path the graph names,
or by the bytes of a copy the mod ships. The skin is another one than the slot the unit sits
in, and the unit's mesh and skeleton are often the mod's own.

Each clip the graph plays is one of:

- the slot's: the game's graph of that slot plays it;
- the mod's own: bytes the game never shipped;
- another skin's: only other skins' graphs play it. Against the unit's skeleton, such a clip
  has every track on a joint of it, some tracks on its joints and others on joints it lacks, or
  no track on its joints.

Three shapes recur:

- an own model and mostly own clips, with a few of a premium skin's (a dance, a recall);
- an own model on a premium skin's whole animation set;
- the game's own graphs, which mix in a clip of another skin.

## Why it is there

Animating is the costliest part of a skin. An author who models a character rigs it to an
existing skeleton's joint names, so that an existing skin's clips play on it, and animates only
what they need.

## What reads it, and how

The client plays each clip on the unit's skeleton, track by track (#GAME-BEHAVIOR-0005): a
clip with a track on some joint of the skeleton plays, and its other tracks are skipped.

A clip named by its game path is found only where the client mounts the file: for a premium
skin's clip that is whenever the champion plays, since a champion's skins share one WAD.

## The repair

None is needed for the clips. What the unit's target is depends on how much of what it plays is
another skin's (#ADR-0014, #ADR-0015).

## Evidence

- ltk-forge's first corpus (1,029 mods, 2026-09-29): with clips of other skins refused, clips
  alone ruled out skin0 for 317 of 677 units that fell back to a premium skin.
- ltk-forge's fifth datasets run (2026-09-30): 162 units fitted no slot for clips of other
  skins. With a unit that plays mostly one skin's clips taken as that skin's, 81 of them went to
  that skin.
- Two samples checked in game by ltk-forge's ports (2026-09-29): own models with 33 of 59 and
  20 of 38 clips their own play on skin0.
