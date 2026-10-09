# ADR-0085: Chroma VFX packs are classed apart

- **Status:** Accepted (2026-10-09)
- **Related:** #MOD-ARTIFACT-0037, #ADR-0033, #ADR-0082

## Context

A chroma VFX pack ships its work at each chroma of one skin, by design (#MOD-ARTIFACT-0037). A
port changes one target slot (#ADR-0082), which leaves open the mods with no skin0 unit; the 55
packs found are among those 128. Under one target slot a pack keeps one of its chromas.

The options open: class the packs apart from champion skins, take a skin with its chromas as one
target, or ship each chroma as a layer a player turns on.

## Decision

The class table classes chroma VFX packs `chroma-vfx-only`, and a corpus run leaves them out as
it leaves every class but `champion-skin` and `audio+skin` (#ADR-0033) (the user's call,
2026-10-09). The archives stay in their datasets. The port does not change: how it ships a pack
is decided later.

## Consequences

- 55 mods are classed so, 68 archives with their byte-identical copies; all were
  `champion-skin` (2026-10-09).
- The class table holds 5,109 mods in scope, and 5,066 of them are found.
- 73 mods with no skin0 unit stay in scope, the case #ADR-0082 leaves open.
- The reduced corpus names 2 of the packs, and a run over it no longer finds them.
- No corpus run measures a pack until a task runs the class.
