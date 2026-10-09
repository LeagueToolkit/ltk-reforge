# GAME-BEHAVIOR-0002: A bin value of another type than the build reads is dropped

- **Status:** Confirmed in game (2026-10-10)
- **Builds:** 16.19
- **Affects:** every bin property
- **Related:** none

## Behaviour

Each property in a bin carries a type tag. The client reads a property only when its tag is the
type the running build registered for that field of that class. A property of another type is
dropped without a message, and the member keeps its default. A property whose field the class
does not declare is dropped the same way.

The client does not convert: a path string where the build reads a file hash is not hashed, an
integer of another width is not widened.

## In a mod

A mod made at one build and played at a later one loses every property whose type Riot changed
in between. Nothing is logged. The mod loads, and what the property set is missing: a texture
is not drawn, an effect keeps its default.

A mod's author never saw a property their own build dropped. Such a value showed nothing when
the mod was made.

## Evidence

- Seen in game during ltk-forge, with no date recorded: a port's linked bins written with the
  mod's own types held path strings where the build reads file hashes, and the textures they
  named were not drawn. Retyped, they were.
- Seen in game, with no date recorded: one undeclared field nested in a map left a skin in its
  bind pose.
- LTK Manager's repair rule for property types rests on the same reading.
- Seen in game (2026-10-06): one undeclared field added alone, a `birthTranslation` inside a
  `VfxShapeLegacy`, did not move its emitter, and the rest of the effect was drawn
  (#MOD-ARTIFACT-0019).
- Seen in game (2026-10-10, the 16.19.8207193 copy, a replay of Braum at skin 0): one property
  with its type changed and nothing else. A material's `switches`, which the build reads as a
  `List2` of embedded `StaticMaterialSwitchDef`, written as a `List` of pointers holding one
  switch set off. The game logged nothing of it and drew the material with the switch at the
  shader's default, as the same material with no `switches` draws. Written as a `List2` of
  embeds, the same switch asked the shader for a variant its cache lacks and the game crashed
  (#GAME-BEHAVIOR-0029). LTK Manager warned of the first mod before it was played.
