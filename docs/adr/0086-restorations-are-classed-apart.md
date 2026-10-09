# ADR-0086: Restorations are classed apart

- **Status:** Accepted (2026-10-09)
- **Related:** #MOD-ARTIFACT-0038, #ADR-0019, #ADR-0023, #ADR-0033, #ADR-0082, #ADR-0085

## Context

A restoration brings back a champion's older look or voice, and many ship several skins on
purpose (#MOD-ARTIFACT-0038). A port changes one target slot unless an option keeps every unit
(#ADR-0082), and a restoration often needs `--keep-stale` as well (#ADR-0023). The class table
classes restorations `champion-skin` with the custom skins, so a corpus run ports them as custom
skins. The chroma VFX packs, the other shape that ships several skins on purpose, are classed
apart (#ADR-0085).

## Decision

The class table classes a restoration by what it brings back (the user's call, 2026-10-09):

- `champion-old`: the look: models, textures, animations or effects;
- `champion-old+vo`: the look and the voice;
- `champion-old-vo`: the voice alone.

A corpus run leaves the three classes out, as it leaves every class but `champion-skin` and
`audio+skin` (#ADR-0033). The archives stay in their datasets.

A mod is classed so when its name or description says old in the game's sense and the old
content is its own champion's (#MOD-ARTIFACT-0038). The voice counts when the mod's VO banks hold
10 own clips or more.

## Consequences

- 157 mods are classed so, 198 rows with their byte-identical copies: 120 `champion-old`, 76
  `champion-old+vo` and 2 `champion-old-vo`; all were `champion-skin` (2026-10-09).
- The class table holds 4,952 mods in scope, and 4,909 of them are found.
- 93 of the 459 mods with extra units leave the runs, and the reduced corpus names 4 of the 157.
- A corpus run no longer measures restorations: a case of #ADR-0019 or #ADR-0023 that only
  restorations show is not seen until a task runs the classes.
- A restoration that does not say old stays `champion-skin`. Mods of the class `audio` were not
  read, and a voice restoration among them stays `audio`.
