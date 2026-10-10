# ADR-0092: A material asking for a shader variant the cache lacks moves to the nearest it holds

- **Status:** Accepted (2026-10-10)
- **Related:** #GAME-BEHAVIOR-0029, #ADR-0069; `docs/design/ARCHITECTURE.md`, "Fixups" and
  "Package"

## Context

A material on a game shader asks for a variant of its programs by its switches and macros. The
cache holds the variants Riot compiled, and the client crashes at the draw of a material that
asks for one it lacks (#GAME-BEHAVIOR-0029). The package check refuses such a material, and 9
mods of the datasets got no package for it: 7 Zed skins with a switch the shader's materials all
set on left off, and 2 skins without a macro every material of Riot's on their shader sets.

In each of the 9 a single define, set on the material, makes every variant it asks for a
compiled one. In game, the Zed material drew with its switch set on; what the switch changes on
screen is not known.

The options: refuse the mod, as the check does; set the one define, and the material draws,
possibly unlike the author's game drew it; or set it only where it is a macro, which changes no
switch of the author's, and refuse the 7 Zed skins.

## Decision

The user, 2026-10-10: **a material that asks a game shader for a variant the target's shader
cache lacks is moved to the nearest variant the cache holds: the one define that, set on the
material, makes every variant of its passes one the cache holds. A material no one define brings
there is left, and the mod gets no package.**

- A define that is a static switch of the shader is set in the material's `switches`, any other
  in its `shaderMacros`. A value the material holds is replaced; a switch or a macro it lacks is
  added.
- The candidates are the base defines of the files that lack the variant, each with the value
  the file pairs it with. Where several would do, the first by name and value is set.
- The move is a best effort that draws, as #ADR-0069's relink is: the rest of the material stays
  the author's.

## Consequences

- The 9 mods get a package, and their materials draw in a variant the author never chose. A
  move that proves wrong in game is a change to the rule, with the check in the record.
- A material two defines from every compiled variant still ends the run. How many is in
  `docs/design/RUNS.md`.
- The fixup runs after `retype` in the conform phase, so that the switches it reads are those
  the target build reads.
