# MOD-ARTIFACT-0036: Emitter vectors as floats in restored old effects

- **Source:** not established
- **Affects:** `VfxEmitterDefinitionData.scale0`, `birthScale0`, `birthRotation0` and
  `birthRotationalVelocity0`; `ValueVector3.dynamics`; `VfxAnimatedVector3fVariableData.values`
- **Related:** #GAME-BEHAVIOR-0002

## What is found

An emitter field the game reads as a `ValueVector3` that holds a float, in one of three shapes:

- the field an `embed = ValueFloat`, its constant an `f32` and its animation a
  `VfxAnimatedFloatVariableData`;
- the field a `ValueVector3` whose `dynamics` is a `VfxAnimatedFloatVariableData`;
- the field a `ValueVector3` whose `dynamics` is a `VfxAnimatedVector3fVariableData` with a list of
  `f32` as its `values`.

The fields are `scale0`, `birthScale0`, `birthRotation0` and `birthRotationalVelocity0` of an
emitter.

It is told from deliberate work by the type: no emitter of the game holds one of these fields as
a float at any build the census history holds.

## Why it is there

Not established. Every mod found restores a champion's or a skin's effects as an old build drew
them, by its name and description: one names a build before 8.17, others a season of 2016 or a
skin's look of 2013. The census history begins at 8.20. Whether the game's own emitters held
these fields as floats before it, or a tool that carried the old effects into a later layout
wrote them, is not read.

## What reads it, and how

The client drops a value of another type than the build reads (#GAME-BEHAVIOR-0002), and the
field keeps its default: a scale of 1 and no rotation on each axis. The mods found are dated 15.10
or later, so the authors' own builds read a vector there and dropped the float too.

## The repair

None. No build of the census history holds such a value, so no conversion of Riot's is read off
one, and a float set on the three axes is not read off any.

## Evidence

- The repair baseline (2026-10-04), over the 5,137 mods of the datasets, by the type check: 36
  mods hold a shape above in a bin the client loads. `VfxAnimatedVector3fVariableData.values` a
  list of floats in 26 mods (73 values), `ValueVector3.dynamics` a `VfxAnimatedFloatVariableData`
  in 23 (408), an emitter's field a `ValueFloat` in 10: `scale0` in 10 (50), `birthScale0` in 6
  (6), `birthRotationalVelocity0` in 4 (52), `birthRotation0` in 4 (36). 4 of the 36 also hold a
  `VfxAnimatedFloatVariableData` whose `values` is a list of vectors.
- The 36 are dated from 15.10.6796849 to 16.15.7983109. They are versions of a few mods, for
  Udyr, Lee Sin, Rengar and Karthus.
- The meta database (generation 2026-10-05, read 2026-10-06): the four fields are
  `Embed<ValueVector3>` at every build from 13.15 to 16.19, with no other type in their history.
- The census history, branch `history-v2`, read 2026-10-06: the entry texts that hold each field
  as an `embed` of `ValueFloat` and of `ValueVector3`, by the hashes of the field and the class,
  at 14 builds: 8.20.2483196, 9.1.2577563, 9.13.2797717, 10.1.3036554, 10.13.3255485,
  11.1.3503054, 11.13.3818870, 12.1.4143965, 12.12.4483320, 12.18.4683251, 12.23.4826195,
  13.1.4875491, 13.14.5204528 and 16.19.8230722. No line holds one of the four as a `ValueFloat`.
  Each is held as a `ValueVector3` in 26,750 to 1,761,037 lines at each build.
