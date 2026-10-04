# ADR-0052: The app ports, and a repair is a preset of the advanced view

- **Status:** Accepted (2026-10-04)
- **Related:** #ADR-0011, #ADR-0050; `docs/design/ROADMAP-UX.md`, "Two views of one run"

## Context

A repair and a port are two runs of the library (#ADR-0011). A repair runs read, load, fix,
build, check and write, and gives the mod at its own slots and paths with whole bins, which
cover Riot's own at every patch. A port runs the same fixups, then facts, decide and plan, and
gives the mod over the game's own skin at each unit's target, as patches. A port can stop
where a repair would write: no skin is a possible target, or the animation reuse the policy
accepts is refused.

The app offered both, with a choice of kind above both views.

## Decision

The app ports, the user's choice of 2026-10-04.

- The simple view runs a port, with the standard settings and the default policy.
- The advanced view starts from the same port. A repair is a preset beside the standard
  settings, and the view says what a repair gives while it is chosen.
- No choice of kind is shown above the views.

## Consequences

- A user of the simple view gets a port's package: patches over the game's skin, not bins that
  cover Riot's.
- A mod that no port takes stops in the simple view, with its findings shown. Its repair is in
  the advanced view, under the preset.
- A request that names no kind is a port.
