# ADR-0021: A borrowed effects texture is legitimate

- **Status:** Accepted (2026-09-29)
- **Related:** #MOD-ARTIFACT-0003, #ADR-0014

## Context

Authors build effects from textures of other skins and other champions (#MOD-ARTIFACT-0003).
A rule that counts such files as evidence of a unit's origin calls custom work a paste.

Borrowed animation is another matter and is judged by the target policy (#ADR-0014).

## Decision

A texture borrowed from another champion or another skin is the author's legitimate work (the
user's call, 2026-09-29, during ltk-forge).

- It is never flagged, stripped, repointed or changed for being borrowed.
- Another champion's skin folder is no evidence of a unit's origin, whatever the file.
- An effects file does not count for the weak file tally of a unit's origin, even from the
  champion's own premium skins.
- A check that a port names no skin other than its target leaves effects textures out.

## Consequences

- A unit whose only tie to a premium skin is borrowed effects is custom work, and stays on
  skin0.
