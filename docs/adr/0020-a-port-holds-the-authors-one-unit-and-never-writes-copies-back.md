# ADR-0020: A port holds the author's one unit and never writes copies back

- **Status:** Accepted (2026-09-29); several clusters of one character each ported superseded
  by #ADR-0082
- **Related:** #MOD-ARTIFACT-0001, #ADR-0012, #ADR-0018

## Context

Mods write their skin bin to every slot so that the mod shows whatever skin the player picks
(#MOD-ARTIFACT-0001).

Detecting the copies finds the one unit the author made. The output could then spread that unit
over the slots again, as the mod did.

## Decision

The output holds the one unit, at its target. Copies leave the port, and no step writes them
back (the user's call, 2026-09-29, during ltk-forge).

Showing a skin0 mod on every skin is the mod manager's job. LTK Manager's built-in base-skins
mod makes every skin past the base stand in for the base. That is the only direction: nothing
spreads a skinN mod onto skin0.

## Consequences

- A spread whose work fits only its premium origin becomes a mod for that skin, and players of
  the base do not see it.
- A spread that also ships the game's own bins for other slots, unchanged or from an older
  build, drops those units too.
- Several distinct clusters in one character are several units, each ported.
