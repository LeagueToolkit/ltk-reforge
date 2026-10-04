# ADR-0056: A resolver under the slot's name stands in for a link nothing loads

- **Status:** Accepted (2026-10-05)
- **Related:** #MOD-ARTIFACT-0002, #GAME-BEHAVIOR-0008

## Context

A port carries the objects a unit's skin entry reaches. The entry reaches its particle systems
through the resolver it links in `mResourceResolver`.

A premium skin pasted into the base slot can keep the link to skin N's resolver while the mod
ships its resolver under the base's name (#MOD-ARTIFACT-0002). At the base no loaded bin holds
skin N's resolver. The link dangles, the mod's resolver is reached by nothing, and the port on
the base carried none of the mod's particle systems. The same mod ported to skin N carried
them, because there the mod's resolver takes the name the entry links.

The options were to leave the port on the base without the particles, to send such a mod to
skin N, or to take the mod's resolver under the slot's name as the unit's.

## Decision

When the resolver an entry links resolves to nothing loaded at the target, and the unit's load
holds a resolver under the target slot's own name, that resolver is the unit's (the user's
call, 2026-10-05). Its map is laid over the game's resolver and its objects are carried, as for
a resolver the entry links. The link is not reported as dangling.

A link that resolves, in the mod or in the game, is followed as it is.

## Consequences

- A pasted mod that fits the base keeps its particle systems there.
- The port gives the spells the mod's effects, whatever the client did with the dangling link
  in the mod as it was.
