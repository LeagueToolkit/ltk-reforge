# ADR-0083: A name only another skin holds is copied under the target's namespace

- **Status:** Accepted (2026-10-09)
- **Related:** #ADR-0082, #GAME-BEHAVIOR-0006, #GAME-BEHAVIOR-0007

## Context

A unit's objects name other objects. At the target, a name resolves only in the bins loaded
there (#GAME-BEHAVIOR-0006): the port's own, the game's skin at the target, and the game bins
the port links. A paste of a premium skin at skin0 keeps names that only that skin's bins hold,
and at skin0 they resolve to nothing: the effect is not there.

A port changes its target slot only (#ADR-0082). Over the corpus, 179 shipping units in 164
mods reach 1,585 names that nothing loaded at their target holds and the game's bins of another
skin of the character hold, nearly all effects systems (`docs/design/RUNS.md`, "Names other
skins hold"). Two ways were open:

- link the game bin of the skin that holds the name, as the port does for a retargeted unit's
  source slot. Every object of that bin then loads at the target, the other skin's entry and
  resolver among them;
- copy the objects the unit needs into its own bins, under the target's namespace.

## Decision

The objects are copied (the user's call, 2026-10-09). A name the target's unit reaches that
nothing loaded at the target holds, and the game's bins of another skin of the character hold
at the target build, is carried as a copy of the game's object: of the unit's premium origin
when its bins hold the name, else of the lowest slot whose bins hold it.

- The copy takes a name under the target's namespace, as a carried object does, and every link
  to the name follows it, the resolver's among them.
- What a copy names that nothing loaded at the target holds, and other skins' bins hold, is
  copied the same way, link by link.
- The files a copy names stay at the game's paths: a file the champion's WAD holds is there at
  any skin (#GAME-BEHAVIOR-0007). No file ships for a copy.
- A name no skin's bins hold stays dangling, and `object.link-dangling` names it.

## Consequences

- 179 units of 164 mods carry 1,585 copies more, and no file more (2026-10-09). None reaches a
  name that nothing holds.
- A copy is the game's object at the port's build. A later change of the other skin's object
  does not reach it, as with any port.
- PROJECT Urgot 1.5.0 and Ben 10 Ultimate Alien Udyr gain nothing: the history holds none of
  the names they leave dangling.
- A retargeted unit keeps its link to its source slot's game bin. Whether that link becomes
  copies too is not decided here.
