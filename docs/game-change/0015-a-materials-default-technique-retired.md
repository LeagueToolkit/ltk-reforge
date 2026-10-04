# GAME-CHANGE-0015: A material's default technique retired

- **Build:** 13.5.4953660
- **Affects:** `StaticMaterialDef.defaultTechnique`
- **Related:** #GAME-BEHAVIOR-0002

## Before

Up to 13.4 a material holds `defaultTechnique: string`, the name of one of its `techniques`.
Every game material holds `"normal"` there.

Almost every material has one technique, named `normal`. At 13.4.4941549, 17 of 14,501 have a
second, named `transition`.

## After

From 13.5 no material holds `defaultTechnique`, and the class does not declare it. The
`techniques` are as before.

## Riot's conversion

The field was removed from every material. Nothing took its place: no field was added to a
material at that build, and the techniques kept their names.

## In a mod

A mod made before 13.5 holds `defaultTechnique` on its materials. A later build drops it
(#GAME-BEHAVIOR-0002).

- **`"normal"`** names the technique every game material held as its default. Nothing the
  author saw depends on the value.
- **Another value** chose a technique no game material chose. A later build reads no choice.
  Which technique the client then draws a material of two techniques with is not established.

A mod made later holds the field too when its author copied a material from an older mod.

## Evidence

- Counted in the census history's entry texts by the class's and the field's hashes
  (2026-10-04): `defaultTechnique` is `"normal"` in 3,817 of 3,817 materials at 10.9.3185929 and
  in 14,501 of 14,501 at 13.4.4941549. No material holds it at 13.5.4953660 (14,578 materials),
  13.15.5229820 (16,365) or 16.19.8207193 (33,165).
- The same materials paired across 13.4.4941549 and 13.5.4953660: 14,497 are in both builds, and
  14,478 are equal but for the removed field. The other 19 changed in another field at that
  build as well.
- The techniques (the same count): at 13.4.4941549, 14,484 materials hold one technique named
  `normal` and 17 hold `normal` and `transition`. At 13.5.4953660 it is 14,561 and 17. At
  16.19.8207193 each of the 33,165 holds one, named `normal`.
- The meta database (generation 2026-09-28, read 2026-10-04): `StaticMaterialDef` declares no
  `defaultTechnique` at any build from 13.15 to 16.19. `techniques` is a list of
  `StaticMaterialTechniqueDef` throughout. The database declares no types before 13.15, so the
  layout up to 13.4 is the fields the census saw on the game's materials.
- The baseline run (2026-10-03), in the objects ltk-forge's port repaired: 136 of the 5,137
  mods hold the field, on 408 materials. Each holds `"normal"`.
- ltk-forge's notes give 10,185 of 10,185 game materials at 13.4, with no build and no date
  recorded.
