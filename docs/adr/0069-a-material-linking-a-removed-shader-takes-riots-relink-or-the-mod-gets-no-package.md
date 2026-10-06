# ADR-0069: A material linking a removed shader takes Riot's relink, or the mod gets no package

- **Status:** Accepted (2026-10-06)
- **Related:** #GAME-CHANGE-0024, #GAME-BEHAVIOR-0023, #GAME-BEHAVIOR-0022; `docs/design/ROADMAP.md`,
  "Rules"

## Context

Riot removes shaders from `data/shaders/shaders.bin` at a build, and its own materials that
named one link another from that build (#GAME-CHANGE-0024). A mod made before keeps the old link,
and the client crashes while it loads a material whose pass links a shader it does not hold
(#GAME-BEHAVIOR-0023). 104 mods of the datasets hold such a material in a bin the client loads,
15 shaders in all.

Riot rewrote each relinked material along with its link: samplers, parameters, switches and
defines changed with the shader. The link alone is not Riot's conversion. For six of the 15 no
material of Riot's was relinked: every one that named the shader left the game with it.

The options: leave the link, and the game crashes; relink to the shader Riot's materials took,
and the material draws with the author's samplers and parameters; relink by the closest name, as
two other fixers do, which is not what Riot did; or give the mod no package.

## Decision

The user, 2026-10-06: **a material's pass linking a shader the target's shader bin lacks is
relinked to the shader Riot's own materials link at the build that removed it, and a mod with a
link that has no such relink gets no package.**

- Where Riot's materials took several shaders, the relink is to the one most of them took.
- The relink is a best effort that draws, not Riot's conversion: the rest of the material stays
  the author's.
- A link to a shader with no relink in the table, or to a name no shader of the game ever had,
  ends the run with a finding that names the material and the shader.

## Consequences

- A mod that crashed the game at load draws its relinked materials, possibly unlike the author's
  game drew them.
- A mod whose material links a shader with no relink ends with no package, as the roadmap's rules
  have it for a mod that cannot be done correctly. How many is in `docs/design/RUNS.md`.
- A relink that proves wrong in game is a change to the table, with the check in the record.
